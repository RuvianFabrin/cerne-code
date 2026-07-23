# Contagem real de tokens (input/output) e requisições

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Alta (transparência de custo)
> **Tipo:** Feature
> **Arquivos prováveis:** `src-tauri/src/providers/mod.rs`, `src-tauri/src/models.rs`, `src-tauri/src/agent/mod.rs`, `src-tauri/src/context.rs`, `src-tauri/src/sessions.rs`, `src-tauri/src/lib.rs`, `src/components/ContextGauge.vue`, `src/stores/session.ts`, `src/api.ts`

## Problema

Hoje o gauge de contexto mostra apenas uma **estimativa** de tokens usados (`chars / 4`). O usuário não sabe quantos tokens reais foram consumidos (entrada + saída) nem quantas requisições foram feitas. Isso importa porque:

1. **Custo real:** o usuário é cobrado por requisição e por tokens (input/output separados). Sub-agentes (`task`) também consomem tokens e requisições, mas hoje são invisíveis.
2. **Estimativa imprecisa:** `chars/4` pode errar por 2-3x dependendo do modelo/tokenizer.
3. **Sem histórico acumulado:** o gauge mostra o contexto atual, não o total consumido na sessão.

## Como a API retorna usage

Na API OpenAI-compatible, o último chunk SSE antes de `[DONE]` (ou um chunk com `choices: []`) inclui:
```json
{
  "usage": {
    "prompt_tokens": 1234,
    "completion_tokens": 567,
    "total_tokens": 1801
  }
}
```
Alguns providers enviam `usage` no chunk `[DONE]` ou num chunk separado com `choices: []`. O código atual em `providers/mod.rs` ignora ambos.

## Fase 1 — Backend: capturar usage da API

### Tarefa 1.1 — Extrair usage do stream SSE
- Em `providers/mod.rs`, na função de streaming (`stream_chat` ou equivalente):
  - Antes de `continue` no `[DONE]`, verificar se o chunk anterior ou o payload `[DONE]` contém `usage`.
  - Também verificar chunks com `choices` vazio ou ausente: `if parsed["choices"].as_array().map_or(true, |c| c.is_empty())` → procurar `parsed["usage"]`.
  - Acumular `prompt_tokens` e `completion_tokens` do último chunk que tiver `usage`.
- Retornar o usage junto com a `ChatMessage` resultado (adicionar campo `usage` ao retorno ou usar um struct wrapper).

### Tarefa 1.2 — Propagar usage ao agent loop
- Em `agent/mod.rs`, após cada chamada ao modelo (incluindo sub-agentes via `task`), acumular os tokens reais:
  ```rust
  session_total_prompt_tokens += usage.prompt_tokens;
  session_total_completion_tokens += usage.completion_tokens;
  session_total_requests += 1;
  ```
- Salvar os acumuladores no `session.json` (persistir entre reinícios).

### Tarefa 1.3 — Emitir usage real ao frontend
- Expandir o evento `agent:context` (ou criar `agent:usage`) para incluir:
  ```rust
  pub struct ContextUsage {
      // campos existentes...
      pub real_prompt_tokens: Option<u32>,     // último turno
      pub real_completion_tokens: Option<u32>,  // último turno
      pub total_prompt_tokens: u32,             // acumulado na sessão
      pub total_completion_tokens: u32,         // acumulado na sessão
      pub total_requests: u32,                  // acumulado na sessão
  }
  ```
- Emitir após cada chamada ao modelo (não só no final do turno).

## Fase 2 — Backend: persistir acumuladores

### Tarefa 2.1 — Campos na Session
- Em `models.rs`, adicionar ao `struct Session`:
  ```rust
  #[serde(default)]
  pub total_prompt_tokens: u32,
  #[serde(default)]
  pub total_completion_tokens: u32,
  #[serde(default)]
  pub total_requests: u32,
  ```

### Tarefa 2.2 — Salvar após cada turno
- Em `agent/mod.rs`, após acumular os tokens do turno, chamar `sessions::save_session(...)` ou função dedicada `sessions::update_usage(...)`.

## Fase 3 — Frontend: mostrar na UI

### Tarefa 3.1 — Expandir ContextGauge
- Ao lado do gauge atual (barra de contexto), adicionar badges compactos:
  ```
  [📊 4.4k / 1000k]  [↓ 12.3k  ↑ 3.2k  🔄 8]
  ```
  - `↓` = total_prompt_tokens (entrada)
  - `↑` = total_completion_tokens (saída)
  - `🔄` = total_requests (requisições)
- Tooltip no badge: "Tokens nesta sessão — Entrada: 12.345 | Saída: 3.210 | Requisições: 8"
- Formato: usar `formatTokens` (12345 → "12.3k")

### Tarefa 3.2 — Atualizar tipos no frontend
- Em `api.ts`, expandir `ContextUsage` com os novos campos.
- Em `session.ts`, o listener `onContextUsage` já atualiza `contextUsage` — os novos campos chegam automaticamente.

### Tarefa 3.3 — Estilo
- Badges com mesmo estilo do gauge (pill, borda arredondada, fonte 11px)
- Cor neutra (#71717a), sem alarme visual (não é contexto, é custo acumulado)

## Critério de conclusão
- Tokens reais de entrada e saída são capturados da resposta da API
- Sub-agentes (`task`) também contam no acumulado
- O acumulado é persistido no `session.json`
- A UI mostra entrada, saída e número de requisições ao lado do gauge
- Os valores atualizam em tempo real após cada chamada ao modelo
