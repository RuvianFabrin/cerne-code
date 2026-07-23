# Controle de reasoning effort do modelo (low / medium / high)

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Média
> **Tipo:** Feature
> **Arquivos prováveis:** `src-tauri/src/providers/mod.rs`, `src-tauri/src/models.rs`, `src-tauri/src/sessions.rs`, `src-tauri/src/lib.rs`, `src/components/ComposerBar.vue`, `src/stores/session.ts`, `src/api.ts`

## Problema

Modelos de raciocínio (QwQ, o1, DeepSeek-R1, etc.) aceitam o parâmetro `reasoning_effort` na request OpenAI-compatible (`"low"`, `"medium"`, `"high"`). Hoje o Cerne não envia esse campo — o modelo usa o default dele (geralmente `"high"`), o que desperdiça tokens e tempo em tarefas simples. O usuário não tem como pedir uma resposta mais rápida/rasa nem forçar raciocínio profundo quando precisa.

## Como funciona na API

O campo vai direto no corpo da request POST `/chat/completions`:
```json
{
  "model": "qwq-32b",
  "messages": [...],
  "stream": true,
  "reasoning_effort": "low"
}
```
- Modelos que não suportam o campo simplesmente o ignoram (não dá erro).
- Valores válidos: `"low"`, `"medium"`, `"high"`.
- Sem o campo = default do modelo (equivalente a não enviar).

## Fase 1 — Backend: enviar reasoning_effort na request

### Tarefa 1.1 — Adicionar campo à Session
- Em `models.rs`, adicionar ao `struct Session`:
  ```rust
  /// Esforço de raciocínio enviado ao modelo (modelos que não suportam ignoram).
  /// None = não enviar o campo (default do modelo).
  #[serde(default)]
  pub reasoning_effort: Option<ReasoningEffort>,
  ```
- Novo enum:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum ReasoningEffort { Low, Medium, High }
  ```

### Tarefa 1.2 — Persistir e atualizar
- Em `sessions.rs`, adicionar função `update_reasoning_effort(app_data_dir, session_id, effort: Option<ReasoningEffort>)` — mesmo padrão de `update_context_length`.
- Em `lib.rs`, expor comando Tauri `update_session_reasoning_effort(id, effort)`.

### Tarefa 1.3 — Incluir na request HTTP
- Em `providers/mod.rs`, na função que monta o `body` da request (`stream_chat` ou equivalente):
  ```rust
  if let Some(effort) = reasoning_effort {
      body["reasoning_effort"] = json!(match effort {
          ReasoningEffort::Low => "low",
          ReasoningEffort::Medium => "medium",
          ReasoningEffort::High => "high",
      });
  }
  ```
- O parâmetro `reasoning_effort` deve ser passado da `Session` até a função de streaming (mesmo caminho do `context_length`).

## Fase 2 — Frontend: seletor no composer

### Tarefa 2.1 — Seletor ao lado do modo de execução
- Em `ComposerBar.vue`, ao lado do seletor Auto/Manual, adicionar um dropdown discreto:
  - Ícone: `psychology` (Material Symbol) ou `🧠`
  - Opções: `Auto` (não envia o campo), `Baixo`, `Médio`, `Alto`
  - Tooltip: "Esforço de raciocínio do modelo"
  - Default: `Auto`
- Ao mudar, chamar `api.updateReasoningEffort(sessionId, value)` e atualizar o store.

### Tarefa 2.2 — Estado no store
- Em `session.ts`, adicionar `reasoningEffort: "auto" | "low" | "medium" | "high"` ao estado.
- Carregar o valor ao selecionar sessão (`selectSession`).
- Listener não é necessário — a mudança é local e imediata.

### Tarefa 2.3 — API
- Em `api.ts`, adicionar:
  ```typescript
  export function updateReasoningEffort(id: string, effort: "low" | "medium" | "high" | null) {
    return invoke("update_session_reasoning_effort", { id, effort });
  }
  ```

## Critério de conclusão
- Usuário consegue escolher o esforço de raciocínio por sessão
- O campo `reasoning_effort` aparece na request HTTP quando diferente de "Auto"
- A escolha é persistida e recarregada ao reabrir a sessão
- Modelos que não suportam o campo não quebram (ignoram silenciosamente)
