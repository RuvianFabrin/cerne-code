---
name: skill-creator
description: Ajuda a criar uma nova skill pro Cerne Code. Use quando o usuário disser algo como "cria uma skill pra..." ou "eu queria que você sempre soubesse fazer X assim" ou "toda vez que eu pedir Y, faça Z".
---

## O que é uma skill no Cerne

Uma pasta com um `SKILL.md` dentro, neste formato:

```
---
name: nome-da-skill
description: Uma linha dizendo QUANDO usar essa skill.
---

Instruções detalhadas aqui - o que fazer, passos, convenções, exemplos.
```

O agente vê só `name`+`description` de cada skill listada no catálogo (pra não inflar o prompt à
toa) e carrega o corpo inteiro sob demanda, via a ferramenta `load_skill`, quando decide que é
relevante pro pedido atual.

## Passo a passo pra criar uma boa skill

1. **Nome**: curto, em `kebab-case` (minúsculo, hífen), só letras/números/hífen.
2. **Description**: a parte mais importante — precisa deixar claro QUANDO usar (e, se ajudar,
   quando NÃO usar). É a única coisa que o agente lê antes de decidir carregar ou não a skill.
3. **Corpo**: passo a passo concreto, convenções específicas do time/projeto, exemplos de
   entrada/saída. Escreva pra outro agente seguir, não pra um humano ler uma vez só.
4. **Escopo**: uma skill criada sem pasta de projeto aberta fica "global" (vale pra qualquer
   sessão); criada com um projeto aberto fica só daquele projeto (`.cerne/skills/`).

## Quando vale criar uma skill

Quando o usuário se pega explicando a MESMA coisa em conversas diferentes — um processo do time, um
formato de saída específico, os passos certos pra revisar um PR, como ele gosta que um relatório
seja estruturado, etc. Se for algo que só vai acontecer uma vez, não precisa virar skill.
