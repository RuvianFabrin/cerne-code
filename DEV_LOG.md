# Cerne Code — Log de Desenvolvimento

> **Data:** 2026-07-22
> **Branch:** main (c:\cerne)
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

## 11. Ferramentas testadas (17/17 ✅)

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

## 12. Próxima feature planejada: `computer_use` (automação de tela)

### Objetivo
Permitir que o LLM controle o PC do usuário (screenshots, mouse, teclado) para fazer testes em sistemas/web sem intervenção humana.

### Fases propostas

**Fase 1 — MVP Windows** (~1 sessão de trabalho)
- Tools: `screenshot`, `click`, `type_text`, `press_key`, `list_windows`
- Crates: `enigo` (mouse/teclado cross-platform), `xcap` (screenshot cross-platform), `windows` crate (EnumWindows, GetWindowRect)
- Sem AX tree ainda — só coordenadas pixel

**Fase 2 — AX tree Windows**
- Tool: `get_window_state` com element_index
- Crate: `uiautomation` (Windows UI Automation)
- Permite clicar em elementos pelo nome/role em vez de coordenadas cruas

**Fase 3 — macOS + Linux**
- macOS: `core-graphics` + `accessibility` crate
- Linux: `atspi` + `x11`/`wayland` crates

**Fase 4 — Browser interaction**
- CDP (Chrome DevTools Protocol) para Chromium/Edge
- Executar JavaScript, ler DOM, clicar em elementos CSS

### Considerações de segurança
- Confirmação antes de ações destrutivas (fechar janelas, deletar arquivos via UI)
- Sandbox de coordenadas (limitar área de atuação)
- Log de todas as ações para auditoria

### Referências
- Qwen Code `computer_use` tools: `computer_use__click`, `computer_use__type_text`, `computer_use__get_window_state`, etc.
- `enigo` crate: https://crates.io/crates/enigo
- `xcap` crate: https://crates.io/crates/xcap
- `uiautomation` crate: https://crates.io/crates/uiautomation

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
│   ├── TodoCard.vue          # NOVO — card visual do todo_list
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
│   └── background.rs         # Background jobs
├── providers/
│   └── mod.rs                # SSE streaming (content + reasoning_content)
├── sandbox.rs                # Sandbox read/write/accept/reject
├── encoding.rs               # Detecção de encoding (UTF-8/16/1252)
├── models.rs                 # Tipos Rust (Session, ChatMessage, etc.)
└── lib.rs                    # Tauri commands (accept_edit, reject_edit, etc.)
```
