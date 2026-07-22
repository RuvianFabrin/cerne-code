# Cerne Code — Log de Desenvolvimento

> **Data:** 2026-07-22
> **Branch:** main (c:\cerne)
> **Commit:** `312efd2`
> **Build:** `src-tauri\target\release\bundle\` (NSIS + MSI)

---

## 1. Redesign do Chat (UI)

### O que foi feito
- **Removida a sidebar de tarefas** (TaskList + resizer) do `ChatView.vue` — tool calls já apareciam inline via `TaskStepGroup`, a sidebar era duplicata
- **Removidas bordas** das bolhas de mensagem (`MessageBubble.vue` + bolha de streaming em `ChatView.vue`)
- **Removidos labels** "Cerne Code" / "Você" das mensagens
- **Fonte mais fina** — `font-weight` global 500→400, headings 700→600, th 600→500

### Arquivos alterados
| Arquivo | Mudança |
|---|---|
| `src/components/ChatView.vue` | Removida sidebar (TaskList import, tasksWidth, tasksCollapsed, drag handlers, CSS), removida borda/label da bolha streaming, font-weights reduzidos |
| `src/components/MessageBubble.vue` | Removidas bordas, removidos role-labels, bolha do assistente = texto limpo sem background |
| `src/components/MarkdownContent.vue` | font-weight 500→400, headings 700→600, th 600→500 |
| `src/components/TaskStepGroup.vue` | font-weights 500→400, 600→500 |
| `src/style.css` | body font-weight 500→400 |

---

## 2. Bug: Panic UTF-8 no `web_search`

### Causa
`websearch.rs:219` — `&r.snippet[..300]` panicava quando byte 300 caía no meio de um caractere UTF-8 multibyte (ex: `—` = 3 bytes). O panic matava a thread tokio e o `web_search` ficava "running" para sempre.

### Correção
```rust
// ANTES
let snippet = if r.snippet.len() > 300 {
    format!("{}...", &r.snippet[..300])
// DEPOIS
let snippet = if r.snippet.chars().count() > 300 {
    format!("{}...", r.snippet.chars().take(300).collect::<String>())
```

### Arquivo
- `src-tauri/src/agent/websearch.rs` linha ~219

---

## 3. Bug: Loading infinito nos tool steps

### Causa
`mod.rs:534` — `tasks.iter_mut().find(|t| t.id == task_id)` sempre atualizava o **primeiro** task com aquele id. Quando o modelo envia múltiplos tool calls com o mesmo `call.id` (ex: todos `"call_0"`), só o primeiro recebia status "done", os outros ficavam "running" para sempre.

### Correção
Rastrear o índice do task diretamente:
```rust
// ANTES
if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
// DEPOIS
let task_idx = tasks.len(); // antes do push
// ...
if let Some(task) = tasks.get_mut(task_idx) {
```

### Arquivo
- `src-tauri/src/agent/mod.rs` linhas ~366 e ~534

---

## 4. Preview de raciocínio durante "Pensando..."

### Problema
O modelo Qwen envia tokens de raciocínio em `delta["reasoning_content"]` antes da resposta final. Esses tokens eram ignorados — durante a fase de thinking, `delta["content"]` é null e nada chegava ao frontend. O usuário via "Pensando..." estático sem feedback.

### Correção (3 camadas)

**Backend** — `src-tauri/src/providers/mod.rs`:
```rust
// Após o bloco de delta["content"], adicionar:
if let Some(thinking) = delta["reasoning_content"].as_str() {
    if !thinking.is_empty() {
        let _ = app.emit("chat:thinking_token", StreamTokenEvent {
            session_id: session_id.to_string(),
            delta: thinking.to_string(),
        });
    }
}
```

**Frontend API** — `src/api.ts`:
```typescript
export function onThinkingToken(cb: (sessionId: string, delta: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; delta: string }>("chat:thinking_token", (e) => cb(e.payload.session_id, e.payload.delta));
}
```

**Frontend Store** — `src/stores/session.ts`:
- Novo estado: `thinkingText: ""`
- Listener acumula tokens, limpa quando content chega ou agent termina

**Frontend UI** — `src/components/ChatView.vue`:
- Computed `thinkingTail` mostra últimos 200 chars do raciocínio
- Template: `<div v-if="sessionStore.thinkingText" class="thinking-preview">` com ícone 🧠 e texto italic cinza

---

## 5. Sandbox auto-accept

### Problema
`write_file`/`edit_file` escreviam na sandbox mas o arquivo nunca chegava ao disco real. O agente ficava em loop tentando rodar scripts que não existiam. Em Auto mode ninguém clicava "Aceitar", em Manual mode o card não era claro.

### Correção (2 camadas)

**Backend** — `src-tauri/src/agent/mod.rs`:
Em Auto mode, aplica o diff **sincronamente** no backend logo após escrever na sandbox (sem race condition):
```rust
if session.execution_mode == ExecutionMode::Auto {
    let _ = sandbox::accept_edit(
        std::path::Path::new(sandbox_path),
        std::path::Path::new(target_path),
    );
    walk_cache::invalidate(std::path::Path::new(target_path));
}
```
Import adicionado: `use crate::{providers, sandbox, sessions, skills, AppState};`

**Frontend** — `src/stores/session.ts`:
Auto-aceita em ambos os modos (em Manual, a aprovação do tool call já serve como permissão):
```typescript
await onPendingEdit((edit) => {
    if (edit.session_id !== this.currentId) return;
    this.pendingEdits.push(edit);
    this.acceptEdit(edit.id);
});
```

---

## 6. `read_file` com offset/limit (leitura parcial)

### Problema
`read_file` lia o arquivo **inteiro** na memória e truncava em 500K chars. Para arquivos grandes, desperdício de memória e tokens.

### Correção
Novos parâmetros opcionais `offset` (linha inicial 0-based) e `limit` (máx de linhas, default 2000). O retorno inclui header com total de linhas:
```
[linhas 101-200 de 5432]
<conteúdo>
```

### Arquivo
- `src-tauri/src/agent/tools.rs` — tool spec + execução

---

## 7. TodoList visual no chat

### O que foi feito
Nova ferramenta `todo_list` que permite ao LLM criar/atualizar uma lista de tarefas visível no chat. Cada chamada substitui a lista inteira. Status: `pending`, `in_progress`, `completed`.

### Backend
- **Tool spec** em `tools.rs` → `always_tool_specs()` (disponível sem projeto)
- **Handler** em `mod.rs` → emite evento `agent:todo_update` com `{session_id, todos}`
- **Label** em `taskLabels.ts` → `"todo_list": "Atualizou a lista de tarefas"`

### Frontend
- **API** — `src/api.ts`: tipo `TodoItem` + listener `onTodoUpdate`
- **Store** — `src/stores/session.ts`: estado `todoSnapshots: {turn, todos}[]`, listener acumula snapshots, limpa em `selectSession`
- **Componente** — `src/components/TodoCard.vue`: card com ícones de status (círculo cinza = pending, spinner azul = in_progress, check verde = completed, texto riscado = completed)
- **Timeline** — `src/components/ChatView.vue`: novo tipo `todo` no `TimelineItem`, interleaved com steps e mensagens na ordem real da conversa

### System prompt
Atualizado em `mod.rs` com instruções sobre `todo_list`, `read_file` offset/limit, e `write_file` auto-accept.

---

## 8. Scroll do chat

### Problema
O texto final do chat não ficava totalmente visível — o padding-bottom era insuficiente.

### Correção
`src/components/ChatView.vue` — `.chat-inner` padding-bottom: `12px` → `80px`

---

## 9. Timeline interleaved

### Problema
TodoCards e tool steps apareciam todos agrupados após a mensagem do usuário, em vez de intercalados com o texto do assistente na ordem real.

### Correção
`src/components/ChatView.vue` — timeline computation reescrita para processar cada mensagem do assistente individualmente, consumindo tasks via `taskOffset` cursor e intercalando steps → TodoCards → texto na ordem correta.

---

## 10. Arquivos de teste criados

| Arquivo | Conteúdo |
|---|---|
| `D:\testeQwen\ferramentas.md` | Documentação das 17 ferramentas com parâmetros |
| `D:\testeQwen\hello.py` | Script Python de teste para ast_grep/ast_edit |
| `C:\Users\ru\AppData\Roaming\com.cerne.app\skills\teste-skill\SKILL.md` | Skill de teste para load_skill |
| `C:\Users\ru\AppData\Roaming\com.cerne.app\sessions\test-tools-001\` | Sessão de teste com D:\testeQwen |

---

## 11. Ferramentas testadas (19/19 ✅)

| # | Ferramenta | Resultado |
|---|-----------|-----------|
| 1 | `list_dir` | ✅ |
| 2 | `read_file` | ✅ (com offset/limit) |
| 3 | `write_file` | ✅ (auto-accept) |
| 4 | `edit_file` | ✅ (auto-accept) |
| 5 | `grep` | ✅ |
| 6 | `run_command` | ✅ |
| 7 | `run_command` (background) | ✅ |
| 8 | `check_background_output` | ✅ |
| 9 | `stop_background` | ✅ |
| 10 | `list_background` | ✅ |
| 11 | `web_search` | ✅ (após fix do panic) |
| 12 | `web_fetch` | ✅ |
| 13 | `task` | ✅ |
| 14 | `ast_grep` | ✅ |
| 15 | `ast_edit` | ✅ |
| 16 | `load_skill` | ✅ |
| 17 | `ask` | ✅ |
| 18 | `verify_completion` | ✅ |
| 19 | `todo_list` | ✅ (visual no chat) |

---

## 12. Próxima feature: `computer_use` (automação de tela)

> ⚠️ **LEIA COM ATENÇÃO ANTES DE IMPLEMENTAR QUALQUER TAREFA DESTE PLANO.**
> Esta feature dá ao LLM controle real sobre mouse, teclado e tela do usuário.
> Um erro de implementação pode causar perda de dados, cliques em botões errados,
> ou execução de ações destrutivas sem intenção. Cada tarefa abaixo tem regras
> de segurança que NÃO podem ser ignoradas ou simplificadas.

### 12.0 — Princípios de segurança (NÃO NEGOCIÁVEIS)

1. **Screenshot antes de toda ação** — Antes de clicar, digitar ou pressionar tecla, o sistema DEVE capturar um screenshot e enviar ao modelo para que ele confirme visualmente que o alvo está correto. Nunca clicar "às cegas" em coordenadas calculadas sem verificação visual.

2. **Verificação de posição do mouse** — Após mover o cursor (se houver cursor visível), capturar screenshot com crosshair/marcador na posição alvo e enviar ao modelo para confirmar ANTES de executar o clique. O modelo deve responder "posição correta" ou "posição errada, ajustar para X,Y".

3. **Requisito de visão** — Se o provider/modelo da sessão NÃO suporta visão (imagens), as ferramentas de `computer_use` DEVEM ser desabilitadas e retornar erro claro: `"computer_use requer um modelo com suporte a visão (ex: qwen-vl, gpt-4o, claude-3). O modelo atual não suporta imagens."` Sem visão, o modelo não pode interpretar screenshots e vai clicar em posições aleatórias.

4. **Nunca executar ações destrutivas sem confirmação** — Fechar janelas, deletar arquivos via UI, formatar campos, enviar formulários com dados reais, clicar em "Sim" em diálogos de confirmação do SO — tudo isso DEVE pausar e pedir aprovação explícita do usuário via `ask`, mesmo em modo Automático.

5. **Log de auditoria** — Toda ação de computer_use (screenshot, click, type, key) deve ser registrada no tasks.json com timestamp, coordenadas, e resultado. O usuário deve poder revisar o histórico completo.

6. **Rate limiting** — Máximo de 1 ação de mouse/teclado por segundo para evitar loops descontrolados. Se o modelo tentar chamar click 10x seguidas, o sistema deve recusar após a 3ª e pedir explicação.

7. **Área de atuação limitada** — Por padrão, o computer_use só pode interagir com janelas que o usuário explicitamente autorizou (via `allow_window` tool ou configuração). Sem autorização, retorna erro. Isso previne o modelo de clicar acidentalmente em janelas de banco de dados, email, etc.

### 12.1 — O que já existe no projeto

**`scripts/cdp.mjs`** — Cliente CDP (Chrome DevTools Protocol) funcional que já faz:
- `screenshot` — captura PNG da página WebView2
- `eval` — executa JavaScript na página
- `click` — clique em coordenadas CSS via `Input.dispatchMouseEvent`
- `type` — digita texto via `Input.insertText`
- `key` — pressiona teclas via `Input.dispatchKeyEvent`
- `list` — lista alvos CDP

**Limitação:** só funciona com a própria WebView2 do Cerne (via `--remote-debugging-port`), não com apps externos. Para apps externos precisa de automação de SO (Win32/X11/CGEvent).

**Como ativar CDP no dev:**
```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
npm run tauri dev
# noutro terminal:
node scripts/cdp.mjs screenshot out.png
```

### 12.2 — Arquitetura proposta

```
┌─────────────────────────────────────────────────┐
│  LLM (modelo com visão)                         │
│  ↓ chama tool                                    │
│  computer_use_screenshot / click / type / key    │
├─────────────────────────────────────────────────┤
│  Backend Rust (src-tauri/src/agent/computer.rs)  │
│  ┌───────────────────────────────────────────┐  │
│  │ 1. Verifica se modelo tem visão           │  │
│  │ 2. Verifica se janela está autorizada     │  │
│  │ 3. Rate limiting (1 ação/seg)             │  │
│  │ 4. Screenshot pré-ação (se click/type)    │  │
│  │ 5. Envia screenshot ao modelo p/ validar  │  │
│  │ 6. Executa ação via crate de SO           │  │
│  │ 7. Screenshot pós-ação p/ confirmar       │  │
│  │ 8. Log no tasks.json                      │  │
│  └───────────────────────────────────────────┘  │
│  ↓ usa crates por plataforma                     │
│  ┌─────────┬──────────┬──────────┐              │
│  │ Windows │  macOS   │  Linux   │              │
│  │ enigo   │  enigo   │  enigo   │ ← mouse/kbd  │
│  │ xcap    │  xcap    │  xcap    │ ← screenshot │
│  │ uiauto  │  AX API  │  atspi   │ ← AX tree   │
│  │ windows │  CGWin   │  x11/wl  │ ← windows   │
│  └─────────┴──────────┴──────────┘              │
├─────────────────────────────────────────────────┤
│  Frontend Vue                                    │
│  - Mostra screenshots inline no chat             │
│  - Mostra cursor overlay (opcional)              │
│  - Card de autorização de janela                 │
│  - Histórico de ações (auditoria)                │
└─────────────────────────────────────────────────┘
```

### 12.3 — Crates Rust por plataforma

| Funcionalidade | Crate | Win | Mac | Linux | Notas |
|---|---|:---:|:---:|:---:|---|
| Mouse + teclado | `enigo` | ✅ | ✅ | ✅ | Cross-platform, bem mantida |
| Screenshot tela | `xcap` | ✅ | ✅ | ✅ | Captura monitor inteiro ou janela |
| Screenshot janela | `xcap` | ✅ | ✅ | ✅ | `Monitor::capture_image()` ou por window |
| Listar janelas | `window_enumerator` | ✅ | ❌ | ❌ | Win-only; Mac/Linux usar FFI |
| Listar janelas (Mac) | `core-graphics` | ❌ | ✅ | ❌ | `CGWindowListCopyWindowInfo` |
| Listar janelas (Linux) | `x11rb` | ❌ | ❌ | ✅ | X11; Wayland precisa `wlroots` |
| AX tree (Win) | `uiautomation` | ✅ | ❌ | ❌ | UI Automation COM |
| AX tree (Mac) | `accessibility-sys` | ❌ | ✅ | ❌ | Accessibility API |
| AX tree (Linux) | `atspi` | ❌ | ❌ | ✅ | AT-SPI2 D-Bus |
| CDP (browser) | `chromiumoxide` | ✅ | ✅ | ✅ | CDP client Rust nativo |

### 12.4 — Tarefas de implementação (independentes)

Cada tarefa abaixo pode ser feita em um chat separado. Leia a tarefa inteira antes de começar.

---

#### Tarefa 12.4.1 — Verificação de visão do modelo

**Objetivo:** Antes de registrar as tools de computer_use, verificar se o modelo da sessão suporta visão. Se não suporta, as tools não aparecem na lista de tools disponíveis.

**Onde implementar:**
- `src-tauri/src/agent/mod.rs` — na função `run_turn`, antes de montar a lista de tools, checar `session.provider` + `session.model` contra uma lista de modelos com visão conhecida, OU usar o endpoint de capabilities do provider.
- `src-tauri/src/providers/mod.rs` — adicionar função `supports_vision(provider, model) -> bool` que consulta a API do provider (ex: OpenRouter `/models` retorna `architecture.input_modalities`).
- Para providers custom (como o Qwen do usuário), adicionar campo `supports_vision: bool` na configuração do custom provider em `config.rs`.

**Regra de segurança:** Se `supports_vision` retorna false, as tools `computer_use_*` NÃO são incluídas na lista de tools enviada ao modelo. O modelo nem sabe que elas existem. Se o modelo tentar chamar mesmo assim (hallucinação), o `execute_tool` retorna erro claro.

**Teste:** Criar sessão com modelo sem visão (ex: qwen3-8b text-only), verificar que computer_use não aparece. Criar sessão com modelo com visão, verificar que aparece.

---

#### Tarefa 12.4.2 — Tool `computer_use_screenshot`

**Objetivo:** Capturar screenshot de uma janela específica ou da tela inteira e retornar como imagem para o modelo.

**Tool spec:**
```json
{
  "name": "computer_use_screenshot",
  "description": "Captura screenshot de uma janela ou da tela inteira. REQUER modelo com visão.",
  "parameters": {
    "window_title": { "type": "string", "description": "Título parcial da janela (opcional, vazio = tela inteira)" },
    "pid": { "type": "integer", "description": "PID do processo (opcional, alternativa a window_title)" }
  }
}
```

**Implementação:**
- Usar `xcap` para capturar a tela ou janela específica
- Converter para PNG base64
- Retornar como conteúdo de imagem na resposta da tool (o modelo precisa receber como imagem, não como texto)
- **Importante:** a resposta da tool precisa ser uma mensagem com `content` no formato multi-part (texto + image_url base64), igual ao que já existe para imagens do usuário em `providers/mod.rs::to_wire_messages`

**Arquivos:**
- `src-tauri/src/agent/computer.rs` — NOVO módulo
- `src-tauri/src/agent/tools.rs` — adicionar tool spec em `always_tool_specs()` (condicional a visão)
- `src-tauri/src/agent/mod.rs` — handler especial (igual `ask`/`task`, precisa retornar imagem)
- `Cargo.toml` — adicionar `xcap`

**Teste:** Chamar screenshot sem argumentos, verificar que o modelo recebe e descreve o que vê na tela.

---

#### Tarefa 12.4.3 — Tool `computer_use_click` (com verificação pré-clique)

**Objetivo:** Clicar em coordenadas de tela, mas ANTES de clicar, capturar screenshot com marcador visual na posição alvo e enviar ao modelo para confirmar.

**Tool spec:**
```json
{
  "name": "computer_use_click",
  "description": "Clica em coordenadas de tela. Antes de clicar, captura screenshot com crosshair na posição para verificação. REQUER modelo com visão.",
  "parameters": {
    "x": { "type": "integer", "description": "Coordenada X em pixels de tela" },
    "y": { "type": "integer", "description": "Coordenada Y em pixels de tela" },
    "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Botão do mouse (default: left)" },
    "window_title": { "type": "string", "description": "Janela alvo (para validação de área autorizada)" }
  }
}
```

**Fluxo de segurança (OBRIGATÓRIO):**
1. Capturar screenshot da tela/janela alvo
2. Desenhar crosshair vermelho (ou círculo) nas coordenadas (x, y) no screenshot
3. Enviar screenshot marcado ao modelo como parte do resultado da tool ANTES de executar o clique
4. O modelo analisa e responde na próxima chamada se a posição está correta
5. Se o modelo chamar `computer_use_click` novamente com as mesmas coordenadas, isso serve como confirmação implícita → executar o clique real
6. Se o modelo chamar com coordenadas diferentes, repetir o processo

**Alternativa mais simples (Fase 1):** Executar o clique imediatamente mas capturar screenshot PÓS-clique e incluir no resultado da tool. O modelo vê o resultado e pode desfazer se clicou errado. Menos seguro mas mais rápido.

**Implementação:**
- `enigo` para o clique real (`Mouse::click`)
- `xcap` para screenshot pré e pós
- `image` crate para desenhar o crosshair no screenshot
- Rate limiting: mínimo 1 segundo entre cliques

**Arquivos:**
- `src-tauri/src/agent/computer.rs`
- `Cargo.toml` — adicionar `enigo`, `image`

**⚠️ REGRA CRÍTICA:** NUNCA clicar sem antes ter um screenshot da tela atual. O modelo precisa ver o estado atual da tela para decidir onde clicar. Se não houver screenshot recente (últimos 5 segundos), capturar um automaticamente antes de executar.

---

#### Tarefa 12.4.4 — Tool `computer_use_type_text`

**Objetivo:** Digitar texto no elemento focado.

**Tool spec:**
```json
{
  "name": "computer_use_type_text",
  "description": "Digita texto via teclado. O elemento alvo deve estar focado antes (via click). REQUER modelo com visão.",
  "parameters": {
    "text": { "type": "string", "description": "Texto a digitar" }
  }
}
```

**Regras de segurança:**
- Screenshot pré-ação para confirmar que o campo certo está focado
- Máximo 500 caracteres por chamada (evitar loops de digitação infinita)
- Log completo do texto digitado no tasks.json

**Implementação:** `enigo` → `Keyboard::type_text`

---

#### Tarefa 12.4.5 — Tool `computer_use_press_key`

**Objetivo:** Pressionar tecla ou combinação (Ctrl+C, Enter, Tab, etc.).

**Tool spec:**
```json
{
  "name": "computer_use_press_key",
  "description": "Pressiona uma tecla ou combinação. REQUER modelo com visão.",
  "parameters": {
    "key": { "type": "string", "description": "Nome da tecla: return, tab, escape, up, down, left, right, space, delete, home, end, pageup, pagedown, f1-f12, ou letra/dígito" },
    "modifiers": { "type": "array", "items": { "type": "string" }, "description": "Modificadores: ctrl, shift, alt, win/cmd" }
  }
}
```

**Regras de segurança:**
- Combinações perigosas bloqueadas por padrão: `Alt+F4`, `Ctrl+Shift+Esc`, `Win+R` + comandos destrutivos
- Lista de bloqueio configurável pelo usuário em Configurações
- Screenshot pós-ação para confirmar o efeito

---

#### Tarefa 12.4.6 — Tool `computer_use_list_windows`

**Objetivo:** Listar janelas abertas com PID, título, posição e tamanho.

**Tool spec:**
```json
{
  "name": "computer_use_list_windows",
  "description": "Lista janelas visíveis na tela com PID, título e geometria. Use para descobrir o PID/título antes de screenshot ou click.",
  "parameters": {}
}
```

**Implementação por plataforma:**
- **Windows:** `windows` crate → `EnumWindows` + `GetWindowText` + `GetWindowRect`
- **macOS:** `core-graphics` → `CGWindowListCopyWindowInfo`
- **Linux:** `x11rb` → `_NET_CLIENT_LIST` + `_NET_WM_NAME`

**Sem risco de segurança** — só leitura.

---

#### Tarefa 12.4.7 — Tool `computer_use_get_window_state` (AX tree)

**Objetivo:** Ler a árvore de acessibilidade de uma janela, retornando elementos clicáveis com índices estáveis.

**Tool spec:**
```json
{
  "name": "computer_use_get_window_state",
  "description": "Lê a árvore de acessibilidade (UI Automation / AX API / AT-SPI) de uma janela. Retorna elementos interativos com [element_index N] para usar em click_element. REQUER modelo com visão.",
  "parameters": {
    "pid": { "type": "integer", "description": "PID do processo" },
    "window_id": { "type": "integer", "description": "ID da janela (de list_windows)" }
  }
}
```

**Por que AX tree é melhor que coordenadas:**
- Elementos têm nomes/roles estáveis ("Botão Salvar", "Campo de email")
- Funciona mesmo se a janela estiver minimizada ou atrás de outra
- Não depende de resolução/DPI/posição da janela
- O modelo pode clicar por `element_index` em vez de coordenadas pixel

**Implementação:**
- **Windows:** `uiautomation` crate → walk tree, collect actionable elements
- **macOS:** `accessibility-sys` → `AXUIElementCopyAttributeValue`
- **Linux:** `atspi` → D-Bus AT-SPI2

**Esta é a tarefa mais complexa.** Sugestão: implementar só Windows na Fase 2, deixar Mac/Linux para Fase 3.

---

#### Tarefa 12.4.8 — Tool `computer_use_browser_execute` (CDP)

**Objetivo:** Interagir com páginas web em browsers Chromium/Edge via CDP, sem precisar de coordenadas de tela.

**Tool spec:**
```json
{
  "name": "computer_use_browser_execute",
  "description": "Executa JavaScript, clica em elementos CSS, ou extrai texto de uma página web via CDP. Funciona com Chrome, Edge, Brave, e a própria WebView do Cerne. REQUER --remote-debugging-port no browser alvo.",
  "parameters": {
    "action": { "type": "string", "enum": ["execute_javascript", "click_element", "get_text", "query_dom"] },
    "javascript": { "type": "string", "description": "JS a executar (para execute_javascript)" },
    "css_selector": { "type": "string", "description": "Seletor CSS (para click_element / query_dom)" },
    "port": { "type": "integer", "description": "Porta CDP (default 9222)" }
  }
}
```

**O que já existe:** `scripts/cdp.mjs` faz exatamente isso via Node.js. A tarefa é portar para Rust usando a crate `chromiumoxide` ou reimplementar o cliente CDP mínimo em Rust (o protocolo é JSON over WebSocket, ~200 linhas).

**Vantagem sobre mouse/teclado de SO:**
- Não precisa de screenshot para clicar (clica por seletor CSS)
- Não depende de posição da janela na tela
- Funciona com janelas minimizadas
- Pode executar JS arbitrário (mais poderoso que click/type)

**Limitação:** só funciona com browsers Chromium-based que tenham `--remote-debugging-port` ativado.

---

#### Tarefa 12.4.9 — Autorização de janelas + UI de permissão

**Objetivo:** Sistema de permissão para controlar quais janelas o computer_use pode interagir.

**Fluxo:**
1. Primeira vez que o modelo tenta interagir com uma janela, o Cerne pausa e mostra um card: "O agente quer interagir com a janela 'Google Chrome - Gmail'. Permitir? [Só esta vez] [Sempre para este app] [Negar]"
2. A decisão é salva por PID ou por nome do executável
3. Em modo Manual, toda ação de computer_use pede aprovação (igual tool calls normais)
4. Em modo Auto, janelas previamente autorizadas não pedem aprovação

**Armazenamento:** `computer_use_permissions.json` no app_data_dir, com lista de executáveis autorizados.

**Arquivos:**
- `src-tauri/src/agent/computer.rs` — checagem de permissão antes de cada ação
- `src/components/ComputerUsePermissionCard.vue` — card de permissão no chat
- `src/stores/session.ts` — estado de permissão pendente

---

#### Tarefa 12.4.10 — Exibição de screenshots inline no chat

**Objetivo:** Mostrar os screenshots capturados pelo computer_use diretamente no chat, para o usuário ver o que o modelo está "enxergando".

**Implementação:**
- O resultado da tool `computer_use_screenshot` inclui a imagem como data URL
- O `MarkdownContent.vue` já renderiza imagens via `<img>` se o markdown tiver `![...](data:image/png;base64,...)`
- Alternativamente, criar um componente `ScreenshotCard.vue` que mostra a imagem com borda e timestamp

**Arquivos:**
- `src/components/ScreenshotCard.vue` — NOVO (opcional, pode usar markdown image)
- `src/components/ChatView.vue` — renderizar screenshots no timeline

---

### 12.5 — Ordem recomendada de implementação

| Ordem | Tarefa | Dependência | Risco |
|:-----:|--------|:-----------:|:-----:|
| 1 | 12.4.1 — Verificação de visão | Nenhuma | Baixo |
| 2 | 12.4.6 — list_windows | Nenhuma | Baixo |
| 3 | 12.4.2 — screenshot | 12.4.1 | Baixo |
| 4 | 12.4.10 — Screenshots no chat | 12.4.2 | Baixo |
| 5 | 12.4.5 — press_key | 12.4.1 | Médio |
| 6 | 12.4.4 — type_text | 12.4.1 | Médio |
| 7 | 12.4.3 — click (com verificação) | 12.4.2 | **Alto** |
| 8 | 12.4.9 — Autorização de janelas | 12.4.3 | Médio |
| 9 | 12.4.8 — Browser CDP | Nenhuma | Baixo |
| 10 | 12.4.7 — AX tree | 12.4.6 | **Alto** |

### 12.6 — O que NÃO fazer

- ❌ **NUNCA** implementar click sem screenshot prévio ou posterior
- ❌ **NUNCA** habilitar computer_use para modelos sem visão
- ❌ **NUNCA** clicar em coordenadas sem que o modelo tenha visto um screenshot recente da tela
- ❌ **NUNCA** executar `Alt+F4`, `Ctrl+Shift+Esc`, ou combinações destrutivas sem whitelist explícita
- ❌ **NUNCA** pular a verificação de janela autorizada
- ❌ **NUNCA** armazenar screenshots em disco sem criptografia (podem conter dados sensíveis)
- ❌ **NUNCA** permitir que o computer_use interaja com janelas de gerenciadores de senha, terminais com sudo, ou diálogos de pagamento sem confirmação extra

---

## Prompt para colar em sessão existente (ensinar novas features ao LLM)

> **Novas funcionalidades disponíveis — use-as:**
>
> 1. **`todo_list`** — Crie e atualize uma lista de tarefas visual no chat. Use em tarefas com 3+ passos. Cada chamada substitui a lista inteira. Formato: `{"todos": [{"content": "descrição", "status": "pending|in_progress|completed"}]}`. Regras: no máximo 1 item `in_progress` por vez; marque `completed` quando terminar; mande TODOS os itens a cada chamada (não só os mudados). A lista aparece visualmente no chat a cada atualização.
>
> 2. **`read_file` com `offset` e `limit`** — Para arquivos grandes, use leitura parcial por linhas em vez de ler tudo. `offset` = linha inicial (0-based), `limit` = máximo de linhas. O retorno inclui o total de linhas do arquivo para você saber se precisa continuar lendo. Exemplo: `read_file(path="log.txt", offset=0, limit=100)` lê as primeiras 100 linhas. Para arquivos pequenos (<2000 linhas) pode ler sem offset/limit.
>
> 3. **`write_file` e `edit_file`** — Agora aplicam as alterações automaticamente no disco. Não precisa mais pedir para o usuário aceitar diff na interface.

---

## Estrutura de arquivos chave

```
src/
├── api.ts                    # Tipos + listeners Tauri events
├── style.css                 # CSS global (--cerne-border, font-weight)
├── taskLabels.ts             # tool name → label amigável
├── stores/session.ts         # Pinia store (estado + listeners)
├── components/
│   ├── ChatView.vue          # Timeline, scroll, status, thinking preview
│   ├── MessageBubble.vue     # Bolha de mensagem (sem borda/label)
│   ├── MarkdownContent.vue   # Render markdown (font-weights)
│   ├── TaskStepGroup.vue     # Tool steps inline (expansível)
│   ├── TodoCard.vue          # Card visual do todo_list
│   ├── ComposerBar.vue       # Input bar
│   └── ...
src-tauri/src/
├── agent/
│   ├── mod.rs                # Agent loop, system prompt, tool dispatch
│   ├── tools.rs              # Tool specs + execute_tool
│   ├── websearch.rs          # web_search + web_fetch
│   ├── subagent.rs           # task (sub-agente)
│   ├── verifier.rs           # verify_completion
│   ├── ast_tools.rs          # ast_grep + ast_edit
│   ├── background.rs         # Background jobs
│   └── computer.rs           # NOVO — computer_use (Fase 1+)
├── providers/
│   └── mod.rs                # SSE streaming (content + reasoning_content)
├── sandbox.rs                # Sandbox read/write/accept/reject
├── encoding.rs               # Detecção de encoding (UTF-8/16/1252)
├── models.rs                 # Tipos Rust (Session, ChatMessage, etc.)
└── lib.rs                    # Tauri commands (accept_edit, reject_edit, etc.)
scripts/
└── cdp.mjs                   # Cliente CDP para automação browser (já funcional)
```
