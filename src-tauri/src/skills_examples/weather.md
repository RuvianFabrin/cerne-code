---
name: weather
description: Consulta a previsão do tempo pra uma cidade/local usando busca na web, sem precisar de chave de API nem instalar nada. Use quando o usuário perguntar sobre clima, temperatura, chuva ou previsão do tempo.
---

## Objetivo

Responder perguntas sobre tempo/clima usando as ferramentas de busca que o Cerne já tem
(`web_search`/`web_fetch`), sem depender de nenhuma API externa configurada.

## Passo a passo

1. Chame `web_search` com uma query direta tipo "previsão do tempo <cidade> hoje" (ou
   "amanhã"/"essa semana", conforme o pedido).
2. Se o resumo da busca já trouxer a informação (temperatura, condição, chance de chuva), responda
   direto — não precisa abrir a página.
3. Se faltar detalhe, use `web_fetch` na fonte mais confiável dos resultados (ex: um site de
   meteorologia conhecido) pra ler o conteúdo completo.
4. Sempre cite a fonte (nome do site) na resposta, e a data/hora da consulta se for relevante —
   previsão do tempo muda, então "verificado agora" importa.

## Observação

Não invente número de temperatura ou previsão sem ter buscado de verdade — sempre baseie a resposta
no que a busca realmente trouxe.
