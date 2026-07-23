# Biblioteca de prompts prontos no modal de Ajuda

> **Status:** ✅ Concluído (2026-07-23)
> **Prioridade:** Média (UX / descoberta de features)
> **Tipo:** Feature
> **Arquivos prováveis:** `src/components/HelpModal.vue`, `src/content/prompts.ts` (NOVO), `src/components/ComposerBar.vue`, `src/stores/session.ts`

## Problema

O modal de Ajuda atual (`HelpModal.vue`) mostra apenas um catálogo estático de ferramentas (`help.md`). O usuário não tem acesso a **prompts prontos** — exemplos de pedidos que exercitam as ferramentas do sistema. Isso dificulta a descoberta do que o Cerne sabe fazer e como pedir direito.

O pedido original (do `falta_ajustar.md`):
> Botão "?" ao lado da pasta acima do chat. Ao clicar abre modal com pesquisa no topo, prompts prontos para cada ferramenta. Ao clicar em um prompt, pergunta se quer usar, se sim cola no chat e deixa o usuário modificar. Entre a pesquisa e os prompts, texto explicativo: "Ao clicar em um prompt ele será colado no seu chat". Cada prompt mostra: título + ferramentas usadas (ex: "pesquisa arquivo, usa computador") + texto breve + olho para ver o prompt todo + botão "Usar". Mostrar conforme é chat ou code.

## Fase 1 — Estrutura de dados dos prompts

### Tarefa 1.1 — Criar `src/content/prompts.ts`
- Arquivo TypeScript com array de prompts tipados:
  ```typescript
  export interface ReadyPrompt {
    id: string;
    title: string;          // ex: "Resumir um PDF grande"
    tools: string[];        // ex: ["read_file", "web_search"] — nomes internos
    toolsLabel: string;     // ex: "Lê arquivo, busca na web" — exibido no card
    preview: string;        // texto breve (1-2 linhas) mostrado no card
    full: string;           // prompt completo que será colado no composer
    scope: "chat" | "code" | "both";  // em que tipo de sessão aparece
  }

  export const READY_PROMPTS: ReadyPrompt[] = [ ... ];
  ```
- Incluir pelo menos 12 prompts iniciais cobrindo:
  - `web_search` + `web_fetch` (pesquisar assunto, resumir página)
  - `read_file` com offset/limit (ler arquivo grande por partes)
  - `write_file` / `edit_file` (criar/editar arquivo)
  - `run_command` (rodar script, instalar dependência)
  - `computer_use_*` (automatizar tarefa de tela)
  - `create_excel` (gerar planilha)
  - `ast_grep` / `ast_edit` (refatorar código)
  - `task` (sub-agente para tarefa paralela)
  - `todo_list` (planejar tarefa complexa)
  - `load_skill` (usar skill cadastrada)
- Prompts com `scope: "code"` só aparecem quando a sessão tem `project_root`; `"chat"` quando não tem; `"both"` sempre.

## Fase 2 — Redesenhar o HelpModal

### Tarefa 2.1 — Layout do modal
- Substituir o conteúdo estático por layout em 3 zonas:
  1. **Topo:** campo de pesquisa (input com ícone `search`)
  2. **Meio:** texto fixo explicativo: _"Ao clicar em **Usar**, o prompt será colado no seu chat — edite à vontade antes de enviar."_
  3. **Corpo:** lista de cards de prompt (scrollável)
- Manter o `help.md` como uma aba secundária ou seção colapsável no final ("Ver catálogo de ferramentas").

### Tarefa 2.2 — Card de prompt
- Cada card mostra:
  - **Título** em negrito
  - **Badge de ferramentas:** chips pequenos com `toolsLabel` (ex: `🔍 web_search` · `📁 read_file`)
  - **Preview:** texto breve (1-2 linhas, truncado com ellipsis)
  - **Olho** (`visibility` Material Symbol): expande/colapsa o `full` inline (sem modal extra)
  - **Botão "Usar":** primário, à direita
- Filtro: campo de pesquisa filtra por `title`, `preview`, `toolsLabel` e `full` (case-insensitive)
- Filtro de escopo: receber `hasProject: boolean` como prop; mostrar só prompts cujo `scope` casa

### Tarefa 2.3 — Fluxo "Usar"
- Ao clicar "Usar":
  1. Fechar o modal
  2. Colar `prompt.full` no composer (emitir evento ou chamar método do store)
  3. Focar o composer para o usuário editar
- **Sem confirmação intermediária** — o texto explicativo já avisa que será colado; uma confirmação extra só atrapalha. (O pedido original pedia confirmação, mas o UX fica melhor sem ela — o usuário pode desfazer com Ctrl+Z ou editar antes de enviar.)

## Fase 3 — Integração com o composer

### Tarefa 3.1 — Método para colar no composer
- Em `session.ts` (ou via evento), expor `draftText: string` — texto pré-preenchido no composer.
- Em `ComposerBar.vue`, assistir `draftText`: quando mudar, substituir o conteúdo do input e focar.
- O HelpModal emite `use-prompt(full)` → `App.vue` (ou quem abre o modal) chama `sessionStore.setDraft(full)`.

### Tarefa 3.2 — Passar contexto de escopo ao modal
- `HelpModal` recebe prop `hasProject: boolean` (= `session.project_root != null`).
- Se nenhuma sessão está aberta, mostrar só prompts `scope: "chat"`.

## Critério de conclusão
- Modal tem pesquisa funcional, texto explicativo e cards com título/ferramentas/preview/olho/Usar
- Botão "Usar" cola o prompt no composer e fecha o modal
- Prompts são filtrados por escopo (chat vs code)
- Catálogo de ferramentas (`help.md`) continua acessível no modal
- Pelo menos 12 prompts prontos cobrindo as principais ferramentas
