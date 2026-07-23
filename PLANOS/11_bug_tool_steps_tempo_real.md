# Bug: Tool steps não aparecem em tempo real durante execução

> **Status:** ✅ Concluído
> **Prioridade:** Alta (regression)
> **Tipo:** Bug
> **Arquivos prováveis:** `src/components/ChatView.vue`

## Problema

Os tool steps (execução de ferramentas) só aparecem na tela **depois** que o modelo para de responder. Durante a execução, o usuário não vê os passos sendo realizados — deveria ver cada tool call aparecendo em sequência, em tempo real.

## Causa raiz

A timeline computation em `ChatView.vue` (linhas 28-60) itera sobre `sessionStore.messages` e, para cada mensagem de assistente com `tool_calls`, fatia os tasks correspondentes de `sessionStore.tasks`:

```typescript
sessionStore.messages.forEach((m, i) => {
  // ...
  } else if (m.role === "assistant") {
    const tcCount = m.tool_calls?.length ?? 0;
    if (tcCount > 0) {
      const batch = allTasks.slice(taskOffset, taskOffset + tcCount);
      // ...
    }
  }
});
```

**O problema:** durante a execução, a mensagem do assistente com `tool_calls` ainda **não foi adicionada** a `sessionStore.messages` — ela só é salva e recarregada quando o turno termina (`onAgentDone` → `reloadCurrent`). Os tasks live (empurrados pelo `onToolCall` handler) estão em `sessionStore.tasks`, mas a timeline não os renderiza porque não há mensagem de assistente correspondente para "ancorar" o slice.

Resultado: os tasks existem no store mas são invisíveis na timeline até o `reloadCurrent` recarregar as mensagens do backend.

## Correção

### Tarefa 1.1 — Mostrar tasks "live" no final da timeline
- Após o loop `forEach` das mensagens, verificar se há tasks em `allTasks` que não foram consumidos pelo `taskOffset`:
  ```typescript
  // Após o forEach:
  const remainingTasks = allTasks.slice(taskOffset);
  if (remainingTasks.length > 0) {
    items.push({ kind: "steps", key: "live-steps", tasks: remainingTasks });
  }
  ```
- Isso garante que tasks empurrados pelo `onToolCall` (que ainda não têm mensagem de assistente correspondente) apareçam no final da timeline em tempo real.

### Tarefa 1.2 — Mostrar tasks live mesmo sem mensagem de assistente anterior
- Se o `taskOffset` nunca foi inicializado (primeira mensagem do turno ainda não chegou), os tasks live ainda precisam aparecer. O slice `allTasks.slice(taskOffset)` já cobre isso porque `taskOffset` começa em 0 e só avança quando encontra tasks associados a mensagens.

### Tarefa 1.3 — Garantir reatividade
- O `computed` da timeline já depende de `sessionStore.tasks` (via `allTasks`) e `sessionStore.messages`. Confirmar que mudanças em `sessionStore.tasks` (push de novo task via `onToolCall`) disparam re-computação. Se não, adicionar `sessionStore.tasks.length` como dependência explícita.

### Tarefa 1.4 — Validar
- Reproduzir: enviar mensagem que gera tool calls → verificar que cada step aparece na tela assim que o `agent:tool_call` é emitido, não só no final do turno.

## Critério de conclusão
- Tool steps aparecem em tempo real durante a execução
- Após o turno terminar, a timeline continua correta (sem duplicação de steps)
- Tasks live desaparecem da seção "live" e aparecem na posição correta após `reloadCurrent`
