# Criação de documentos formatados (Excel, Word, PowerPoint)

> **Prioridade:** Média (feature complexa, dividir em fases)
> **Tipo:** Feature
> **Arquivos prováveis:** `src-tauri/src/agent/tools.rs`, `src-tauri/src/docgen.rs` (NOVO), `Cargo.toml`

## Problema

O usuário quer poder pedir ao agente: "crie um Excel com estas colunas", "gere um Word formatado com título e tabela", "crie um PowerPoint com 3 slides". Hoje o agente só consegue criar arquivos de texto plano (.md, .txt, .csv).

## Avaliação de complexidade

| Formato | Crate Rust | Maturidade | Complexidade |
|---------|-----------|:----------:|:------------:|
| Excel (.xlsx) | `rust_xlsxwriter` | ✅ Boa | Média |
| Word (.docx) | `docx-rs` | ⚠️ Básica | Alta |
| PowerPoint (.pptx) | Sem crate madura | ❌ | Muito Alta |
| PDF (saída) | `printpdf` / `genpdf` | ✅ Boa | Média |

## Fase 1 — Excel (.xlsx) com formatação

### Tarefa 1.1 — Adicionar crate `rust_xlsxwriter`
- Adicionar `rust_xlsxwriter` ao `Cargo.toml`
- Criar módulo `src-tauri/src/docgen.rs` com funções de geração

### Tarefa 1.2 — Tool `create_excel`
- Tool spec:
  ```json
  {
    "name": "create_excel",
    "parameters": {
      "path": "string (caminho do arquivo .xlsx)",
      "sheets": [{
        "name": "string",
        "headers": ["string"],
        "rows": [["valor1", "valor2"]],
        "column_widths": [15, 20],
        "freeze_header": true,
        "auto_filter": true
      }]
    }
  }
  ```
- Suportar: múltiplas abas, headers com negrito, largura de colunas, freeze pane, auto filter
- Formatação de células: negrito, itálico, cor de fundo, bordas, formato de número/data
- Escrever direto no disco (não precisa de sandbox — é criação, não edição)

### Tarefa 1.3 — Tool `read_excel`
- Ler planilha existente e retornar como markdown (já existe via `calamine` no `attachments.rs`)
- Expor como tool para o LLM poder ler planilhas do projeto

## Fase 2 — Word (.docx) com formatação

### Tarefa 2.1 — Avaliar crate `docx-rs`
- Testar se `docx-rs` suporta: títulos, parágrafos, tabelas, negrito/itálico, listas, imagens
- Se for muito limitado, avaliar alternativa: gerar OOXML manualmente (é ZIP + XML)

### Tarefa 2.2 — Tool `create_word`
- Tool spec com estrutura de documento:
  ```json
  {
    "name": "create_word",
    "parameters": {
      "path": "string",
      "elements": [
        {"type": "heading", "level": 1, "text": "Título"},
        {"type": "paragraph", "text": "Corpo do texto...", "bold": false},
        {"type": "table", "headers": [...], "rows": [...]},
        {"type": "list", "items": ["item1", "item2"]}
      ]
    }
  }
  ```

### Tarefa 2.3 — Tool `read_word`
- Já existe via `docx-rust` no `attachments.rs`
- Expor como tool

## Fase 3 — PowerPoint (.pptx)

### Tarefa 3.1 — Gerar PPTX via OOXML manual
- Não existe crate Rust madura para PPTX
- PPTX é um ZIP com XML (OOXML) — possível gerar manualmente
- Estrutura mínima: `[Content_Types].xml`, `_rels/.rels`, `ppt/presentation.xml`, `ppt/slides/slide1.xml`
- Começar com slides de texto simples (título + corpo)

### Tarefa 3.2 — Tool `create_powerpoint`
- Tool spec com slides:
  ```json
  {
    "name": "create_powerpoint",
    "parameters": {
      "path": "string",
      "slides": [
        {"layout": "title", "title": "Título", "subtitle": "Subtítulo"},
        {"layout": "content", "title": "Slide 2", "body": "Texto..."},
        {"layout": "two_column", "title": "Comparação", "left": "...", "right": "..."}
      ]
    }
  }
  ```

### Tarefa 3.3 — Layouts avançados (futuro)
- Tabelas, imagens, gráficos
- Temas e cores customizadas

## Fase 4 — PDF (saída)

### Tarefa 4.1 — Tool `create_pdf`
- Usar `genpdf` ou `printpdf` para gerar PDF a partir de markdown
- Suportar: títulos, parágrafos, tabelas, código formatado
- Útil para relatórios e documentos que não precisam ser editados depois

## Ordem recomendada
1. **Fase 1** (Excel) — mais útil e crate mais madura
2. **Fase 2** (Word) — útil mas crate limitada
3. **Fase 4** (PDF) — útil para relatórios
4. **Fase 3** (PowerPoint) — mais complexo, deixar por último

## Critério de conclusão (por fase)
- Fase 1: LLM consegue criar .xlsx com múltiplas abas, headers formatados, e dados
- Fase 2: LLM consegue criar .docx com títulos, parágrafos e tabelas
- Fase 3: LLM consegue criar .pptx com slides de texto
- Fase 4: LLM consegue criar .pdf a partir de conteúdo estruturado
