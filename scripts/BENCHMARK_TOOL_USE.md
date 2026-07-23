# Benchmark Tool Use — Cerne Code (2026-07-23)

## O que foi testado
Capacidade dos modelos de **usar as ferramentas do Cerne Code** corretamente via Ollama.
6 modelos × 6 tarefas (list_dir, grep, read_file, edit_file, web_search, run_command).

## Ranking Final

```
Pos | Modelo                     | Nota     | Tempo méd | Ponto fraco
----|----------------------------|----------|-----------|---------------------------
🥇 | gpt-oss:20b                | 60/60    |     4.0s  | Nenhum — perfeito
🥈 | ornith:9b                  | 49/60    |     4.6s  | grep (usou run_command)
🥉 | devstral-small-2:24b       | 49/60    |     5.1s  | grep (usou list_dir)
 4 | ministral-3:8b             | 48/60    |     2.1s  | grep (usou list_dir)
 5 | gemma4:12b-it-qat          | 41/60    |     7.8s  | grep + edit_file
 6 | qwen3-coder:30b            | 22/60    |     5.2s  | Gera tool call como TEXTO
```

## Detalhe por ferramenta

| Ferramenta | gpt-oss | ornith | devstral | ministral | gemma4 | qwen3-coder |
|------------|:-------:|:------:|:--------:|:---------:|:------:|:-----------:|
| list_dir   | ✅ 10   | ✅ 10  | ✅ 10    | ✅ 10     | ✅ 10  | ❌ 0 (texto) |
| grep       | ✅ 10   | ❌ 2   | ❌ 2     | ❌ 2      | ❌ 2   | ✅ 10        |
| read_file  | ✅ 10   | ⚠️ 7   | ⚠️ 7     | ⚠️ 6      | ⚠️ 7   | ❌ 0 (recusou) |
| edit_file  | ✅ 10   | ✅ 10  | ✅ 10    | ✅ 10     | ❌ 2   | ❌ 2         |
| web_search | ✅ 10   | ✅ 10  | ✅ 10    | ✅ 10     | ✅ 10  | ❌ 0 (texto) |
| run_command| ✅ 10   | ✅ 10  | ✅ 10    | ✅ 10     | ✅ 10  | ✅ 10        |

## Análise por modelo

### 🥇 gpt-oss:20b — PERFEITO (60/60)
- Único que acertou TODAS as 6 ferramentas, incluindo grep
- Rápido (4.0s média), argumentos sempre corretos
- **Melhor escolha como agente no Cerne Code**

### 🥈 ornith:9b — Muito bom (49/60)
- Rápido (4.6s), bom em edit_file, web_search, run_command
- Fraqueza: confunde grep com run_command
- **Bom custo-benefício (9B params)**

### 🥉 devstral-small-2:24b — Muito bom (49/60)
- Mesmo perfil do ornith, um pouco mais lento
- Fraqueza: confunde grep com list_dir

### 4. ministral-3:8b — Bom e MUITO rápido (48/60)
- **Mais rápido: 2.1s média** — 2x mais rápido que gpt-oss
- Mesmo problema com grep
- **Ótimo para tarefas simples onde velocidade importa**

### 5. gemma4:12b-it-qat — Fraco em tools (41/60)
- Apesar de bom em raciocínio/código, tool use é fraco
- Confunde edit_file com read_file, grep com list_dir
- **Não recomendado como agente — melhor para chat direto**

### 6. qwen3-coder:30b — Problema estrutural (22/60)
- Gera tool calls como TEXTO XML em vez do mecanismo tool_calls da API
- O Cerne não reconhece como tool call → ferramenta nunca executa
- **Incompatível com o agent loop do Cerne via Ollama**

## Recomendação para o Cerne Code

| Uso | Modelo | Por quê |
|-----|--------|---------|
| **Agente (tools)** | gpt-oss:20b | Único com 60/60 — usa todas as tools corretamente |
| **Agente leve** | ornith:9b ou ministral-3:8b | 48-49/60, rápidos, 8-9B params |
| **Chat (sem tools)** | gemma4:12b-it-qat | Bom raciocínio, mas fraco em tool use |
| **Evitar como agente** | qwen3-coder:30b | Tool calls em formato texto — incompatível |

## Observação importante
O **grep** foi a ferramenta mais difícil — só gpt-oss e qwen3-coder acertaram.
Todos os outros modelos tentaram usar `list_dir` ou `run_command` em vez de `grep`.
Isso sugere que a descrição da tool grep poderia ser mais clara para modelos menores.
