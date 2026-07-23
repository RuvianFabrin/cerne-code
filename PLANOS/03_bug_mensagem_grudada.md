# Bug: Mensagem grudada após parar execução

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Alta (bug)
> **Tipo:** Bug
> **Arquivos prováveis:** `src/stores/session.ts`, `src/components/ChatView.vue`, `src-tauri/src/agent/mod.rs`

## Problema

Quando o usuário clica no botão de parar (stop) durante a execução do agente e depois envia uma nova mensagem, a nova mensagem do usuário aparece "grudada" na mensagem anterior do assistente (que foi interrompida), em vez de aparecer como um novo turno separado visualmente.

## Causa provável

Quando o agente é cancelado mid-turn, a mensagem do assistente pode ficar com `content` vazio ou parcial. A timeline computation no `ChatView.vue` pode estar agrupando a nova mensagem do usuário com o turno anterior porque o contador de `userTurn` não incrementa corretamente após um cancelamento.

Outra possibilidade: o backend não salva a mensagem do assistente interrompida no `messages.json`, então quando o frontend recarrega, a sequência de mensagens fica inconsistente (duas mensagens de usuário consecutivas sem assistente entre elas).

## Fase 1 — Diagnóstico

### Tarefa 1.1 — Reproduzir e capturar estado
- Reproduzir o bug: enviar mensagem → esperar agente começar → clicar stop → enviar nova mensagem
- Capturar screenshot do bug
- Dumpar o `messages.json` da sessão para ver a sequência de mensagens salva
- Verificar se há duas mensagens `role: "user"` consecutivas sem `role: "assistant"` entre elas

### Tarefa 1.2 — Analisar timeline computation
- Verificar no `ChatView.vue` se a timeline lida corretamente com mensagens de usuário consecutivas
- Verificar se o `userTurn` incrementa corretamente para cada mensagem de usuário

## Fase 2 — Correção

### Tarefa 2.1 — Backend: salvar mensagem do assistente ao cancelar
- No `mod.rs`, quando o turno é cancelado (via `cancel_turn`), salvar a mensagem do assistente com o conteúdo parcial que foi gerado até o momento
- Se nenhum conteúdo foi gerado, salvar uma mensagem com `content: "[Execução cancelada pelo usuário]"` para manter a alternância user/assistant

### Tarefa 2.2 — Frontend: timeline robusta a mensagens consecutivas
- Na timeline computation, garantir que cada mensagem de usuário incrementa `userTurn` independentemente de haver ou não assistente entre elas
- Se houver duas mensagens de usuário consecutivas, renderizar ambas com seus respectivos turnos

### Tarefa 2.3 — Validar com o usuário
- Reproduzir o cenário original e confirmar que a nova mensagem aparece separada
- Pedir screenshot de validação

## Critério de conclusão
- Após cancelar e enviar nova mensagem, a nova mensagem aparece como turno separado
- Não há mensagens "grudadas" visualmente
- `messages.json` mantém alternância user/assistant consistente
