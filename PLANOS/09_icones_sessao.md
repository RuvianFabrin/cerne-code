# Ícones de sessão: distinguir chat de code/agente

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Baixa (UX visual)
> **Tipo:** Feature
> **Arquivos prováveis:** `src-tauri/src/models.rs`, `src-tauri/src/sessions.rs`, `src-tauri/src/lib.rs`, `src/components/Sidebar.vue`, `src/components/NewSessionDialog.vue`, `src/api.ts`, `src/stores/session.ts`

## Problema

Todas as sessões na Sidebar mostram o mesmo ícone `chat_bubble`, independentemente do tipo de uso. O usuário quer distinguir visualmente sessões de **chat** (conversa, perguntas, sem projeto) de sessões de **code/agente** (com pasta de projeto aberta, execução de ferramentas, automação).

Pedido original (`falta_ajustar.md`):
> Nos ícones da lista de sessão, identifique com ícone de "code/agente" e deixe o ícone atual para chat.

## Decisão de design

O tipo da sessão é **derivado**, não escolhido pelo usuário:
- Sessão **sem** `project_root` → tipo `chat` → ícone `chat_bubble`
- Sessão **com** `project_root` → tipo `code` → ícone `terminal` (Material Symbol)

Não é necessário adicionar campo novo ao `Session` nem mudar o backend — o `project_root` já existe e é a fonte de verdade. A distinção é puramente visual no frontend.

> **Alternativa considerada:** adicionar `session_type: "chat" | "code"` ao `Session` e deixar o usuário escolher na criação. Descartada porque duplica informação que já existe em `project_root` e cria estado inconsistente (sessão "code" sem pasta, ou "chat" com pasta).

## Fase 1 — Ícone dinâmico na Sidebar

### Tarefa 1.1 — Trocar ícone fixo por condicional
- Em `Sidebar.vue`, substituir:
  ```html
  <span class="msi">chat_bubble</span>
  ```
  por:
  ```html
  <span class="msi">{{ s.project_root ? "terminal" : "chat_bubble" }}</span>
  ```
- O tipo `Session` no frontend (`api.ts`) já deve ter `project_root: string | null` — confirmar e adicionar se faltar.

### Tarefa 1.2 — Tooltip no ícone
- Adicionar `v-tooltip` no ícone:
  - `project_root` presente → `"Sessão de código — {caminho do projeto}"`
  - Sem `project_root` → `"Chat"`

### Tarefa 1.3 — Cor sutil diferenciada (opcional)
- Ícone `terminal` com cor levemente diferente (ex: `color: var(--cerne-accent)`) para reforçar a distinção sem poluir.
- Ícone `chat_bubble` mantém a cor atual.

## Fase 2 — Filtro por tipo na Sidebar (opcional, futuro)

### Tarefa 2.1 — Chips de filtro
- Abaixo da busca, dois chips: `Todos` · `💬 Chat` · `⌨️ Code`
- Filtra a lista de sessões por tipo
- Default: `Todos`

> Esta fase é opcional e só faz sentido quando o usuário tiver muitas sessões. Não bloqueia a Fase 1.

## Critério de conclusão
- Sessões com projeto aberto mostram ícone `terminal`
- Sessões sem projeto mostram ícone `chat_bubble`
- Tooltip indica o tipo e o caminho do projeto
- Nenhuma mudança no backend necessária
