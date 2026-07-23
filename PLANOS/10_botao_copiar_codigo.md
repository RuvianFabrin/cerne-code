# Botão de copiar em blocos de código

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Alta (UX básica)
> **Tipo:** Feature
> **Arquivos prováveis:** `src/components/MarkdownContent.vue`

## Problema

Blocos de código renderizados no chat (via `MarkdownContent.vue`) não têm botão de copiar. O usuário precisa selecionar o texto manualmente, o que é especialmente ruim em blocos longos (scripts PowerShell, código, etc.) — a seleção pode incluir ou excluir linhas indesejadas.

## Solução

Adicionar um botão pequeno no canto superior direito de cada `<pre>` (bloco de código) que copia o conteúdo para a clipboard com um clique.

## Fase 1 — Botão de copiar

### Tarefa 1.1 — Pós-processar blocos de código após render do markdown
- No `MarkdownContent.vue`, após o markdown ser renderizado (via `v-html` ou `marked`), usar `nextTick` + `querySelectorAll("pre")` para injetar um botão em cada `<pre>`:
  ```html
  <button class="code-copy-btn" title="Copiar">
    <span class="msi">content_copy</span>
  </button>
  ```
- Alternativa mais limpa: usar um renderer customizado no `marked` (ou `markdown-it`) que já gera o botão no HTML do `<pre>`. Preferir esta abordagem se o parser usado suportar custom renderers facilmente.

### Tarefa 1.2 — Handler de clique
- Ao clicar no botão:
  1. Ler o texto do `<code>` filho do `<pre>` (`pre.querySelector("code")?.textContent`)
  2. `navigator.clipboard.writeText(text)`
  3. Trocar o ícone para `check` (✅) por 1.5s, depois voltar para `content_copy`
- Usar event delegation no container `.markdown-body` em vez de adicionar listener em cada botão (mais eficiente, funciona com conteúdo dinâmico).

### Tarefa 1.3 — Estilo do botão
- Posição: `position: absolute; top: 6px; right: 6px` dentro do `<pre>` (que precisa de `position: relative`)
- Aparência: fundo semi-transparente (`rgba(0,0,0,0.05)` light / `rgba(255,255,255,0.1)` dark), borda arredondada, ícone 14px
- Visibilidade: `opacity: 0` por padrão, `opacity: 1` no `:hover` do `<pre>` (não atrapalha a leitura)
- Transição suave: `transition: opacity 0.15s`

## Critério de conclusão
- Todo bloco de código (`<pre>`) tem botão de copiar no canto superior direito
- Botão aparece no hover do bloco
- Clique copia o conteúdo e mostra feedback visual (✅) por 1.5s
- Funciona em ambos os temas (light/dark)
