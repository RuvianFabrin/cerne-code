# Benchmark de Modelos Locais — 2026-07-23

## Metodologia
- 2 testes por modelo: **Raciocínio** (lógica/matemática) e **Código** (Python)
- Cada teste vale 10 pontos → máximo 20 pontos por modelo
- Medidos: tempo de resposta, tokens gerados, tokens/segundo
- Provider: Ollama (TurboQuant estava offline)
- `max_tokens: 512`, `stream: false`

## Ranking Final

| Pos | Modelo | Nota | Tempo méd | Tok/s | Observações |
|:---:|--------|:----:|:---------:|:-----:|-------------|
| 🥇 | **ornith:9b** | 20/20 | 7.7s | 37.6 | Melhor custo-benefício. Rápido, preciso, 9B params |
| 🥈 | **gemma4:12b-it-qat** | 20/20 | 13.4s | 33.4 | Excelente qualidade, QAT otimizado, visão |
| 🥉 | **gpt-oss:20b** | 20/20 | 17.6s | 40.4 | Melhor tok/s bruto, thinking nativo |
| 4 | **devstral-small-2:24b** | 20/20 | 15.8s | 8.0 | Preciso mas lento (24B params pesa) |
| 5 | **ministral-3:8b** | 16/20 | 8.3s | 26.8 | Rápido, mas arredondou 4.5→4. Visão |
| 6 | **qwen3-coder:30b** | 16/20 | 21.6s | 15.1 | Código perfeito, mas raciocínio lento (1.2 tok/s) |
| 7 | **qwen3.5:4b** | 2/20 | 7.9s | 72.7 | ⚠️ Respostas vazias — thinking consumiu os 512 tokens |
| 8 | **qwen3.5:9b** | 2/20 | 12.8s | 46.3 | ⚠️ Mesmo problema — modelo thinking, max_tokens insuficiente |

## Análise por categoria

### Raciocínio (lógica/matemática)
| Modelo | Nota | Resposta |
|--------|:----:|----------|
| ornith:9b | 10 | ✅ 4,5 — explicação clara e correta |
| gemma4:12b-it-qat | 10 | ✅ 4,5 — direto e preciso |
| gpt-oss:20b | 10 | ✅ 4.5 — explicação passo a passo |
| devstral-small-2:24b | 10 | ✅ 4.5 (arredondou pra 5, mas explicou) |
| ministral-3:8b | 6 | ⚠️ 4 — arredondou sem mencionar o .5 |
| qwen3-coder:30b | 6 | ⚠️ 4 — arredondou, mas explicou o cálculo |
| qwen3.5:9b | 2 | ❌ Vazio (thinking consumiu tokens) |
| qwen3.5:4b | 2 | ❌ Vazio (thinking consumiu tokens) |

### Código (Python)
| Modelo | Nota | Qualidade |
|--------|:----:|-----------|
| ornith:9b | 10 | ✅ Com type hints (`s: str -> str`) |
| gemma4:12b-it-qat | 10 | ✅ Limpo e correto |
| gpt-oss:20b | 10 | ✅ Limpo e correto |
| devstral-small-2:24b | 10 | ✅ Correto |
| ministral-3:8b | 10 | ✅ Correto (list comprehension) |
| qwen3-coder:30b | 10 | ✅ Correto e conciso |
| qwen3.5:9b | 0 | ❌ Vazio |
| qwen3.5:4b | 0 | ❌ Vazio |

### Velocidade (tokens/segundo)
| Modelo | Tok/s | Tamanho |
|--------|:-----:|:-------:|
| qwen3.5:4b | 72.7 | 4B ⚡ |
| qwen3.5:9b | 46.3 | 9B |
| gpt-oss:20b | 40.4 | 20B |
| ornith:9b | 37.6 | 9B |
| gemma4:12b-it-qat | 33.4 | 12B |
| ministral-3:8b | 26.8 | 8B |
| qwen3-coder:30b | 15.1 | 30B |
| devstral-small-2:24b | 8.0 | 24B 🐢 |

## Recomendações

| Uso | Modelo recomendado | Por quê |
|-----|-------------------|---------|
| **Dia a dia (chat + código)** | ornith:9b | Melhor equilíbrio qualidade/velocidade, 9B leve |
| **Máxima qualidade** | gemma4:12b-it-qat | QAT otimizado, visão, thinking |
| **Código pesado** | qwen3-coder:30b | Especialista em código, mas lento |
| **Rápido + bom** | gpt-oss:20b | 40 tok/s com qualidade máxima |
| **Com visão** | gemma4:12b-it-qat ou ministral-3:8b | Ambos suportam imagem |
| **Evitar** | qwen3.5:9b/4b (via Ollama) | Thinking consome tokens demais com max_tokens baixo |

## Notas
- **qwen3.5:9b/4b**: O resultado ruim é enganoso — são modelos "thinking" que gastam os 512 tokens de `max_tokens` em raciocínio interno antes de gerar a resposta visível. Com `max_tokens` maior (2048+), provavelmente responderiam corretamente. Não é falta de capacidade, é limitação do teste.
- **TurboQuant** estava offline durante o teste — modelos exclusivos dele não foram avaliados.
- **devstral-small-2:24b** é preciso mas muito lento (8 tok/s) — 24B params pesa na GPU.
