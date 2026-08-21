---
name: summarize
description: Resume um texto longo, artigo, documento ou página web de forma densa e fiel ao original, sem adicionar opinião. Use quando o usuário pedir um resumo ou "TL;DR" de algo.
---

## Objetivo

Produzir um resumo curto e fiel ao original — sem inventar, sem opinar, sem inflar com floreio.

## Passo a passo

1. Se for uma URL, use `web_fetch` pra ler o conteúdo completo antes de resumir — nunca resuma só
   pelo título/trecho que apareceu numa busca.
2. Se for um arquivo, use `read_file` (ou a extração de anexo, se já veio anexado na conversa).
3. Identifique: do que trata, os pontos principais (3-6 tópicos costuma bastar) e a
   conclusão/decisão/resultado, se houver.
4. Escreva o resumo em bullet points ou 1-2 parágrafos curtos — o tamanho deve ser proporcional ao
   pedido do usuário ("resumo rápido" vs "resumo detalhado"), nunca maior do que precisa.
5. Não adicione opinião, avaliação de qualidade nem informação que não estava no texto original.

## Quando NÃO usar

Se o usuário pedir uma ANÁLISE (crítica, comparação, opinião) em vez de um resumo, isso é outro tipo
de pedido — não force o formato de resumo puro nesse caso.
