# Ajuste: Espaço entre chat e composer

> **Status:** ✅ Concluído
> **Prioridade:** Alta (UX)
> **Tipo:** Bug visual / CSS
> **Arquivos prováveis:** `src/components/ChatView.vue`

## Problema

Após o ajuste de `padding-bottom: 80px` no `.chat-inner` (feito para corrigir o scroll que não chegava ao final), ficou um espaço excessivo entre a última mensagem do chat e a barra de composição (composer). O espaço vazio dá a sensação de que o chat "terminou cedo" ou que há conteúdo faltando.

## Fase 1 — Ajuste fino do padding

### Tarefa 1.1 — Reduzir padding-bottom e testar
- Reduzir `padding-bottom` do `.chat-inner` de `80px` para um valor menor (testar `24px`, `32px`, `40px`)
- Verificar se o último texto do chat continua totalmente visível acima do composer
- O objetivo é o menor padding que ainda permita ver a última linha sem corte

### Tarefa 1.2 — Validar com o usuário
- Tirar screenshot do app com uma conversa longa
- Mostrar ao usuário e pedir confirmação: "O espaço entre o chat e o composer ficou adequado?"
- Se não, ajustar o valor e repetir

### Tarefa 1.3 — Considerar padding dinâmico
- Se o padding fixo não resolver para todos os casos, considerar usar `scroll-padding-bottom` no `.chat-scroll` em vez de padding no `.chat-inner`
- Isso permite que o scroll pare no ponto certo sem adicionar espaço visual vazio

## Critério de conclusão
- Última mensagem do chat visível sem corte
- Sem espaço vazio excessivo entre chat e composer
- Usuário validou visualmente
