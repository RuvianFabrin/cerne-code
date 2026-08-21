---
name: project-manager
description: Decompõe um objetivo grande em sub-tarefas independentes e delega cada uma para um sub-agente (task), sintetizando os resultados no final. Use quando o pedido claramente envolve várias frentes de trabalho que podem ser feitas em paralelo ou quase-independentemente (ex. "monte o frontend E o backend", "implemente X e escreva os testes E a documentação").
---

## Objetivo

Coordenar um pedido grande demais pra resolver como uma sequência linear de passos, decompondo em
sub-tarefas que fazem sentido delegar separadamente — sem virar um pipeline fixo (isso é outra coisa,
ver a skill/tool `run_pipeline` pra quando o rigor de dev→qa→analista faz mais sentido que delegação
dinâmica).

## Quando usar (e quando NÃO usar)

Use quando o pedido tem **frentes de trabalho genuinamente separáveis** — ex: "crie a tela Angular
que consome a API E o backend Spring REST completo" (duas frentes técnicas distintas), ou "implemente
a funcionalidade E escreva os testes E atualize a documentação" (mesma funcionalidade, entregáveis
diferentes que não dependem um do outro pra começar).

NÃO use pra um pedido que já é uma sequência natural de passos únicos (isso é só trabalhar
normalmente, sem precisar de `task` nenhuma) nem pra quando as sub-tarefas dependem umas das outras
em cadeia (ex: "implemente X, depois teste X" — aqui o teste PRECISA do resultado do X, não são
independentes, delegar em paralelo não ajuda e pode até atrapalhar).

## Passo a passo

1. Decomponha o objetivo em sub-tarefas que sejam **realmente independentes** entre si (cada uma
   pode ser descrita e resolvida sem depender do resultado da outra).
2. Para cada sub-tarefa, chame `task({ description, prompt })` com um prompt **autocontido** — o
   sub-agente não vê o histórico desta conversa, então inclua tudo que ele precisa saber (contexto
   do projeto, convenções a seguir, onde os arquivos relevantes estão).
3. Se houver 2+ sub-tarefas independentes no mesmo turno e o provider for de API (não local), elas já
   rodam em paralelo automaticamente — não precisa fazer nada especial pra isso acontecer.
4. Delegue em **lotes pequenos e coerentes**, não tudo de uma vez — cada `task` em paralelo consome
   tokens de verdade; um objetivo com 10 frentes genuinamente independentes ainda deveria ser
   quebrado em 2-3 lotes, não uma rajada de 10 chamadas simultâneas.
5. Depois que os sub-agentes devolverem os relatórios finais, sintetize o resultado combinado pro
   usuário — não repasse os relatórios crus, uma síntese do que foi feito e se algo ficou pendente.
6. Se a síntese revelar que falta mais uma rodada de delegação (ex: um sub-agente encontrou um
   problema que gera uma nova sub-tarefa), repita o processo — mas com bom senso: isso não deveria
   virar uma cadeia longa e não-supervisionada de delegações.

## Nota sobre paralelismo e custo

Rodar várias `task` em paralelo via provider de API já mostra um aviso de custo/rate-limit pro
usuário (mecanismo existente do Cerne). Em provider local, as `task` rodam sempre em fila (uma de
cada vez) — não force paralelismo que o provider local não vai entregar de qualquer forma.
