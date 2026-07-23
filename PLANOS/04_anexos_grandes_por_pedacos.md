# Anexos grandes: salvar como .md e ler por pedaços

> **Prioridade:** Alta (economia de tokens + suporte a arquivos grandes)
> **Tipo:** Otimização / Feature
> **Arquivos prováveis:** `src-tauri/src/attachments.rs`, `src-tauri/src/agent/tools.rs`, `src-tauri/src/agent/mod.rs`, `src/stores/session.ts`

## Problema

Hoje quando o usuário anexa um PDF ou Word grande, o texto extraído é enviado **inteiro** como parte da mensagem do usuário. Um PDF de 50 páginas pode gerar 200K+ caracteres = ~50K tokens, estourando o contexto ou desperdiçando janela de contexto.

## Solução proposta

Em vez de embutir o texto do anexo na mensagem, salvar o texto extraído como um arquivo `.md` na pasta da sessão e indicar ao LLM que ele pode ler por pedaços usando `read_file` com `offset`/`limit`.

## Fase 1 — Backend: salvar anexo como arquivo na sessão

### Tarefa 1.1 — Criar pasta de anexos na sessão
- Em `sessions.rs`, ao criar sessão, criar subpasta `attachments/` dentro do diretório da sessão
- Caminho: `{app_data_dir}/sessions/{session_id}/attachments/`

### Tarefa 1.2 — Modificar extração de anexos
- Em `attachments.rs`, após extrair texto de PDF/Word/Excel, salvar o resultado como `{app_data_dir}/sessions/{session_id}/attachments/{nome_original}.md`
- O arquivo `.md` deve ter um header com metadata:
  ```markdown
  ---
  source: "Versão Final SCA 0707.pdf"
  pages: 7
  extracted_chars: 45230
  extracted_at: 2026-07-22T20:00:00Z
  ---

  # Conteúdo extraído de Versão Final SCA 0707.pdf

  [texto extraído aqui]
  ```

### Tarefa 1.3 — Modificar mensagem do usuário
- Em vez de embutir o texto inteiro no `content` da mensagem, embutir apenas um resumo:
  ```
  📎 Anexo: Versão Final SCA 0707.pdf (7 páginas, 45.230 caracteres)
  Arquivo salvo em: attachments/Versão_Final_SCA_0707.md
  Use read_file(path="attachments/Versão_Final_SCA_0707.md", offset=0, limit=200) para ler por partes.
  ```
- O `display_content` (mostrado no chat) mostra o resumo com ícone
- O `content` (enviado ao LLM) inclui o resumo + instrução de como ler

### Tarefa 1.4 — Registrar anexo como extra_read_path
- Adicionar a pasta `attachments/` da sessão como `extra_read_paths` automaticamente
- Assim o `read_file` consegue ler os anexos sem o usuário precisar configurar nada

## Fase 2 — Planilhas (Excel/CSV)

### Tarefa 2.1 — Extrair planilhas como .md estruturado
- Para `.xlsx`/`.xls`/`.csv`, extrair cada aba como uma seção markdown com tabela
- Formato:
  ```markdown
  ## Aba: Planilha1
  | Coluna A | Coluna B | Coluna C |
  |----------|----------|----------|
  | valor1   | valor2   | valor3   |
  ```
- Se a planilha tiver muitas linhas (>500), salvar apenas as primeiras 500 + metadata com total
- O LLM pode usar `read_file` com offset/limit para ler o resto

### Tarefa 2.2 — Suporte a fórmulas (futuro)
- Avaliar se a crate `calamine` consegue extrair fórmulas (não só valores)
- Se sim, incluir coluna de fórmulas no markdown
- Se não, documentar limitação

## Fase 3 — Imagens

### Tarefa 3.1 — Manter imagens como data URL
- Imagens continuam sendo enviadas como data URL base64 (necessário para visão do modelo)
- Não há como "ler por pedaços" uma imagem — ela precisa ser enviada inteira
- Mas limitar o tamanho máximo da imagem (redimensionar se > 2048px) para economizar tokens de visão

## Critério de conclusão
- PDF de 50 páginas não estoura o contexto
- LLM consegue ler o anexo por pedaços via read_file
- Usuário vê no chat um resumo do anexo (não o texto inteiro)
- Planilhas são extraídas como tabelas markdown legíveis
