# Alerta de consumo de tokens no computer_use

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Média (UX / transparência)
> **Tipo:** Feature UX
> **Arquivos prováveis:** `src/components/ChatView.vue`, `src/stores/session.ts`, `src-tauri/src/agent/mod.rs`

## Problema

Cada screenshot enviado ao modelo consome tokens de visão. Um screenshot 1920×1080 em PNG base64 tem ~200-800KB, que os modelos de visão tokenizam como ~1000-2000 tokens por imagem (dependendo do modelo). Como o computer_use tira screenshot após cada ação (click, type, key, scroll), uma sequência de 10 ações pode consumir 10-20K tokens só de screenshots — sem o usuário saber.

## Análise de consumo real

| Ação | Screenshots | Tokens estimados (Qwen-VL) |
|------|:-----------:|:--------------------------:|
| `screenshot` | 1 | ~1.500 |
| `click` | 1 (pós) | ~1.500 |
| `type_text` | 1 (pós) | ~1.500 |
| `press_key` | 1 (pós) | ~1.500 |
| `scroll` | 1 (pós) | ~1.500 |
| `list_windows` | 0 | 0 |
| `authorize` | 0 | 0 |
| `browser_execute` | 0 | 0 |
| `get_window_state` | 0 | 0 |
| `click_element` | 0 | 0 |

**Sessão típica de automação (20 ações):** ~30K tokens só de screenshots.
**Contexto total do Qwen:** 1M tokens → 30K = 3% do contexto. **Aceitável**, mas o usuário deve saber.

## Fase 1 — Alerta visual na primeira ação

### Tarefa 1.1 — Detectar primeira ação de computer_use com screenshot
- No `session.ts`, adicionar flag `computerUseWarned: boolean` (reseta ao trocar de sessão)
- No listener de `onToolCall`, se a tool começa com `computer_use_` e não é `list_windows`/`authorize`/`browser_execute`/`get_window_state`/`click_element`:
  - Se `computerUseWarned === false`, setar `showComputerUseWarning = true`
  - Setar `computerUseWarned = true`

### Tarefa 1.2 — Card de aviso no chat
- Componente `ComputerUseWarningCard.vue`:
  ```
  ⚠️ Automação de tela ativa
  Cada screenshot consome ~1.500 tokens de visão. Uma sessão de automação
  com 20 ações pode usar ~30K tokens extras. O contexto total do modelo
  é 1M tokens, então o impacto é pequeno (~3%), mas fique ciente.
  [Entendi, continuar] [Cancelar automação]
  ```
- Mostrar o card uma vez por sessão, antes da primeira ação com screenshot
- "Entendi, continuar" → esconde o card e deixa a ação prosseguir
- "Cancelar automação" → cancela o turno atual

### Tarefa 1.3 — Contador de screenshots na UI
- No `ContextGauge.vue` ou ao lado dele, mostrar contador: "📸 5 screenshots (~7.5K tokens)"
- Atualizar a cada screenshot enviado
- Reseta ao trocar de sessão

## Fase 2 — Otimização de tokens de screenshot

### Tarefa 2.1 — Reduzir resolução do screenshot
- Antes de enviar ao modelo, redimensionar o screenshot para max 1280px de largura
- Um screenshot 1280×720 consome ~500-800 tokens vs ~1500-2000 de um 1920×1080
- Economia de ~50% nos tokens de visão
- Implementar no `computer.rs` usando a crate `image` (já é dependência)

### Tarefa 2.2 — Comprimir PNG para JPEG
- JPEG com qualidade 80% é 3-5x menor que PNG para screenshots
- Menos bytes base64 = menos tokens
- Implementar no `rgba_to_base64` trocando `ImageFormat::Png` por `ImageFormat::Jpeg`

### Tarefa 2.3 — Screenshot sob demanda (não automático)
- Opção de configuração: "Screenshot automático após cada ação" (default: ON)
- Se OFF, o modelo precisa chamar `computer_use_screenshot` explicitamente quando quiser ver o resultado
- Reduz pela metade o número de screenshots em sessões longas
- Mas aumenta o risco do modelo agir sem ver o resultado

## Critério de conclusão
- Usuário vê aviso na primeira ação de computer_use
- Contador de screenshots visível na UI
- Screenshots otimizados (1280px + JPEG) para economizar ~50% de tokens
