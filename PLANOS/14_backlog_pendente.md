# Backlog Pendente — Cerne Code

> Arquivo de DETALHE TÉCNICO — decisões, arquivos/funções exatos, bugs reais encontrados e como
> foram corrigidos, escopo reduzido e por quê. Não é a lista de trabalho do dia a dia — pra isso,
> ver as 3 listas enxutas: `PLANOS/aFazer.md` (pedido, ainda sem código), `PLANOS/Testar.md`
> (implementado, esperando você confirmar) e `PLANOS/Feito.md` (implementado E confirmado). Este
> arquivo é onde cada uma delas aponta quando você quer o histórico completo de como algo foi
> construído. Atualizado em cada sessão de desenvolvimento.

---

## Implementadas nesta sessão (para teste)

| # | Tarefa | Status |
|---|--------|--------|
| T1 | AskCard com renderização Markdown | ✅ APROVADO |
| T2 | Default Manual + Pensamento Desligado | ✅ APROVADO |
| T3 | Esconder janela CMD ao executar comandos | ✅ APROVADO |
| T4 | Seletor de MCP no Composer | ✅ APROVADO |
| T5 | Ajuda atualizada com novas funcionalidades | ✅ APROVADO |
| T6 | Nova sessão sem pasta obrigatória | ✅ APROVADO |
| T7 | Pastas com modo leitura vs leitura+escrita | ✅ APROVADO |
| T8 | Caminho colado no composer vira pasta (pergunta modo) | ✅ APROVADO |
| T9 | Auto-nomear sessão via LLM na primeira mensagem | ✅ APROVADO |
| T10 | Ícone da sessão muda conforme modo (chat→code) | ✅ APROVADO |
| T15 | Remover nudge/TAREFA_CONCLUIDA/Continuando automaticamente | ✅ APROVADO |
| T18 | Nova sessão sem modal — criar direto ao clicar + | ✅ APROVADO |
| T19 | Sempre caminhos absolutos | ✅ APROVADO |
| T20 | Caminho completo nos diffs | ✅ APROVADO |
| T21 | Modal de disclaimer na primeira abertura (botão libera em 3s) | ✅ APROVADO |
| T22 | Seção Sobre com créditos das ferramentas usadas | ✅ APROVADO |
| T23 | Botão de pasta 📁 no topo do composer (sempre visível) | ✅ APROVADO (corrigido BUG: pasta extra sem project_root agora permite edição) |
| Shell | Detecção automática: pwsh7 > powershell5 > cmd (Windows) | ✅ APROVADO |
| T24 | Personas: prompts prontos cadastráveis como system prompt selecionável | ✅ IMPLEMENTADO — 2026-08-14. Precursor leve da Fase A3 (agente nomeado); ver detalhe abaixo |
| T25 | Fix: passo de tool call ao vivo mostrava label bruto truncado (IN/OUT não atualizavam sem recarregar) | ✅ IMPLEMENTADO — 2026-08-14 |
| T26 | Fix: `llama-server` (e jobs em background) ficavam órfãos consumindo RAM/VRAM ao fechar o app | ✅ IMPLEMENTADO — 2026-08-14 |
| T27 | Fase A1 do roteiro: UUID de rastreamento de execução de agente/skill | ✅ IMPLEMENTADO — 2026-08-14 (backend only, sem UI ainda) |
| T28 | Fix + Fase A2: texto do sub-agente/verificador vazava no streaming do chat principal; canal sintético por execução | ✅ IMPLEMENTADO — 2026-08-14 |
| T30 | Fase A6: descrição de skill cortada no catálogo do system prompt + tool `read_skill_details` | ✅ IMPLEMENTADO — 2026-08-14 |
| T31 | Fase A5: modal batelado de aprovação de agentes/skills no modo Manual | ✅ IMPLEMENTADO — 2026-08-14 (reduzido: aprovar/recusar todos, sem "um a um") |
| T32 | Fase A4: `task`s em paralelo via API (join_all) quando 2+ no mesmo turno; local continua sequencial | ✅ IMPLEMENTADO — 2026-08-14 (sem toast pro aviso em Auto/YOLO ainda, sem teto configurável de paralelismo) |
| T33 | Fase F1: Configurações vira modal (Ajuda/Sobre já eram) | ✅ IMPLEMENTADO E TESTADO — 2026-08-14 (confirmado pelo usuário na janela real: modal abre por cima, rascunho do composer sobrevive, modal-sobre-modal funciona, maximizar funciona) |
| T34 | Fase C1: painel de jobs em segundo plano com push em tempo real + cancelar | ✅ IMPLEMENTADO — 2026-08-14 (`cargo test` não rodou nesta sessão — problema de ambiente local, não de código; ver detalhe) |
| T35 | Fase C2: painel de execuções de agente/skill (task/verify_completion) somente leitura em tempo real | ✅ IMPLEMENTADO — 2026-08-14 (sem cancelamento individual nem posição na fila — gaps documentados) |
| T36 | Fase B1: painel "Agentes & Skills" (skills + personas, busca, usar agora) | ✅ IMPLEMENTADO — 2026-08-15 (badge "em uso agora" só pra personas, gap documentado pra skills) |
| T37 | Fase D2: navegador de arquivos no composer (árvore lazy-load, insere caminho) | ✅ IMPLEMENTADO — 2026-08-15 |
| T38 | Fase D1: visualizador de diff agregado da sessão + trocar project_root | ✅ IMPLEMENTADO — 2026-08-15 (gap: não distingue aceito/rejeitado no histórico) |
| T47 | Fase B2: skills de exemplo (`weather`/`summarize`/`skill-creator`) semeadas na 1ª execução, adaptadas do picoClaw | ✅ IMPLEMENTADO — 2026-08-16 (reduzido: sem loader de 3 níveis, exemplos ficam editáveis/deletáveis como skill normal) |
| T48 | Fase A3: Persona ganha allowlist de ferramentas (`tools`) opcional | ✅ IMPLEMENTADO — 2026-08-16 (retrocompatível: vazio = sem restrição; `model` override avaliado e adiado por risco). Não testado na janela — feito sem build a pedido do usuário |
| T14 | Background jobs com callback automático | ✅ IMPLEMENTADO — 2026-08-16 (`cargo check --lib`/`--tests` limpos; `cargo test` não roda nesta máquina, ver nota de ambiente já documentada — não testado na janela a pedido do usuário). Ver detalhe abaixo |
| T29 | Pastas na lista de sessões (2 níveis, expansível/recolhível) | ✅ IMPLEMENTADO — 2026-08-16 (drag-and-drop NÃO implementado, mover é via botão "mover para" — ver detalhe abaixo; `cargo check`/`vue-tsc --noEmit` limpos, não testado na janela) |
| T16 | Ferramenta `create_pptx` (PowerPoint) | ✅ IMPLEMENTADO — 2026-08-16, com imagens (não só texto/tabela) — ver detalhe abaixo. `cargo check --lib`/`--tests` limpos; `cargo test` não roda nesta máquina (ambiente já documentado). Não testado abrindo o .pptx de verdade no PowerPoint/LibreOffice — ver ressalva no detalhe |
| T17 | IA cria ferramentas Python customizadas (`create_python_tool`/`update_python_tool`, gerenciadas via `uv`) | ✅ IMPLEMENTADO — 2026-08-16, ver detalhe abaixo. `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos; `cargo test` não roda nesta máquina. Requer `uv` instalado no PATH do usuário pra rodar de verdade — não verificado nesta sessão |

### Detalhe de T24-T26 (implementadas em 2026-08-14)

- **T24 — Personas**: `src-tauri/src/personas.rs` (CRUD + storage em `personas.json`), `Session.persona_id`, injeção no system prompt em `agent/mod.rs` (mesmo ponto do Fable), seletor no `ComposerBar.vue`, gestão em `Settings.vue` → seção Personas. Precursor simplificado da Fase A3 do roteiro (`13_roteiro_agentes_skills_fases.md`) — falta ainda tools/model/skills por agente, allowlist, e o formato `AGENT.md` completo.
- **T25 — Fix live tool step**: `agent:tool_call` agora manda `command`/`file_path` desde o início da chamada; `agent:tool_result` manda `detail`/`additions`/`deletions`/`duration_ms` ao terminar — sem precisar recarregar a sessão. `TaskStepGroup.vue` mostra "Processando..." enquanto falta o resultado. Aplicado também em `subagent.rs`/`verifier.rs` (mesmos eventos compartilhados).
- **T26 — Fix llama-server órfão**: `taskkill /PID /T /F` (já usado em `background.rs::stop`) extraído pra `agent/shell.rs::kill_pid_tree_blocking`, chamado num handler `tauri::RunEvent::ExitRequested` novo em `lib.rs::run` — mata `llama_children` e `background_jobs` de forma síncrona antes do processo do app sumir.
- **T27 — Fase A1 (UUID de execução)**: `models::AgentExecution` (id/parent_id/session_id/kind/name/status/timestamps), registro em memória `AppState.agent_executions`, helpers `start_agent_execution`/`finish_agent_execution` em `agent/mod.rs` chamados nos call sites de `task`/`verify_completion`. `subagent::run`/`verifier::run` ganharam parâmetro `execution_id` e propagam ele nos eventos `agent:tool_call`/`agent:tool_result` que emitem (campo novo `execution_id`, opcional). Comando `list_agent_executions` exposto (`api.listAgentExecutions()` no frontend, tipos `AgentExecution`/`ToolCallPayload.execution_id`/`ToolResultPayload.execution_id` em `api.ts`) — ainda sem consumidor de UI (isso é Fase B/C).
- **T28 — Fix vazamento de streaming + canal por execução (A2)**: achado ao implementar a A1 — `subagent::run`/`verifier::run` chamavam `providers::chat_stream` passando o `session_id` **real**, então o texto/thinking do sub-agente ou verificador ia pro `chat:token`/`chat:thinking_token` da sessão e se misturava com o streaming do agente principal na bolha do chat visível (bug real, não só teórico — confirmado lendo `stores/session.ts::onChatToken`, que só filtra por `session_id`, sem diferenciar origem). Corrigido usando o mesmo padrão do `maybe_compact`: canal sintético `{session_id}::exec::{execution_id}` em vez do `session_id` cru. De brinde, isso é exatamente o "canal de streaming próprio por execução" que a Fase A2 pedia — só falta um listener de UI que abra esse canal (Fase C).
- **T30 — Fase A6 (descrição concisa + "ler mais")**: `SKILL_CATALOG_DESC_MAX_CHARS = 200` em `agent/mod.rs`, aplicado via o `truncate()` (char-safe, já existente) na hora de montar o bloco "Skills disponíveis" do system prompt — se alguma descrição foi cortada, uma linha extra avisa o LLM que pode chamar a nova tool `read_skill_details(name)` (spec em `tools.rs`, dispatch em `agent/mod.rs` igual `load_skill`) pra ler a descrição inteira sem comprometer a chamada (`load_skill` continua sendo o "carregar pra usar"). Rótulo amigável adicionado em `taskLabels.ts` + 4 locales.
- **T31 — Fase A5 (modal batelado)**: hoje `task`/`load_skill`/`verify_completion` já pausavam individualmente no modo Manual via `request_permission` (mesmo mecanismo de qualquer tool call) — o que faltava era agrupar quando o modelo planeja usar mais de um no mesmo turno. Backend: `AppState.pending_agent_plans` (canal oneshot, mesmo padrão de `pending_permissions`), `agent::request_agents_skills_plan` (emite `agent:agents_skills_plan` com a lista de itens, espera uma resposta só), comando `answer_agents_skills_plan`. No loop de `run_turn`, antes de processar as tool calls do turno: se Manual e há `task`/`load_skill`/`verify_completion` na lista, pergunta batelado primeiro; o resultado fica num `HashMap<id, bool>` consultado na hora do `approved` de cada chamada, pra não perguntar de novo individualmente pelas que já foram decididas. Frontend: `AgentsSkillsPlanCard.vue` (lista os itens com ícone por tipo + nome/descrição), listener `onAgentsSkillsPlan`/estado `pendingAgentsSkillsPlan` na store, renderizado em `ChatView.vue` antes do `PermissionCard`. **Reduzido** do desenho original: só aprova/recusa tudo de uma vez, sem granularidade "um a um" (documentado como próxima iteração no comentário do código, se algum dia fizer falta).
- **T32 — Fase A4 (fila local vs. paralelo API)**: local não precisou de mudança nenhuma — o loop de `run_turn` já roda cada turno numa única cadeia sequencial de `.await`s, então "fila de 1 por vez" já era o comportamento por construção. Pra API: antes do loop sequencial de tool calls, `agent/mod.rs` calcula `task_call_ids` (todas as chamadas `task` do turno) e `parallel_eligible = !cfg.kind.is_local() && task_call_ids.len() >= 2` (novo `ProviderKind::is_local()` em `models.rs`). Quando elegível, monta um `Vec` de futures (`async move` com clones locais de `app`/`cfg`/`api_key`/`session.model`/`extra_paths` — mesmo padrão já usado no auto-naming de sessão em paralelo) chamando `subagent::run` pra cada `task` aprovada, e roda todas via `futures_util::future::join_all` (continuam na mesma tokio task — sem `tokio::spawn`, então sem precisar de bounds `'static`/`Send`, só clones locais). Resultado guardado em `precomputed_task_results: HashMap<call_id, Result<String>>`; o loop sequencial que vem depois, ao chegar numa chamada `task`, checa esse mapa primeiro (`precomputed_task_results.remove(&call.id)`) antes de cair no caminho antigo (sequencial, ainda o único caminho pra provider local ou pra um `task` isolado). Aviso: no modo Manual, o `AgentsSkillsPlanCard.vue` (T31) ganhou uma nota extra (`AgentsSkillsPlan.parallel`) quando o plano vai rodar em paralelo; em Auto/YOLO (sem modal nenhum hoje) emite `agent:parallel_execution_info` — sem componente de toast consumindo esse evento ainda (fica pra quando o Cerne tiver um sistema de notificação genérico). Também não tem teto configurável de paralelismo (`max_parallel_api_executions`) — todas as `task` elegíveis do turno disparam juntas; se algum dia um turno pedir muitas de uma vez isso pode valer a pena limitar.
- **T33 — Fase F1 (Settings → modal)**: `App.vue` removeu o `view: ref<"chat"|"settings">` que alternava `<ChatView>`/`<Settings>` via `v-if`/`v-else` — agora `<ChatView>` fica sempre montado e `<Settings v-model:visible="showSettings" />` é só mais um overlay, junto de `HelpModal`/`AboutModal`. `Sidebar.vue` perdeu a prop `view` (não faz mais sentido, não há "view" pra destacar) — o botão de Configurações virou `@click="emit('open-settings')"`, igual Ajuda/Sobre, sem estado "ativo" (nenhum dos três outros botões tinha). `Settings.vue` ganhou `defineProps<{ visible: boolean }>()`/`update:visible` e o template inteiro (que já era grande, ~750 linhas) foi envolvido num `<Dialog modal maximizable :style="{width:'860px'}">` do PrimeVue — mesmo padrão de `HelpModal.vue`, sem precisar tocar no conteúdo interno das seções. O `<h1>` duplicado com o título (que agora já aparece no header do Dialog) foi removido. **Benefício estrutural de brinde**: como `ChatView` não é mais desmontado ao abrir Configurações, o texto do composer não se perde mais — isso resolve o motivo original do pedido sem precisar de nenhuma lógica extra de "salvar rascunho". **Verificado**: testado pelo usuário na janela real (`npm run tauri dev`) — modal abre por cima sem trocar de tela, rascunho do composer sobrevive ao abrir/fechar Configurações, `SkillEditorModal`/`PersonaEditorModal` abrem corretamente por cima do modal de Settings (modal-sobre-modal), e o botão de maximizar funciona. Fase F fechada de verdade.
- **T34 — Fase C1 (painel de background tasks)**: antes o frontend não tinha NENHUM comando Tauri pra listar/parar jobs em segundo plano — só existiam como tool do LLM (`run_command(background=true)`/`check_background_output`/`stop_background`), invisíveis pra UI exceto indiretamente via `TaskItem`. Backend: `BackgroundJobs` ganhou um campo `app: Option<AppHandle>` (via novo construtor `BackgroundJobs::new(app_handle)`, chamado no `setup()` do `lib.rs::run` — `None` nos testes, que continuam usando `BackgroundJobs::default()`), `spawn_reader` passou a emitir `agent:background_output` a cada linha nova do processo, rate-limitado a 1 evento/200ms por job (`Instant` compartilhado entre os readers de stdout/stderr — sem isso um build verboso inundaria o frontend). Novos comandos `list_background_jobs` (versão estruturada de `list()`, com `BackgroundJobInfo{id,command,status,output}`) e `stop_background_job` (chama o `stop()` que já existia, com o fix de `taskkill /T /F` pra árvore de processo inteira). Frontend: `BackgroundJobsPanel.vue` — badge "N processos (M rodando)" que expande numa lista, cada linha abre um modal com o output completo atualizado ao vivo, botão de parar por job. Montado em `ChatView.vue` acima do composer, ao lado de `AgentsSkillsPlanCard`/`PermissionCard`/`AskCard`.
- **T35 — Fase C2 (sessão de agente/skill visualizável)**: nenhuma mudança de backend — a infra da A1 (`list_agent_executions`, `execution_id` nos eventos) e A2 (canal sintético `{session_id}::exec::{execution_id}` que `subagent::run`/`verifier::run` já usavam pra não vazar streaming no chat principal) já cobria tudo que essa tarefa precisava. `AgentExecutionsPanel.vue`: badge de contagem (filtra `list_agent_executions` pela sessão atual, com poll a cada 3s já que não há push de mudança de status), lista expansível, e um modal por execução que assina `chat:token`/`chat:thinking_token` (comparando `session_id` do evento com o canal sintético) e `agent:tool_call`/`agent:tool_result` (comparando `execution_id`) pra mostrar texto e passos em tempo real, só leitura. **Dois gaps documentados no código e no roteiro**: (1) sem cancelamento individual por execução — hoje `task`/`verify_completion` não têm handle de abort próprio, só dá pra cancelar o turno inteiro; (2) sem posição na fila — o registro marca `running` assim que começa, não existe estado "na fila" rastreável (a fila local é só a ordem implícita do loop sequencial). Ambos ficariam do tamanho da A4 pra implementar direito — deixados como próxima iteração em vez de meio-implementados.
- **T36 — Fase B1 (painel Agentes & Skills)**: `AgentsSkillsPanel.vue`, aberto via novo botão "Agentes & Skills" na barra lateral (`Sidebar.vue` → `open-agents-skills` → `App.vue`, mesmo padrão modal da Fase F). Duas abas — Skills (`list_skills`) e Personas (`list_personas`) — com busca por nome/descrição. Cada linha tem "Editar" (reabre `SkillEditorModal`/`PersonaEditorModal` já existentes) e "Usar agora": pra Persona é imediato (`sessionStore.updatePersona`); pra Skill não dá pra forçar o `load_skill` direto (quem decide é o LLM dentro do turno), então prepara uma mensagem no composer via `sessionStore.setDraft` (mecanismo que já existia, usado pelos `READY_PROMPTS`) pedindo pra usar aquela skill, e o usuário revisa/envia. Nenhuma mudança de backend — tudo reaproveita comandos já existentes. **Gap documentado**: badge "em uso agora" só funciona pra Personas (comparação direta com `session.persona_id`); Skills não têm rastreamento de uso porque `load_skill` não gera um `AgentExecution` (só `task`/`verify_completion` geram, e sem nome de skill associado).
- **T37 — Fase D2 (navegador de arquivos)**: `list_dir_entries(path)` novo comando Tauri em `lib.rs`, não-recursivo de propósito (1 nível por chamada, carregado sob demanda) — mesmo espírito de leitura livre da tool `list_dir` do agente, sem gating extra de `extra_read_paths` (quem navega aqui é o usuário via clique, não o LLM). Frontend: `FileTreeNode.vue` (nó recursivo — usa a auto-referência de componente do Vue 3 SFC, sem precisar registrar manualmente) faz lazy-load dos filhos só quando expandido; `FileBrowser.vue` é o modal com seletor de pasta raiz (reaproveita o diálogo nativo já usado em `ExtraReadPaths.vue`) + a árvore. Botão novo no `ComposerBar.vue` (ícone 📁 ao lado de `ExtraReadPaths`) abre o modal com `project_root` da sessão como raiz padrão. Clicar num arquivo, ou no ícone de "inserir" numa pasta, usa `sessionStore.setDraft` (mesmo mecanismo do painel B1) pra colocar o caminho absoluto no composer.
- **T38 — Fase D1 (diff agregado + trocar pasta)**: backend precisou só de um comando novo — `update_session_project_root` (`sessions::update_project_root` + Tauri command), porque não existia NENHUM jeito de trocar a pasta de trabalho de uma sessão já criada (só na criação). A agregação de diff em si não precisou de backend: `RepoDiffViewer.vue` reaproveita `sessionStore.tasks` (já carregado) filtrando `write_file`/`edit_file`/`ast_edit` com diff no `detail`, agrupa por `file_path`, soma `additions`/`deletions`. `splitDiffDetail`/`diffLines` (antes só dentro de `TaskStepGroup.vue`) foram extraídos pra `src/diffUtils.ts`, módulo compartilhado entre os dois componentes. UI: lista de arquivos à esquerda (com badge "pendente" quando o arquivo ainda está em `sessionStore.pendingEdits`) + diff colorido à direita, botão "Trocar pasta" reaproveitando o diálogo nativo já usado em `FileBrowser.vue`/`ExtraReadPaths.vue`. Acessível por um novo botão (ícone `difference`) no composer, ao lado do navegador de arquivos.
  - **Gap conhecido**: `cargo test --lib` não conseguiu RODAR nesta sessão — o binário de teste falha ao iniciar com `STATUS_ENTRYPOINT_NOT_FOUND`, persistente mesmo após `cargo clean -p cerne --profile dev` completo (removeu 41GB e refez tudo do zero, mesmo erro). `cargo check --lib` e `cargo check --tests` (que type-checka o código de teste, incluindo os usos de `BackgroundJobs::default()`/`self.jobs` renomeado, sem precisar linkar/rodar o binário) passaram 100% limpos, então a lógica está correta — o problema parece ser do ambiente Windows local (suspeita: antivírus interferindo com o binário recém-compilado, um padrão conhecido). Recomendo rodar `cargo test` de novo depois, fora desta sessão, pra confirmar os 134 testes + as mudanças em `background.rs` (nenhum teste novo foi adicionado pra esta feature especificamente, já que ela depende de emissão de evento Tauri real, difícil de testar em unit test sem um app mockado).
- **T48 — Fase A3 (Persona ganha allowlist de ferramentas)**: `Persona` (`personas.rs`) ganhou campo `tools: Vec<String>` com `#[serde(default)]` — personas salvas antes desse campo existir continuam carregando normalmente com `tools` vazio (testado explicitamente: `tools_allowlist_roundtrips_and_defaults_empty_for_old_data`, escreve um JSON sem a chave `tools` e confere que carrega sem erro). `create_persona`/`update_persona` (backend + comando Tauri + `api.ts`) ganharam o parâmetro. Em `agent/mod.rs`, depois de montar `tool_specs` (always+project+MCP), se a persona ativa da sessão tem `tools` não-vazio, filtra o toolset pra só essas ferramentas — `ask` sempre fica disponível (sem isso o modelo pode ficar travado sem como pedir esclarecimento). Frontend: `PersonaEditorModal.vue` ganhou uma grade de checkboxes com um subconjunto curado de ferramentas (arquivo/busca/comando/sub-agente — fora `computer_use_*`/MCP, dinâmicos demais pra um checkbox fixo).
  - **Decisão consciente de escopo**: NÃO implementei `model` (override de modelo por persona), que também fazia parte do `AgentFrontmatter` original do roteiro. Risco avaliado como alto demais pra mexer sem poder testar na janela (o usuário pediu pra não gerar build até terminarmos essa rodada) — trocar o modelo no meio do fluxo de uma sessão existente tem chance real de mandar um modelo que não existe nesse provider, ou incompatível com o `reasoning_effort`/context window já configurado, e isso quebraria silenciosamente até alguém notar. Fica pra uma iteração com mais cautela/teste ao vivo.
- **B3 — Fase B3 (usuário cria seus próprios agentes)**: em vez de montar um sistema paralelo (`AgentEditorModal.vue` + formato `AGENT.md` + `agents.rs` novo do zero), completei a mesma decisão pragmática já tomada desde o T24 ("Persona é a versão reduzida de agente nomeado deste roteiro") — `Persona` ganhou `skills: Vec<String>` (`personas.rs`, `#[serde(default)]`, mesma semântica de `tools` do T48: vazio = sem restrição). Junto com `tools` (já existente), isso fecha as duas peças de A3/B3 que dava pra implementar sem risco: allowlist de ferramentas E de skills por agente.
  - Backend (`agent/mod.rs`): a persona ativa da sessão agora é carregada UMA vez, antes de montar o catálogo de skills do system prompt (antes era lida de novo, mais tarde, só pra injetar `## Perfil ativo` — unifiquei as duas leituras). Se `persona.skills` não é vazio, o catálogo mostrado ao modelo já vem filtrado pra só essas skills — e, por segurança (o modelo poderia ainda tentar chamar `load_skill` com um nome fora da lista, por exemplo se lembrar de uma skill de um turno anterior antes da persona mudar), o dispatch de `load_skill` também checa via `skill_allowed_for_persona()` e recusa com um erro claro se a skill pedida não estiver na allowlist. `read_skill_details` continua sem restrição de propósito (só mostra a descrição completa, não carrega/expõe o conteúdo pra seguir — mesma lógica de "ler é seguro, carregar é a ação restrita" já usada na Fase A6).
  - Frontend: `PersonaEditorModal.vue` ganhou uma segunda grade de checkboxes pro catálogo de skills — busca via `api.listSkills(projectRoot)` (skills globais + do projeto da sessão atual), diferente da grade de tools que usa uma lista curada fixa (`SELECTABLE_TOOLS`), já que skills são dinâmicas por natureza.
  - 4 testes novos: `skills_allowlist_roundtrips_and_defaults_empty_for_old_data` (`personas.rs`, espelha o teste equivalente de `tools` do T48) + 3 em `agent/mod.rs` pra `skill_allowed_for_persona` (sem persona ativa libera tudo; persona com allowlist restringe corretamente; persona com allowlist vazia libera tudo).
  - **Decisão consciente de escopo, reafirmada**: `model` (override de modelo por agente/persona) e `max_turns` continuam DELIBERADAMENTE de fora, pelo mesmo motivo já registrado no T48 — risco de mandar um modelo incompatível com o provider/contexto configurado sem poder testar ao vivo nesta rodada. `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos; `cargo test` não roda nesta máquina (mesmo problema de ambiente já documentado).
- **Fase E — Composer (E1-E5)**: as 5 tarefas do roteiro, na ordem implementada:
  - **E5 (prompt de código)**: já estava "majoritariamente feito" (botão de copiar, `10_botao_copiar_codigo.md`) — só faltava a instrução de prompt engineering. `SYSTEM_PROMPT` (`agent/mod.rs`) ganhou uma frase curta pedindo um bloco de código por comando de shell mostrado na resposta (facilita copiar isolado), exceto quando os comandos precisam rodar juntos em sequência de um único paste.
  - **E3 (MCPs)**: backend — `sessions::create_session` agora nasce com `enabled_mcp_servers: Some(vec![])` (nenhum MCP habilitado) em vez de `None` (= todos habilitados) — só afeta sessão criada a partir de agora, sessões antigas continuam com `None` = todos habilitados (retrocompatível, nenhuma migração necessária). Frontend: os botões individuais de toggle por servidor no rodapé do `ComposerBar.vue` (`mcp-toggle-btn`, um por servidor — poluía visualmente com muitos MCPs configurados) viraram um único botão "MCPs (N/M)" que abre um `Dialog` com checkboxes — mesma lógica de habilitar/desabilitar (`toggleMcpServer`) reaproveitada, só a apresentação mudou.
  - **E4 (ícone de visão)**: novo ícone dedicado no `composer-toolbar` (ao lado do `ProviderPicker`) com 3 estados visuais (não testado/cinza, suporta imagem/verde, não suporta/vermelho). Cache client-side em `localStorage` (`cerne-vision-support-cache`) chaveado por `provider::model::fork_ou_custom_provider_id` — **não** por sessão, pra não testar de novo à toa toda vez que o usuário troca de sessão com o MESMO modelo (evita uma chamada real ao provider/servidor local desnecessária). Clique no ícone força reteste (`retestVisionSupport`, ignora o cache).
  - **E2 (atalho `/`)**: `/` como PRIMEIRO caractere digitado numa caixa vazia abre um popover filtrável combinando skills, personas, MCPs e "ready prompts" (`content/prompts.ts`, já usados no `HelpModal.vue`) — fecha sozinho assim que aparece espaço/quebra de linha (deixa de ser atalho, vira texto normal digitado) ou quando uma opção é escolhida. Navegação por teclado completa: setas cima/baixo trocam o item ativo, Enter seleciona, Esc fecha o menu sem mexer no texto. Cada tipo de item tem uma ação diferente ao selecionar: skill → prepara o pedido no composer (mesmo texto do "usar agora" do painel B1); persona → ativa na sessão direto (`onPersonaChange`) e limpa o composer; MCP → liga/desliga (`toggleMcpServer`) e limpa o composer; ready prompt → insere o texto completo do prompt no composer. `@mousedown.prevent` no item evita que o `blur` do textarea feche o menu antes do clique registrar (padrão comum em dropdown-sobre-input).
  - **E1 (Fable é skill?)**: só investigação/decisão, sem mudança de código — já estava concluída no roteiro original ("não forçar a unificação", Fable e Skills resolvem problemas diferentes). A tarefa de UI condicional ("se aprovado, mover botão Fable pra um menu unificado") não foi implementada por falta de aprovação explícita — fica como estava, cada um no seu lugar de sempre no composer.
  - `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos.
  - **✅ E3 e E4 testados na janela real — 2026-08-17**: usuário confirmou o modal único "MCPs (N/M)"
    (E3) e o ícone dedicado de suporte a visão com cache (E4) funcionando.
- **Replanejamento das Fases 3-6 (Parte I) — 2026-08-16, só documentação, nenhum código**: pedido do usuário pra detalhar/planejar o que faltava da Parte I do roteiro, no mesmo padrão granular da Parte II (arquivos/funções concretas, decisões explícitas, perguntas em aberto) em vez do nível conceitual em que essas fases foram esboçadas originalmente (antes de quase toda a Parte II existir). Achado central: boa parte do que a Parte I pedia já é consequência do que a Parte II construiu neste meio-tempo — o replanejamento é tanto "aqui está o plano" quanto "aqui está o que já não precisa mais ser feito".
  - **Fase 3 (pipeline Dev→QA→Analista)**: a única das 4 que genuinamente precisa de código novo — `agent/pipeline.rs` (laço determinístico, não decidido pelo LLM) encadeando `subagent::run` (DEV, reaproveitado sem mudança), `verifier::run` (QA, reaproveitado sem mudança) e um novo `agent/analyst.rs` (cópia estrutural do verifier, focado em cobertura de requisito em vez de correção técnica — decisão de duplicar em vez de generalizar o verifier pra não arriscar um módulo já testado em produção). Usa `AgentExecution.parent_id` pela primeira vez de verdade. Gatilho natural: um item novo no menu `/` (Fase E2, já implementado), fechando a pergunta original "ReadyPrompt ou comando /pipeline?".
  - **Fase 2 e Fase 5**: achado que a maior parte já está implementada por infraestrutura da Parte II — Fase 2 (prompts prontos como agentes leves) é literalmente o que `Persona` já faz desde T24/T48/B3 (`system_prompt_override`=`Persona.content`, filtro de tools/skills já existe); Fase 5 (gerente dinâmico) já funciona hoje via `task` (catálogo no prompt, paralelo via A4, rastreamento via C2) — falta só uma skill/persona "Gerente de Projeto" ensinando o padrão de delegação, não infraestrutura nova.
  - **Fase 4 (skills de produtividade)**: reclassificação de arquitetura por item — `file_organizer` é skill (ação pontual) com uma ideia nova de usar `create_python_tool` (T17) pra gerar undo de verdade em vez de um mecanismo específico; `email_triage` é skill que assume um MCP de e-mail do usuário já conectado, explicitamente fora de escopo o Cerne implementar cliente IMAP/Gmail próprio; `english_tutor` é **Persona**, não skill — é um modo de sessão inteira (mesma categoria do Fable), não uma ação carregada sob demanda no meio de uma tarefa.
  - **Fase 6 (skill store)**: a mais especulativa, decomposta em 3 degraus por custo de infraestrutura. Degrau 1 recomendado (zero infra nova): botão "Importar skill de URL" no `AgentsSkillsPanel.vue`, que também fecha o item pendente da Fase 1. Nuance de segurança nova (pós-T17): compartilhar SKILL.md é texto lido pelo LLM (risco = prompt injection, mitigado com preview obrigatório); compartilhar ferramentas Python seria código de verdade rodando no computador do usuário (risco categoricamente maior) — recomendado manter fora do escopo de compartilhamento até essa distinção ser resolvida explicitamente.
  - Checklist-resumo no roteiro (`13_roteiro_agentes_skills_fases.md`, seção "Checklist de Implementação Progressiva (Parte I)") atualizado pra refletir o que já está ✅ (consequência da Parte II) vs. o que ainda é ☐ (trabalho real pendente, agora bem menor que o esboço original).
- **Fase 3 — Pipeline Dev → QA → Analista (orquestração determinística)**: implementado exatamente como replanejado — nenhuma peça nova reinventa o que já existia, só uma orquestração nova por cima.
  - **`agent/analyst.rs` (novo)**: cópia estrutural de `verifier.rs` (mesmo toolset read-only `read_file`/`list_dir`/`grep`/`ast_grep`/`run_command`, mesmo formato de veredito APROVADO/REFUTADO na primeira linha, mesmo `extract_verdict` com fallback pra REFUTADO se o modelo não seguir o formato) — decisão deliberada de duplicar em vez de generalizar `verifier.rs` com um parâmetro de "papel", pra não arriscar um módulo já testado em produção por causa de uma feature nova. A diferença real está só no `ANALYST_SYSTEM_PROMPT` e nos argumentos de `run()`: recebe o **requisito original** do usuário (não só o relatório do dev), e audita cobertura de requisito, não correção técnica (isso é o QA, que já rodou antes na sequência).
  - **`agent/pipeline.rs` (novo)**: `pipeline::run(...)` — laço `loop` em Rust puro (não decidido por LLM) que encadeia `subagent::run` (DEV, reaproveitado sem NENHUMA mudança) → `verifier::run` (QA, reaproveitado sem NENHUMA mudança) → se REFUTADO volta pro DEV com as pendências anexadas ao prompt e incrementa `round` → `analyst::run` (Analista, novo) → se REFUTADO volta pro DEV → se os dois aprovarem, devolve relatório final consolidado. `max_rounds` default 3 (`pipeline::DEFAULT_MAX_ROUNDS`), nunca trava silenciosamente — se estourar o limite, devolve um relatório claro do que ficou pendente (`pending_report`) em vez de erro genérico.
  - **Tool nova `run_pipeline(requirement, max_rounds?)`** (`agent/tools.rs`, dentro de `project_tool_specs()` — precisa de `project_root`, mesma exigência de `task`/`verify_completion`) — dispatch tratado à parte em `agent/mod.rs` (mesmo padrão de `task`/`verify_completion`, precisa de `app`/`state`/`cfg` que `tools::execute_tool` não recebe). Entra também no modal batelado de aprovação do modo Manual (Fase A5) — mesma lista que já incluía `task`/`load_skill`/`verify_completion`.
  - **Guarda de profundidade**: `run_pipeline` excluída do toolset do sub-agente (`subagent_tool_specs()`, `subagent.rs`) — a etapa DEV de um pipeline não pode disparar outro pipeline de dentro de si mesma. Naturalmente já excluída do toolset do QA/Analista também, já que esses usam allowlist (não blocklist) e `run_pipeline` nunca foi adicionada a nenhuma das duas.
  - **`AgentExecution.parent_id` usado pela primeira vez de verdade**: documentado desde a Fase A1 como "sempre `None` na prática hoje" — `start_agent_execution` ganhou um parâmetro `parent_id: Option<&str>` (os 3 call sites antigos de `task`/`verify_completion` passam `None`, comportamento idêntico a antes), e cada etapa do pipeline (dev/qa/analista, por round) é registrada como filha da execução do pipeline inteiro. `AgentExecutionsPanel.vue` (Fase C2) não precisou de nenhuma mudança — já sabe renderizar isso, só passa a mostrar uma árvore de verdade em vez de sempre achatada.
  - **Evento `agent:pipeline_status`** (`{session_id, step, round, max_rounds}`) emitido antes de cada etapa — frontend (`ChatView.vue::statusLabel`) mostra "🛠️ Dev implementando (round 1/3)...", "🧪 QA testando...", "📋 Analista conferindo..." com prioridade sobre o status genérico "pensando"/"rodando ferramenta" enquanto o pipeline estiver ativo. Estado `pipelineStatus` na store (`stores/session.ts`), limpo em `onAgentDone`/`selectSession` (nunca fica "preso" mostrando o último round depois que o turno termina ou a sessão troca).
  - **Gatilho no composer**: com o atalho `/` (Fase E2, já implementado) disponível, o pipeline virou mais um item do menu (`kind: "pipeline"`, só aparece com pasta de projeto associada) em vez de precisar de um comando dedicado — resolve a pergunta original "ReadyPrompt ou comando `/pipeline`?" de graça. Diferente de skill/persona (ação imediata), selecionar insere uma frase-modelo no composer ("Use o pipeline Dev → QA → Analista para: ") e deixa o cursor pronto pro usuário completar com o requisito — não dá pra chamar a tool direto do frontend porque `run_pipeline` precisa do requisito completo, que o usuário ainda não tinha digitado no momento de abrir o menu.
  - 7 testes novos: `analyst_toolset_is_read_only_allowlist` + 3 de `extract_verdict` (`analyst.rs`); `is_refutado_recognizes_prefix_case_insensitive` + `pending_report_includes_round_and_verdict` (`pipeline.rs`); `subagent_toolset_excludes_task_but_keeps_everything_else` atualizado pra também confirmar exclusão de `run_pipeline` (contagem `parent_names.len() - 4` em vez de `- 3`).
  - **Perguntas em aberto do roteiro, resolvidas na implementação**: não adicionei um `ask` extra entre rounds em modo Manual (o `ExecutionMode::Manual` já pausa em toda tool call do DEV a cada round, seria redundante); não adicionei limite de tempo/tokens específico do pipeline além de `max_rounds` (doom-loop detection + `MAX_AGENTIC_STEPS` de cada sub-chamada já cobrem loop patológico dentro de uma etapa).
- **Fase 4 — Skills de produtividade**: implementada como replanejada — 2 das 3 ideias são skills novas (conteúdo, zero código Rust), a 3ª é documentação de Persona (também zero código).
  - **`file-organizer` (skill nova, seedada)**: `src-tauri/src/skills_examples/file_organizer.md`, adicionada ao array `EXAMPLE_SKILLS` (`skills.rs`) — semeada na primeira execução do app, junto de `weather`/`summarize`/`skill-creator` (mesmo mecanismo do B2/T47, nenhuma mudança na função `ensure_global_skills_dir`, só mais 2 entradas no array). A skill ensina: `list_dir` → classificar por extensão/tamanho/data → `ask` confirmando o plano → **antes de mover, usar `create_python_tool` (T17) pra gerar um organizador com log de undo reutilizável** (grava um manifesto `origem→destino` antes de mover, aceita `--reverse` pra desfazer) — a ideia nova do replanejamento (reaproveitar o T17 em vez de inventar um mecanismo de undo específico) foi implementada como conteúdo da própria skill, ensinando o padrão em vez de codificar um "undo" no Cerne.
  - **`email-triage` (skill nova, seedada)**: `src-tauri/src/skills_examples/email_triage.md`, mesmo mecanismo de seed. Ensina o fluxo assumindo que o usuário já tem um MCP de e-mail conectado (verifica `mcp__*` disponíveis antes de agir, se não tiver nenhum avisa o usuário e para) — documentado explicitamente que o Cerne NÃO implementa cliente IMAP/Gmail/Outlook próprio, é fora de escopo de propósito.
  - **`english-tutor` (Persona, documentada, NÃO seedada)**: diferente das outras duas, não virou skill nem foi criada automaticamente como Persona — mesma decisão de escopo já tomada pra Fase 2 (semear Persona polui a lista de escolha ativa do usuário de um jeito que semear skill não faz, já que skill só é lida sob demanda). Em vez disso, nova seção "## Personas" adicionada nos 4 arquivos de ajuda (`content/help.{pt-BR,en,es,zh}.md` — essa seção não existia ainda, é a primeira vez que Persona é documentada na ajuda do app) com o formato explicado + o exemplo completo copiável do Tutor de Inglês (nome/content/tools sugeridos).
  - Teste `ensure_global_skills_dir_seeds_example_skills_once` (`skills.rs`) atualizado pra confirmar as 2 novas skills também são semeadas. `cargo check --lib`/`--tests` limpos.
  - **Gap conhecido, não deste escopo**: os arquivos de ajuda ainda não documentam `create_python_tool`/`update_python_tool`, `create_pptx`, `run_pipeline`, nem pastas de sessão (T29) — a seção "## Personas" foi adicionada de forma pontual só pro que a Fase 4c precisava, não é uma modernização completa da ajuda (isso já era um item pendente registrado desde a Fase 1, "documentar formato SKILL.md na ajuda", que segue em aberto).
- **Fase 5 — Multi-agente gerenciado**: confirmado o achado do replanejamento — nenhuma infraestrutura nova foi necessária, só a skill `project-manager` (`src-tauri/src/skills_examples/project_manager.md`, seedada via `EXAMPLE_SKILLS` igual as outras da Fase 4). Ensina o padrão de decompor um objetivo em sub-tarefas genuinamente independentes, delegar via `task` (que já roda em paralelo automaticamente com 2+ chamadas no mesmo turno via provider de API, Fase A4), sintetizar os relatórios, e repetir se precisar de mais uma rodada — com uma seção explícita "quando NÃO usar" (pedidos com sub-tarefas em cadeia/dependentes não se beneficiam de delegação paralela) e uma nota sobre lotes pequenos em vez de despejar todas as sub-tarefas de uma vez (custo de tokens real, não é um limite técnico novo). Teste `ensure_global_skills_dir_seeds_example_skills_once` atualizado. `cargo check --lib`/`--tests` limpos.
  - **✅ Testado na janela real — 2026-08-17**: usuário confirmou `file-organizer`/`email-triage`
    (Fase 4) e `project-manager` (Fase 5) funcionando.
- **Reestruturação de `search_auto` (busca keyless) — 2026-08-16, pedido do usuário depois de uma pesquisa sobre o MCP de busca do Ollama e o repositório `odysseus-dev/odysseus`**:
  - **Contexto da pesquisa**: usuário perguntou se o MCP de busca do Ollama valia a pena conectar, e pediu pra estudar como o Odysseus (`github.com/odysseus-dev/odysseus`) implementa busca. Achados (via agente de pesquisa): o MCP do Ollama é uma API REST hospedada nos servidores da Ollama, **paga/com conta** (precisa de API key, limite maior só no plano pago) — estruturalmente igual às opções Brave/Tavily que o Cerne já suporta, não resolve a fragilidade do modo keyless, então não foi conectado. O Odysseus usa SearXNG auto-hospedado como fonte principal com uma **cadeia de fallback com retry** (tenta 1 provider, retry, só escala pro próximo em falha/vazio) em vez de disparar várias fontes em paralelo sempre — essa é a ideia que valia trazer pro Cerne.
  - **Mudança em `websearch.rs::search_auto`**: antes disparava DuckDuckGo+Brave+Mojeek sempre em paralelo (`tokio::join!` dos 3) e mesclava o resultado, mesmo quando uma fonte só já bastaria. Agora: tenta o **DuckDuckGo sozinho primeiro** (com a nova função `with_retry`, que dá 1 retry com 400ms de espera só em caso de ERRO de rede/HTTP, não em resultado vazio-mas-válido) — se vier com `>= MIN_RESULTS` (8), devolve na hora, só 1 request de verdade. Só escala pra Brave+Mojeek em paralelo (mesma lógica de merge multi-engine por posição de antes) quando o DuckDuckGo veio fraco, vazio ou falhou mesmo com retry.
  - **Por que essa versão híbrida, e não uma cadeia estritamente 1-por-vez (DDG→Brave→Mojeek em sequência) igual o Odysseus faz**: o Odysseus usa SearXNG como fonte PRINCIPAL (que já agrega vários engines internamente) e só cai pro DuckDuckGo isolado como fallback — o padrão "1 provider por vez" faz sentido pra eles porque o provider principal já é multi-engine por dentro. O Cerne no modo `Auto` não tem um SearXNG "por baixo" (isso é uma opção separada, `SearchProviderKind::Searxng`, que o usuário escolhe explicitamente) — o valor da busca `Auto` do Cerne sempre foi justamente agregar 3 fontes independentes pra ganhar precisão por consenso (documentado desde a implementação original, "estudei o código-fonte do SearXNG pra copiar a ideia real"). Perder isso no caminho feliz seria uma regressão de qualidade sem necessidade — a maioria das buscas o DuckDuckGo sozinho já responde bem. O resultado é o melhor dos dois mundos: caminho feliz vira 1 request (reduz MUITO a chance de bloqueio anti-bot, resolve o mesmo problema que o semáforo do T46 mitigava só parcialmente) e o merge multi-engine entra exatamente quando é preciso (DuckDuckGo com dificuldade).
  - **Semáforo do T46 mantido**: continua limitando a 2 chamadas concorrentes de `search_auto` no app inteiro — complementar à mudança de hoje, não substituído por ela (o semáforo protege contra rajada de VÁRIAS buscas simultâneas de turnos/sub-agentes diferentes; a cadeia de fallback protege contra excesso de requisições DENTRO de uma única busca).
  - 3 testes novos pra `with_retry` (sucesso na 2ª tentativa depois de 1 erro; desiste depois de 2 tentativas devolvendo o erro da 1ª; resultado vazio-mas-Ok NÃO dispara retry — diferencia "falhou" de "achou pouco"). `cargo check --lib`/`--tests` limpos; `cargo test` não roda nesta máquina (ambiente já documentado).
  - **✅ Testado em produção — 2026-08-17**: usuário confirmou o semáforo do T46 funcionando contra
    as fontes reais, rodando em paralelo com sub-agentes.
- **Novos provedores de busca (Serper, Exa, Google Custom Search, Bing/Azure) — ✅ testado — 2026-08-17**:
  usuário confirmou funcionando contra chave de API real.
- **Modelo global / remoção do "Provider ativo" — ✅ IMPLEMENTADO e TESTADO — 2026-08-17**: pedido
  do usuário depois de reparar que Configurações mostrava uma seção "Provider ativo" com
  OpenRouter marcado "Ativo" por padrão, sem relação nenhuma com o modelo que ele realmente estava
  usando nas sessões. Achado investigando: `AppConfig` já tinha os campos certos
  (`active_provider`/`active_model`/`active_llama_fork`/`active_custom_provider_id`), mas só
  `setActiveProvider` (chamado só por aquela seção de Configurações) escrevia neles — os outros 3
  setters (`setActiveModel`/`setActiveLlamaFork`/`setActiveCustomProvider`) existiam no store mas
  nunca eram chamados por nenhum componente, então trocar de modelo numa sessão nunca atualizava o
  "último modelo usado" global. `NewSessionDialog.vue` já lia esses campos pra pré-popular sessão
  nova, mas na prática eles quase nunca mudavam depois do primeiro uso do app. Corrigido: nova
  ação `providerStore.setActiveSelection(provider, model, fork?, customProviderId?)`
  (`stores/provider.ts`) grava tudo numa escrita só, chamada de `ComposerBar.vue::onModelChange`
  (troca de modelo numa sessão existente) e `NewSessionDialog.vue::create` (escolha na criação) —
  agora TODA escolha de modelo em qualquer sessão vira o default global de verdade. A seção
  "Provider ativo" foi removida de `Settings.vue` (marcação manual redundante, confusa depois da
  mudança), junto com as chaves i18n `settings.activeProvider*` (4 idiomas) e o CSS
  `.provider-grid`/`.provider-card`/`.active-badge`. Os 4 setters antigos do store viraram só 1
  (`setActiveSelection`) — os 3 que já eram mortos e o `setActiveProvider` (órfão depois da
  remoção da seção) foram removidos. `vue-tsc --noEmit` limpo. **Confirmado pelo usuário**: "1 -
  ok, pode marcar como concluído".
- **Lembrar o contexto configurado de um modelo entre sessões — ✅ IMPLEMENTADO — 2026-08-18**:
  pedido do usuário ("ao colocar o contexto de um LLM por API, que salve para que independente da
  API, venha o mesmo contexto cadastrado em outras sessões"). Antes, `context_length` só existia
  por SESSÃO (`Session.context_length`) — trocar de sessão com o MESMO modelo perdia o ajuste
  manual feito no `ContextGauge.vue`. Novo arquivo `model_context_overrides.json` em
  `app_data_dir` (`config.rs`, mesmo padrão de `model_favorites.json` já existente), mapa
  `"provider_key::model_id" -> tamanho`, onde `provider_key` reaproveita a MESMA convenção de
  conexão que os favoritos já usam (`openrouter`/`llama_cpp:{fork}`/`custom:{id}`/etc — resolve a
  pergunta em aberto do registro original: sim, por modelo+provider, não só nome do modelo, já que
  o mesmo nome pode significar coisas diferentes em conexões diferentes). Novos comandos Tauri
  `get_model_context_override`/`set_model_context_override`. Frontend: `modelContextOverrideKey`
  exportado de `stores/provider.ts` (reaproveita a `modelsCacheKey` interna, antes não exportada);
  `sessionStore.updateContextLength` agora também grava o override toda vez que o usuário ajusta
  manualmente; nova ação `applyRememberedContextLength` (chamada ao entrar em qualquer sessão, via
  `selectSession`) aplica o valor lembrado automaticamente quando a sessão ainda não tem um
  `context_length` próprio — nunca sobrescreve um valor que a sessão já tenha. 1 teste novo em
  `config.rs` (roundtrip). `cargo test --lib` (199 passando) e `vue-tsc --noEmit` limpos.

  **✅ CONFIRMADO E CORRIGIDO NA JANELA REAL — 2026-08-20.** Usuário testou e reportou: "funciona
  pra local, mas não pra API" — três bugs reais encontrados e corrigidos:
  - **O valor lembrado nunca disparava pra conexões de API.** Causa raiz: pra OpenRouter, a criação
    de sessão (`create_session`) já preenchia `context_length` automaticamente com o valor real
    devolvido pelo `/models` da própria OpenRouter — e `applyRememberedContextLength` (frontend) só
    age quando a sessão ainda não tem NENHUM `context_length`. Como a API quase sempre já vinha
    preenchida (e local falhava mais esse lookup automático), o valor lembrado só tinha chance
    real de aplicar em provider local. Corrigido MOVENDO a prioridade pro backend: nova
    `resolve_session_context_length` (`lib.rs`) — chamada por `create_session` E
    `update_session_provider_model` — confere o valor LEMBRADO primeiro (mesma chave, replicada em
    Rust via `model_context_override_key`, com 4 testes confirmando que bate com a convenção do
    frontend) e só cai no lookup automático (`resolve_context_length`) quando não há nada lembrado
    ainda. Resolve de vez, sem depender da corrida entre criação e o hook do frontend.
  - **Model browser mostrando Ctx errado** (ex: usuário viu 131K quando a própria página da
    OpenRouter mostra 1M pra `deepseek/deepseek-v4-flash-0731`, com print comparando as duas
    telas). Causa raiz, achada lendo `providers::resolve_context_length`: a ordem de prioridade
    estava INVERTIDA — cache em disco e a tabela `KNOWN_CONTEXT_LENGTHS` (chute hardcoded, pensada
    só pra quando o provider NÃO manda contexto nenhum) eram checados ANTES do valor real que a
    OpenRouter mandava (`provider_override`). O id bateu por SUBSTRING numa entrada antiga da
    tabela (`"deepseek-v4-flash"` → 131072) e ficou PRESO nesse valor errado, inclusive persistido
    no cache (perpetuando o erro em toda chamada futura). Corrigido: valor real do provider agora
    sempre vence e sobrescreve qualquer cache velho; a tabela hardcoded só é usada quando o
    provider não devolve nada. 3 testes novos em `providers::tests`.
  - **"Criar sessão" ficou visivelmente lento** — efeito colateral direto do fix acima: toda sessão
    de API (`create_session`/`update_session_provider_model` → `resolve_context_length` →
    `providers::get_context_length`) refazia um fetch do catálogo INTEIRO da OpenRouter (~414
    modelos) só pra achar o contexto de UM modelo — e, como o valor real agora sempre grava no
    cache, isso significava até 414 leituras+escritas sequenciais do MESMO arquivo JSON por
    listagem. Dois fixes em `providers/mod.rs`: (a) `get_context_length` agora confere o cache em
    disco ANTES de tentar qualquer fetch de rede — se o modelo já foi visto antes (ex: o usuário
    já abriu "Nova sessão" e viu a lista), retorna na hora, sem round-trip nenhum; (b) `list_models`
    resolve o contexto de TODOS os modelos da resposta em memória contra uma única leitura do
    cache, e só grava no disco UMA vez no final (e só se algo realmente mudou), em vez de tocar o
    arquivo por modelo. `cargo test --lib` (218 passando) confirma sem regressão.
- **"Faltou a opção para criar agente" — ✅ CORRIGIDO — 2026-08-17**: usuário reportou (com print)
  não achar como criar um agente no painel "Agentes & Skills" — só via as abas "Skills",
  "Personas" e "Ferramentas Python". Achado: não faltava funcionalidade nenhuma — `Persona` já é
  literalmente o "agente nomeado" que a Fase A3 original pedia (nome + `system_prompt_override` +
  filtro de tools/skills), só que a aba continuava rotulada "Personas" em vez de "Agentes", então
  ninguém achava a opção procurando por "agente". Corrigido só o rótulo: aba
  `agentsSkillsPanel.personasTab` → "Agentes ({count})", botão `settings.createPersona` → "Criar
  agente", `agentsSkillsPanel.noPersonas` → "Nenhum agente encontrado" — nos 4 idiomas
  (`AgentsSkillsPanel.vue` continua usando `Persona` por baixo, é troca de texto só). O seletor de
  Persona dentro do composer (rodapé, troca a persona ativa da sessão) foi deixado como está —
  superfície diferente, já estabelecida, sem o mesmo problema de descoberta. `vue-tsc --noEmit`
  limpo.
- **Separar "Agentes" de "Personas" no painel — ✅ IMPLEMENTADO — 2026-08-18**: pedido do usuário
  na manhã seguinte à leva noturna, depois de ver a aba única "Agentes" (fusão do dia anterior) —
  "apesar de serem coisas parecidas, vai servir pra objetivos diferentes: Agente é como se fosse
  orquestrador (use a ferramenta X, depois Y, leia o arquivo da pasta W e mova pra pasta R),
  Persona é como se fosse professor (Especialista em inglês, ensina pra profissional de TI, com X
  anos de experiência); na programação é igual, mas no uso é diferente". Implementado exatamente
  assim: novo campo `PersonaKind` (`Agent` | `Persona`, default `Persona` — retrocompatível com
  todo registro salvo antes desse campo existir) em `models::Persona`/`personas.rs`
  (`create_persona`/`update_persona` ganham o parâmetro `kind`). Painel "Agentes & Skills" volta a
  ter 4 abas (Skills, Agentes, Personas, Ferramentas Python) — as duas do meio filtram a MESMA
  lista (`sessionStore.personas`) por `kind`, sem duplicar dado nem storage. `PersonaEditorModal.vue`
  ganhou uma prop `defaultKind` (só usada criando do zero — editar um existente sempre usa o
  `kind` já salvo) que troca título/texto de ajuda/placeholder do conteúdo pra dar o exemplo certo
  em cada contexto (procedural pra Agente, especialista/tom pra Persona). **Confirmado pelo
  usuário**: "Pode dar agente e persona como pronto." O seletor de Persona no
  rodapé do composer foi deixado como está (mostra as duas juntas) — fora do escopo do pedido,
  que era especificamente sobre o painel. Novo teste
  `kind_defaults_to_persona_for_old_data_and_roundtrips_agent` em `personas.rs`. `cargo check
  --lib`/`--tests` (196 passando) e `vue-tsc --noEmit` limpos.
- **Aviso de `git` ausente — não testável nesta máquina — 2026-08-17**: o mecanismo já existe (ver
  T17 acima, `sessionStore.checkGitAvailable` + card de aviso no chat quando `git` não é encontrado
  e a sessão tem `project_root`), mas não dá pra confirmar visualmente porque `git` está instalado
  nesta máquina — precisaria de um ambiente sem `git` no PATH pra exercitar o caminho de erro de
  verdade. Fica registrado como implementado mas sem confirmação visual possível por ora.
- **Fase 6 — Skill Store / Comunidade (Degrau 1: "Importar skill de URL")**: implementado exatamente como recomendado no replanejamento — zero registry, zero hospedagem nova, fecha também o item pendente da Fase 1 ("permitir importar skill de URL").
  - **Backend (`skills.rs`)**: `fetch_skill_from_url(url)` busca o texto cru de uma URL (reaproveita `websearch::validate_public_url`, promovida de privada pra `pub(crate)`, pra bloquear localhost/rede interna — mesma validação que `web_fetch` já usa, já que a URL vem de input livre do usuário) e devolve até 200k caracteres. `import_skill(app_data_dir, content)` parseia o frontmatter do conteúdo (reaproveita `split_frontmatter`, já existente), extrai `name` obrigatório (erro claro se faltar), e salva como skill nova — falha se já existir uma com o mesmo slug (mesma regra de `create_skill`, não sobrescreve silenciosamente). Extraí um helper `slugify()` compartilhado entre `create_skill` e `import_skill` (estava duplicado inline). Dois comandos Tauri novos (`fetch_skill_from_url`, `import_skill`).
  - **Frontend**: `SkillImportModal.vue` novo — campo de URL + botão "Buscar", mostra o conteúdo COMPLETO num textarea editável antes de qualquer coisa ser salva (nunca "instala direto"), botão "Importar" só habilita depois do fetch trazer conteúdo. Botão "Importar de URL" adicionado ao lado de "+ Nova skill" na aba Skills do `AgentsSkillsPanel.vue`.
  - **Segurança, reafirmada da decisão do roteiro**: skill é texto lido pelo LLM, não código executado — o preview obrigatório antes de salvar é a mitigação real (contra prompt injection), não uma checagem de "permissão" (a skill importada não ganha nenhuma capacidade que o Cerne já não desse antes). Deliberadamente NÃO incluí ferramentas Python (T17) nesse fluxo de import — o roteiro já registrava essa distinção (compartilhar código executável é categoria de risco diferente de compartilhar texto).
  - 3 testes novos em `skills.rs`: roundtrip completo (conteúdo salvo é byte-a-byte o que foi importado, sem reescrever nada), erro claro sem `name` no frontmatter, erro ao importar slug duplicado (não sobrescreve). `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos.
  - **✅ Testado na janela real — 2026-08-17**: fluxo completo (colar URL → buscar → editar preview → importar → aparecer na lista) confirmado pelo usuário contra uma URL de verdade (`raw.githubusercontent.com/sipeed/picoclaw/.../summarize/SKILL.md`). Funcionou de primeira.
- **T17 (Python tools) — ✅ testado, e checagem de dependência externa adicionada — 2026-08-17**:
  usuário confirmou `create_python_tool` funcionando (tem `uv` instalado), mas levantou o cenário
  de um usuário só com o Cerne Code instalado, sem `uv` — hoje isso batia num erro de shell confuso
  ("comando não encontrado"). Adicionado `agent::shell::command_exists` (já existia, usado pra
  detectar `pwsh`/`powershell`, promovido de privado pra `pub(crate)`) + comando Tauri genérico
  `check_command_available(name)`. Dois usos: (1) `create_python_tool`/`update_python_tool` checam
  `uv` e anexam um aviso claro no resultado da tool se não encontrado (a ferramenta é criada mesmo
  assim — usuário pode instalar `uv` depois); (2) `git` é checado uma vez na abertura do app
  (`sessionStore.checkGitAvailable`), com um card de aviso dispensável no chat (mesmo padrão visual
  do aviso de computer-use) quando a sessão atual tem `project_root` e `git` não foi encontrado —
  relevante pro visualizador de diff de repositório (T42) funcionar. MCP via `npx`/`node` avaliado e
  **decidido não precisar** de checagem nova — já existe o botão "Testar conexão" ao configurar um
  servidor MCP, que já expõe erro claro se o comando não existir (o servidor já roda através do
  shell detectado do sistema, que resolve `npx`/`node` igual um terminal resolveria).
- **Fix: telas de console piscando na abertura + tela preta rápida ao terminar resposta do LLM — ✅ CORRIGIDO e TESTADO — 2026-08-17**:
  - **Telas de console**: causado pelo `git.rs` (novo nesta sessão) — cada chamada `git status`/`git
    diff` no Windows abria uma janela de console visível por faltar `CREATE_NO_WINDOW`, diferente do
    resto do código que já tinha esse cuidado (`agent/shell.rs::apply_creation_flags`, só pra
    `tokio::process::Command`). Adicionado `apply_std_creation_flags` (mesma flag, pra
    `std::process::Command`) e aplicado em `git::run_git`. Achado relacionado: `RepoDiffViewer.vue`
    rodava `git status`/`git diff` toda vez que o usuário trocava de SESSÃO (o `watch` do
    `project_root` disparava mesmo com o modal fechado) — corrigido pra só rodar quando o modal está
    de fato aberto.
  - **Tela preta ao terminar resposta**: `onAgentDone` (`stores/session.ts`) limpava
    `streamingText`/`liveBlocks` (o texto "ao vivo") ANTES de terminar de recarregar o histórico
    persistido (5 chamadas sequenciais ao backend) — buraco visual entre o streaming sumir e o
    histórico final ainda não ter chegado. Corrigido: `fetchReloadData`/`applyReloadData` extraídos
    de `reloadCurrent()`, busca das 5 partes agora em `Promise.all` (paralelo, mais rápido também),
    e só troca TODO o estado (histórico novo + limpar streaming) numa tacada síncrona só, depois que
    tudo já chegou — sem buraco no meio.
  - **✅ Confirmado pelo usuário na janela real**: "sem telas pretas".
- **Fix: vazamento de memória em sessão de uso prolongado — ✅ CORRIGIDO — 2026-08-17**: investigando
  um relato do usuário ("o app fechou sozinho depois de muito tempo de uso"), achei evidência real
  no Log de Eventos do Windows — evento `RADAR_PRE_LEAK_64` (heurística de vazamento do próprio
  Windows) cerca de 1h20 antes de um crash de verdade (`AppCrash`, exceção `0xc0000409`, falha
  fatal de alocação) em 2026-08-07. Causa raiz real, achada lendo o código: todo
  `run_command(background=true)` cujo processo termina SOZINHO (a maioria — build, teste, `git`,
  etc.) nunca era removido de `BackgroundJobs.jobs` (só o `stop()` explícito removia) — o `Child`
  handle e o buffer de output inteiro (até `MAX_OUTPUT_LINES` cada) ficavam presos em memória pra
  sempre. Corrigido com `prune_finished_jobs` (`background.rs`): teto de 50 jobs rastreados
  simultaneamente, remove os mais antigos JÁ TERMINADOS (nunca um rodando) quando passa do teto,
  chamado toda vez que um novo job começa. Mesma categoria de problema também corrigida por
  precaução em `state.agent_executions` (`agent/mod.rs::prune_finished_executions`, teto de 100 —
  guarda `.steps` com o detalhe de cada tool call de `task`/pipeline, também nunca liberado antes).
  **Ressalva honesta**: não dá pra confirmar que essa era a causa EXATA do crash de 07/08 — os
  jobs em segundo plano só existem desde 14/08 nesse código, depois da data do crash — mas é um
  vazamento real e concreto encontrado de qualquer forma, vale corrigir independente disso.
  `cargo check --lib`/`--tests` (176 testes) limpos. Sem teste dedicado pro teto exato (exigiria
  spawnar 50+ processos reais só pra testar a constante, custo/benefício ruim) — coberto pela
  suíte existente não quebrar.
- **T14 — Background jobs com callback automático**: `background.rs::spawn_reader` (chamado uma vez por stdout, uma vez por stderr de cada job) agora recebe um `JobCompletionCtx` compartilhado (via `Arc`/`Clone`) com um contador `remaining_streams: Arc<Mutex<u8>>` iniciado em 2 — quando o loop de leitura de um dos dois streams termina (pipe fechou, processo morreu), decrementa o contador; quem chegar a 0 (o último dos dois a fechar) dispara `on_job_finished`, que (1) emite `agent:background_done` com id/comando/output final (novo evento, ainda sem consumidor de UI — fica pra quando o Cerne tiver notificação genérica, mesma observação já feita no T32) e (2) injeta uma `ChatMessage{role:"system", ...}` com o output final direto no `chat_log.json` da sessão que iniciou o job (via `sessions::load_messages`/`save_messages`, mesmas funções que `run_turn` já usa). `BackgroundJobs::start` ganhou dois parâmetros novos (`app_data_dir`, `session_id`) pra isso ser possível — teve que propagar por toda a cadeia de chamada: `tools::execute_tool`/`execute_project_tool` ganharam `session_id: &str` (o `app_data_dir` já vinha como parâmetro), e os 3 call sites (`agent/mod.rs`, `subagent.rs`, `verifier.rs`) passaram a mandar o `session_id` que já tinham em escopo. System prompt (`agent/mod.rs`) ganhou uma frase avisando o modelo pra não ficar chamando `check_background_output` em loop só pra descobrir se terminou, já que o resultado aparece sozinho. Teste novo `finished_job_injects_message_into_session_history` (`background.rs`) confirma a injeção fim-a-fim com um `echo` real. **Não implementado** (fora do pedido original, e mais arriscado): reabrir automaticamente o *turno* do LLM quando a mensagem é injetada — hoje ela só fica disponível pro *próximo* turno que rodar (user manda mensagem nova, ou outro processo dispara `run_turn`); não há um mecanismo que force a sessão a continuar sozinha no meio do nada. Mesma pergunta em aberto documentada na Fase G (sessões orquestradas) — se um dia isso for resolvido, os dois recursos usariam a mesma solução.
  - Mudança inteira é aditiva/retrocompatível: só afeta sessão com persona ativa E `tools` explicitamente preenchido — todo o resto do comportamento (incluindo toda skill/persona já cadastrada) fica idêntico a antes. `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos. **Não testado na janela real** (a pedido do usuário, sem build nesta rodada).
- **T16 — Ferramenta `create_pptx`**: não existe crate PPTX madura pro Rust (diferente de `rust_xlsxwriter`/`docx-rust`/`printpdf`, já usadas por `create_excel`/`create_word`/`create_pdf`) — um `.pptx` é só um ZIP com partes XML descritas pelo schema OOXML PresentationML, então montei na mão usando a crate `zip` (já dependência do projeto, antes só em `[dev-dependencies]` pro teste de `attachments.rs` que lê xlsx de anexo — **promovida pra `[dependencies]` de verdade** já que agora é usada em produção). Estrutura gerada: 1 `slideMaster` + 1 `slideLayout` "blank" fixos (boilerplate), 1 `theme` com as cores do Cerne, e N slides (1 por item da lista `slides`), cada slide com shapes posicionadas explicitamente via `a:xfrm` (não depende de placeholder herdado do layout). Cada slide empilha `elements` de cima pra baixo: `paragraph` (com negrito/itálico/tamanho), `bullets`, `table`, e **`image`** — pedido explícito do usuário pra não fazer a versão reduzida sem imagem que eu tinha cogitado inicialmente. Imagem: lê o arquivo do disco (mesma resolução de caminho do `read_file` — aceita absoluto de qualquer lugar), usa a crate `image` (já dependência, usada hoje pro `computer_use`) só pra ler as dimensões reais em pixel e converter pra EMU a 96 DPI (mesma conversão do python-pptx), preservando a proporção original quando `width_in`/`height_in` não são informados, e encolhendo mantendo proporção se não couber no slide. Vira uma parte `ppt/media/imageN.ext` + relationship no `.rels` do slide + elemento `<p:pic>`; `[Content_Types].xml` ganha um `<Default Extension="...">` por extensão de imagem realmente usada (dedupe automático). Se uma imagem específica falhar ao decodificar (arquivo corrompido/formato exótico), cai num fallback 800×600 em vez de derrubar a apresentação inteira por causa de uma imagem ruim.
  - Teste `create_pptx_end_to_end_with_image` (`tools.rs`) gera um PNG de verdade em memória (via a própria crate `image`), monta uma apresentação de 2 slides (parágrafo+bullets no primeiro, tabela+imagem no segundo), reabre o `.pptx` resultante com `zip::ZipArchive` e confere: todas as partes obrigatórias do OOXML presentes, texto de cada slide aparece no XML certo, e os bytes da imagem embutida batem byte-a-byte com o arquivo original. Teste `create_pptx_rejects_empty_slides_via_execute_project_tool` confirma a validação de `slides` vazio.
  - **Ressalva importante**: gerar o zip com a estrutura OOXML correta foi verificado por leitura cuidadosa do schema (de memória, sem uma referência ao lado) + o teste acima, que confirma que o arquivo é um ZIP válido com as partes certas e o texto/imagem certos — mas **nunca foi de fato aberto num PowerPoint ou LibreOffice Impress reais** nesta sessão (não tem nenhum dos dois disponível neste ambiente, e não rodei build/preview). Existe uma chance real de algum detalhe fino do schema (ex: um atributo obrigatório faltando no `theme1.xml`) fazer o PowerPoint abrir com "reparo automático" na primeira vez, mesmo o arquivo sendo estruturalmente um ZIP válido com XML bem formado. Recomendo fortemente abrir um `.pptx` gerado de teste no PowerPoint/LibreOffice assim que possível e reportar qualquer aviso de reparo — é o único jeito de fechar essa lacuna de verdade. `cargo test` não roda nesta máquina (mesmo problema de ambiente já documentado em T34), então nem o teste automatizado pôde ser executado, só type-checado.
- **T17 — IA cria ferramentas Python customizadas**: novo módulo `python_tools.rs`, seguindo exatamente o fluxo descrito no backlog original — LLM gera script → salva em `{app_data_dir}/python_tools/<slug>/` → gera uma skill automática (`skills.rs`, reaproveitado sem mudar nada nele) → aparece no catálogo de skills de QUALQUER sessão futura, não só a que criou. Cada ferramenta tem `tool.py` (o script real, executável) + `meta.json` (nome/descrição/dependências, pra listar/editar depois sem precisar reparsear o `.py`). Gerenciamento de dependências via **UV**: `tool.py` recebe um cabeçalho de metadata inline **PEP 723** (`# /// script\n# dependencies = [...]\n# ///`) gerado automaticamente a partir da lista `dependencies` — isso é o que faz `uv run tool.py` instalar as dependências sozinho num ambiente Python efêmero, sem o Cerne precisar gerenciar venv nenhum na mão (era a alternativa mais simples e correta pro que o backlog pedia, "usar UV pra gerenciar dependências"). A skill companheira (nome `python-tool-<slug>`, pra não colidir com skill escrita à mão) documenta as dependências e o comando exato (`uv run "<caminho>" [args]`) via `run_command`.
  - Duas ferramentas novas pro agente (`agent/tools.rs`, em `always_tool_specs()` — GLOBAIS, não presas a projeto, mesmo grupo de `web_search`/`web_fetch`): `create_python_tool(name, description, script, dependencies)` (falha se o nome já existir) e `update_python_tool(...)` (mesmos parâmetros, falha se NÃO existir — pensada pro próprio agente corrigir um bug numa ferramenta que ele mesmo escreveu antes, em vez de duplicar com outro nome). O verificador (`verifier.rs`) não ganha essas tools (ele só usa `project_tool_specs()`, nunca `always_tool_specs()` — mesma exclusão estrutural que já existia pra `web_search` no verificador).
  - Frontend: nova aba "Ferramentas Python" no `AgentsSkillsPanel.vue` (`list_python_tools`), com "usar agora" (prepara mensagem no composer pedindo pra usar a skill companheira, mesmo mecanismo já usado pra skill comum), "editar" (`PythonToolEditorModal.vue` novo, edita descrição/dependências/script e chama `update_python_tool` — regenera a skill companheira igual o backend faz) e "excluir" (`delete_python_tool`, remove a pasta da ferramenta E a skill companheira, com confirmação). **Sem botão de "criar nova" na UI** — decisão de escopo deliberada, o próprio backlog descreve a criação como ação do LLM ("O LLM pode criar scripts... usuário vê... e pode editar/remover"), não do usuário.
  - 5 testes novos em `python_tools.rs` (create+list roundtrip incluindo o cabeçalho PEP 723 no arquivo real, nome duplicado rejeitado, update troca script/deps e regenera a skill, update de ferramenta inexistente falha, delete remove ferramenta+skill, delete de ferramenta inexistente falha). `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos.
  - **Ressalva**: nunca testei rodando `uv` de verdade nesta sessão (não sei se está instalado neste ambiente, e não rodei build/preview) — a integração é só "gerar um arquivo que `uv run` sabe interpretar", verificada por leitura do formato PEP 723 (spec pública, formato simples), não por execução real. Se o usuário não tiver `uv` instalado, a skill gerada instrui a rodar `uv run ...` e vai falhar com "comando não encontrado" — não há checagem/aviso prévio de que `uv` precisa estar no PATH. Vale adicionar uma checagem (`uv --version` via `run_command`) na primeira vez que o agente for usar isso, como possível iteração futura, mas não implementei pra não adicionar complexidade sem necessidade comprovada.
- **Redesign da área de chat/composer — ✅ IMPLEMENTADO e CONFIRMADO — 2026-08-20**, pedido do
  usuário em várias rodadas incrementais (cada uma testada ao vivo antes da próxima):
  - **Barra superior no chat**: seletor de modelo + ícone de visão (canto esquerdo) e
    navegar-arquivos/ver-diff (canto direito) saíram de dentro da caixa do composer e subiram pro
    topo da área de chat (`ChatView.vue`, novo `.chat-topbar`). A lógica/estado continuam 100% em
    `ComposerBar.vue` — só a posição visual mudou, via `<Teleport>` pros alvos `#chat-topbar-left`/
    `#chat-topbar-right`. **Bug real encontrado e corrigido**: sem a prop `defer` (Vue 3.5+), o
    Teleport falhava silenciosamente ("Failed to locate Teleport target...", visto no console pelo
    usuário) porque o alvo e quem teleporta montam na MESMA passada síncrona — `defer` resolve
    exatamente esse caso.
  - **Menu "+"** (Popover, mesmo padrão de `ExtraReadPaths.vue`) agrupa: anexar arquivo, Pastas
    extra, Modo de execução, Raciocínio, Persona, Método Fable e MCP — tudo que antes vivia solto
    como dropdowns/botões no rodapé do composer.
  - **Resumo em texto** (`Modelo: X, Modo: Y, Raciocínio: Z`), inspirado no Claude Code desktop,
    substitui os dropdowns de Modo/Raciocínio no rodapé — atualiza sozinho conforme a escolha muda
    no "+". Fica FORA da caixa do composer (embaixo dela), a pedido do usuário depois de ver a
    primeira versão (dentro do rodapé).
  - **`ContextGauge.vue`**: só a barra fica visível agora (ícone + texto numérico saíram); hover
    mostra os tokens usados (tooltip já existia), clique continua abrindo a edição manual.
  - **`ProviderPicker.vue`**: removido o modo "resumido/recolhido" (branch inteira de
    `expanded`/`picker-summary`/`collapse-btn`) — fica sempre expandido, dropdown de modelo mais
    largo (220px → 340px). Removido também o ícone de favoritar (estrela) do topbar e das linhas do
    dropdown — favoritar um modelo continua existindo só no navegador de modelos (Configurações),
    nunca foi removido de lá. Prop `collapsible` renomeada pra `hideVisionTest` (só sobrou pra
    suprimir o ícone de visão duplicado no contexto do composer — não tem mais nada a ver com
    colapsar).
  - **Espaçamento**: `.chat-inner`/`.composer-wrap` de 820px pra 960px de largura útil; padding
    vertical das linhas de sessão/pasta na sidebar reduzido em 3px (`SidebarSessionItem.vue`/
    `Sidebar.vue`).
  - `vue-tsc --noEmit` limpo em cada etapa; verificação visual foi 100% o usuário testando no
    `npm run tauri dev` ao vivo (app desktop Tauri, sem como confirmar layout por browser headless).
- **Bolinha piscando na sidebar (sessão processando) — ✅ CONFIRMADO — 2026-08-20**, com DOIS bugs
  reais encontrados e corrigidos durante o teste ao vivo (pedido original 2026-08-18, ver
  `sessionStore.processingSessionIds` já documentado no código):
  - **Bug 1 — pedido de aprovação de uma sessão em background era perdido pra sempre**: usuário
    testou 2 sessões de API rodando juntas; a primeira (modo Manual, sem `project_root`) pediu
    aprovação pra usar uma skill, o usuário trocou pra segunda sessão ANTES de responder, e a
    sessão A ficou travada com a bolinha piscando sem parar. Causa raiz real (não era concorrência
    entre sessões, primeira hipótese descartada por investigação): `pendingQuestion`/
    `pendingPermission`/`pendingAgentsSkillsPlan` (`stores/session.ts`) eram um valor ÚNICO global,
    e os handlers dos eventos (`onAskQuestion`/`onPermissionRequest`/`onAgentsSkillsPlan`)
    DESCARTAVAM o pedido inteiro se não fosse da sessão atualmente aberta (`if (x.session_id !==
    currentId) return`) — a sessão em background ficava esperando pra sempre uma resposta que
    nunca seria dada (o canal oneshot do backend não tem timeout), e por isso o turno nunca
    terminava (nunca emite `agent:done`/`agent:error`, que é o que limpa a bolinha). Corrigido:
    os 3 viraram mapas por `session_id` (`pendingQuestionBySession` etc.) sempre preenchidos,
    projetados pra sessão atual via 3 `getters` novos do Pinia (`pendingQuestion` etc. continuam
    existindo com a MESMA API pros componentes — `AskCard.vue`/`PermissionCard.vue`/
    `AgentsSkillsPlanCard.vue` não precisaram mudar nada) — voltar pra uma sessão que ficou
    esperando aprovação em background agora mostra o cartão certinho.
  - **Bug 2 — nenhuma chamada de busca na web tinha timeout**: mesmo teste expôs um segundo
    travamento (usuário reportou "a pesquisa ficou travada nas duas sessões", ~1min20s+ parado em
    "Buscou na web") — causa raiz: as 11 chamadas HTTP de `agent/websearch.rs` (DuckDuckGo/Brave/
    Mojeek scraping + as 7 APIs com chave) usavam `reqwest::Client::new()` sem NENHUM timeout — se
    o provider travar a conexão sem responder (ex: DuckDuckGo bloqueando por rate-limit, que foi o
    caso real do usuário), a chamada ficava pendurada pra sempre, travando o turno inteiro (e por
    tabela a bolinha, mesmo mecanismo do Bug 1). Corrigido: novo `http_client()` (timeout de 20s)
    substitui todo `reqwest::Client::new()` do arquivo. Usuário confirmou depois trocando pra busca
    via Exa (com chave de API) — o DuckDuckGo sem chave é o mais sujeito a bloqueio mesmo.
  - `cargo check --lib`/`cargo test --lib` (10 testes de `websearch`) e `vue-tsc --noEmit` limpos.
- **STT em português via Voicebox — ✅ IMPLEMENTADO e CONFIRMADO — 2026-08-20**: usuário perguntou
  se dava pra transcrever em português usando o Voicebox local. Resposta: já funcionava de graça
  (qualquer Whisper sem sufixo `.en` — Base/Small/Medium/Large/Turbo, as próprias opções que o
  Voicebox oferece pra download — é multilíngue), só faltava o Cerne deixar configurar a dica de
  idioma em vez de sempre deixar o Whisper auto-detectar (funciona, mas é menos confiável/mais
  lento). `voicebox::transcribe_audio` ganhou um parâmetro `language: Option<&str>` (campo
  multipart `language`, código ISO 639-1, ex: "pt" — omitido quando vazio, cai no auto-detect de
  sempre). Novo campo `voicebox_stt_language: String` no `AppConfig`, exposto em Configurações →
  Voz → STT (só aparece com o motor "Voicebox local") como "Idioma da transcrição". `cargo check`/
  `vue-tsc --noEmit` limpos.
- **Marcador por mensagem do usuário no chat — ✅ IMPLEMENTADO e CONFIRMADO (com 2 bugs corrigidos
  ao vivo) — 2026-08-20**: usuário viu um recurso parecido na própria interface do Claude Code
  (print) e perguntou se dava pra fazer no Cerne — uma faixa fina na borda esquerda do chat com um
  traço por mensagem SUA (não do assistente), clique rola até aquela mensagem.
  `ChatView.vue::userMessageMarkers` (computed, filtra `timeline` por `kind==='message' &&
  message.role==='user'`) + `messageEls` (`Map<string, HTMLElement>` populado via template ref
  funcional em cada bolha de usuário) + `scrollToMessage` (`el.scrollIntoView({behavior:'smooth'})`).
  - **Bug 1 (achado testando ao vivo)**: a faixa cobria a altura de `.chat-layout` INTEIRO
    (barra de topo + área de mensagens + composer) — com só 2 mensagens, uma ficava quase atrás da
    barra de topo e a outra quase atrás do composer, praticamente invisível. Corrigido: novo
    wrapper `.chat-scroll-wrap` (flex:1, position:relative) envolve `.chat-scroll` E a faixa de
    marcadores como irmãos — a faixa agora cobre exatamente a altura real da área de mensagens,
    não a tela toda.
  - **Bug 2 (achado no mesmo teste): clique não rolava até a mensagem**. Causa raiz: o template ref
    tentava pegar `.$el` da instância do componente `MessageBubble` — mas `MessageBubble.vue` tem
    DOIS nós raiz desde a Fase do botão-ao-selecionar-texto desta mesma sessão (`<div class="row">`
    + `<Teleport to="body">` do popup de leitura em voz alta da seleção), o que torna `.$el` num
    componente multi-raiz não confiável (pode apontar pro anchor do Teleport, não pro conteúdo
    real). Corrigido: o `ref` agora vai numa `<div>` comum que ENVOLVE o `<MessageBubble>`, em vez
    de tentar pegar o elemento do componente diretamente — `ref` num elemento HTML nativo sempre dá
    o nó DOM de verdade, sem ambiguidade de multi-raiz.
  - Também trocado de "espalhado proporcionalmente pela altura toda" pra uma pilha compacta (gap
    fixo de 6px, um logo abaixo do outro) — pedido do usuário depois de ver o primeiro resultado
    com as marcações longe umas das outras sem necessidade.
  - `vue-tsc --noEmit` limpo em cada etapa.

---

## Pendentes (planejadas, não implementadas)

### Prioridade Alta

| # | Tarefa | Descrição | Complexidade |
|---|--------|-----------|-------------|
| T16 | Ferramenta `create_pptx` | ~~Criar/editar arquivos PPTX. Copiar lógica do python-pptx para Rust (ou usar crate `pptx`). Incluir: criar slides, adicionar texto/imagens/tabelas, formatar. Similar ao `create_excel`/`create_word`/`create_pdf` que já existem.~~ ✅ IMPLEMENTADO — 2026-08-16, ver detalhe abaixo. | Média |
| T17 | IA cria ferramentas Python customizadas (UV manager) | ~~O LLM pode criar scripts Python como "ferramentas" que ficam disponíveis em sessões futuras. Fluxo: LLM gera script → salva em pasta de tools do Cerne → gera instruções de uso (SKILL.md automático) → inclui nas instruções de novas sessões. Usar UV para gerenciar dependências Python. O usuário vê as ferramentas criadas na UI e pode editar/remover.~~ ✅ IMPLEMENTADO — 2026-08-16, ver detalhe abaixo. | Alta |
| T39 | Fix: falso positivo de doom loop em polling legítimo (`check_background_output`) | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Baixa |
| T40 | Mover badges de C1/C2 pra painel flutuante (hoje cobrem o composer) | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Média |
| T41 | `FileBrowser.vue`: "Trocar pasta" deve oferecer as pastas já selecionadas na sessão, não só diálogo do SO | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Baixa |
| T42 | `RepoDiffViewer.vue`: mesmo ajuste do T41 + diffs não aparecem | ✅ CORRIGIDO — 2026-08-16 (causa raiz real encontrada). Ver detalhe abaixo. | Média |
| T43 | Bug: chat sempre usa porta 8082 do llama.cpp, ignora a porta do fork selecionado | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Baixa |
| T44 | Fix: painéis flutuantes (C1/C2) esticavam/quebravam o layout quando abertos | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Baixa |
| T45 | Bug: sub-agente (`task`) nunca tinha `web_search`/`web_fetch`/`load_skill`/`todo_list` | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Baixa |
| T46 | Bug: `web_search` falhando ("todas as fontes indisponíveis") quando roda em paralelo com sub-agentes | ✅ CORRIGIDO — 2026-08-16. Ver detalhe abaixo. | Baixa |

#### Detalhe de T39-T42 — achados testando C1/C2/D1/D2 ao vivo (2026-08-15)

> Usuário testou os painéis novos na build de produção. Registrando os ajustes pedidos — **nada
> implementado ainda**, ficou pra próxima sessão.

**T39 — Falso positivo de doom loop em polling legítimo**
- Achado: ao testar o job em background (item 5), o LLM chamou `check_background_output` 3 vezes
  seguidas com os MESMOS argumentos (`{"id": "..."}`) enquanto esperava o processo terminar — isso é
  uso normal (polling), mas `is_doom_loop`/`DOOM_LOOP_THRESHOLD` (`agent/mod.rs:31`, hoje `3`) tratou
  como loop de verdade e abortou o turno ("⚠️ Parei a execução...").
- Efeito colateral (T40 abaixo): como o turno abortou, os processos em background e a execução do
  sub-agente ficaram "presos" aparecendo nos painéis (nunca chegaram a `done`/`exited` de fato, do
  ponto de vista do turno, mesmo o processo real já tendo terminado).
- **Correção aplicada (2026-08-16)**: nova constante `DOOM_LOOP_EXEMPT_TOOLS = ["check_background_output", "list_background"]`
  em `agent/mod.rs` — essas chamadas não entram mais na janela `recent_calls` usada por `is_doom_loop`
  (nos 3 pontos que alimentam essa janela: loop principal, `subagent.rs`, `verifier.rs`). Ferramentas
  que MUDAM estado continuam sujeitas ao threshold normal de 3. `cargo check --lib`/`--tests` limpos.

**T40 — Badges de C1/C2 cobrindo o composer — CORRIGIDO (2026-08-16)**
- `BackgroundJobsPanel`/`AgentExecutionsPanel` saíram de dentro do `.composer-wrap` e foram pra um
  novo `<div class="floating-status">` — sibling do `.composer-wrap` dentro de `.chat-layout`,
  `position: absolute; top: 12px; right: 16px` (canto superior direito, fora do fluxo do composer).

**T41 — `FileBrowser.vue`: "Trocar pasta" deveria oferecer as pastas já selecionadas — CORRIGIDO (2026-08-16)**
- Novo componente compartilhado `FolderPicker.vue`: dropdown com `project_root` + cada
  `extra_read_paths` da sessão, mais um item "Escolher outra pasta..." que abre o diálogo nativo do
  SO. `FileBrowser.vue` trocou o botão "Trocar pasta" + texto do caminho por esse componente.

**T42 — `RepoDiffViewer.vue`: mesmo ajuste do T41 + diffs não aparecem — CORRIGIDO (2026-08-16)**
- Mesmo `FolderPicker.vue` do T41 aplicado em `RepoDiffViewer.vue` (era mesmo o mesmo componente,
  reaproveitado — não precisou duplicar lógica).
- **Causa raiz real encontrada** (a teoria de "pasta errada" de fato não se sustentava, como já
  suspeitado): edições feitas por um sub-agente (dentro de `task`/`run_pipeline`, ver Fase 3) nunca
  entravam em `sessionStore.tasks` — esse array só recebe as tool calls do LOOP PRINCIPAL
  (`sessions::save_tasks`, só chamado em `agent/mod.rs`). As chamadas do sub-agente Dev existiam só
  como evento efêmero de UI (visível ao vivo, perdido depois que a sessão recarregava) — exatamente
  o tipo de sessão que a Fase 3/pipeline gera bastante. `RepoDiffViewer.vue` lia só
  `sessionStore.tasks`, então nunca via essas edições.
- **Correção**: reaproveitando a infraestrutura do T14/Fase 3 (`AgentExecution.steps`, gravado desde
  a correção de "histórico de execuções" mais cedo nesta sessão), `RepoDiffViewer.vue` agora também
  busca `api.listAgentExecutions()` (filtrado pela sessão atual), achata `.steps` de todas as
  execuções, e soma isso a `sessionStore.tasks` antes de agrupar por arquivo. Precisa reabrir o
  modal (ou trocar de sessão) pra buscar de novo — não há push automático quando uma execução nova
  aparece, mesmo padrão de "busca ao abrir" já usado noutros lugares.
- `npx vue-tsc --noEmit` limpo. **Não testado na janela real** (implementado sem supervisão, ver
  nota geral desta sessão).

**T43 — Bug: chat sempre chama a porta 8082, ignora a porta do fork selecionado**
- Achado pelo usuário (2026-08-15): fork "Mainline" sobe o `llama-server` na porta 8083 (confirmado
  — `ensure_llama_ready`/`providers::llama_cpp::start_server` usam `fork.port` corretamente pra
  INICIAR o processo, `llama_cpp.rs:232-248`), mas o chat em si tenta falar com `127.0.0.1:8082` e
  falha — porta errada, servidor não está lá.
- **Causa raiz encontrada**: `build_provider_config` (`lib.rs:99-134`) monta a `base_url` da chamada
  de chat assim: `ProviderKind::LlamaCpp => cfg.llama_cpp_base_url.clone()` (`lib.rs:130`) — usa
  SEMPRE o valor fixo `AppConfig.llama_cpp_base_url`, cujo default é `"http://127.0.0.1:8082/v1"`
  (`models.rs:216`). A função **nem recebe** o `fork_id`/`session.llama_fork` como parâmetro —
  ignora completamente qual fork está selecionado na sessão. Os 3 call sites (`lib.rs:160`,
  `lib.rs:218`, `lib.rs:657`, `agent/mod.rs:1422`) também não passam o fork adiante.
- **Correção aplicada (2026-08-16)**: `build_provider_config` ganhou um parâmetro `fork_id: Option<&str>`
  e um branch dedicado pra `LlamaCpp` — carrega `LlamaForkConfig` de verdade
  (`providers::llama_cpp::load_forks`) e monta `base_url` como `http://127.0.0.1:{fork.port}/v1`,
  nunca mais o `cfg.llama_cpp_base_url` fixo. `fork_id: None` cai em `cfg.active_llama_fork`.
  `provider_config_for` (usado pelo `run_turn`) ganhou o mesmo parâmetro e passa
  `session.llama_fork.as_deref()` no call site que importa (`agent/mod.rs:550`, o do chat de
  verdade — antes usava o fork certo só pra INICIAR o servidor via `ensure_llama_ready`, nunca pra
  montar a URL da chamada). `subagent::run`/`verifier::run`/compactação reusam o mesmo `cfg` já
  corrigido, não precisaram de mudança própria. Os outros 3 call sites (`list_provider_models`,
  `resolve_context_length`, `test_vision`) passam `None` — nenhum dos três de fato exercia esse
  caminho na prática (listagem de modelos usa `list_llama_presets` separado, que já lia `fork.port`
  certo desde sempre), mas ganham o mesmo fallback correto de qualquer forma. `cargo check --lib` e
  `cargo check --tests` limpos. **Não testado na janela real ainda** — build novo pendente.

**T44 — Painéis flutuantes esticando/quebrando quando abertos — CORRIGIDO (2026-08-16)**
- Achado pelo usuário: com o badge fechado (T40) ficou bonito, mas ao abrir (lista expandida OU o
  modal de detalhe de uma execução) a área esticava e quebrava o layout.
- **Causa provável**: clássica pegadinha de flexbox — `.floating-status` é `display:flex;
  flex-direction:column` e seus filhos (`.bg-jobs`/`.ae-panel`, e dentro deles `.bg-jobs-list`/
  `.ae-list`) não tinham `min-width:0`, então não encolhiam pra caber no `max-width:320px` do pai
  quando o conteúdo (nome de execução longo, comando longo) pedia mais espaço — o bloco crescia pra
  fora do container ancorado à direita, "vazando" por cima do chat.
- **Correção aplicada**: `min-width:0`/`max-width:100%` nos wrappers, `.bg-jobs-list`/`.ae-list`
  ganharam `max-height:280px` + `overflow-y:auto` (não crescem mais indefinidamente), `.bg-job-row`/
  `.ae-row` ganharam `max-width:320px` (força a elipse do nome funcionar de verdade), os dois
  `Dialog` de detalhe ganharam `maxWidth:'92vw'` (nunca mais largos que a tela), `.ae-steps` ganhou
  `max-height:200px` com scroll próprio, e `overflow-wrap:anywhere` nos textos de passo pra strings
  sem espaço (tipo comando `node -e "..."` de uma linha só) quebrarem em vez de forçar largura.
  `.floating-status` também ganhou teto de altura (`max-height: calc(100% - 24px)`) com scroll.
- `vue-tsc --noEmit` limpo. **Não testado na janela real ainda**.

**T45 — Bug: sub-agente sem `web_search`/`web_fetch`/`load_skill`/`todo_list` — CORRIGIDO (2026-08-16)**
- Achado pelo usuário: `task` de teste ("buscar capital do Brasil e maior rio") reportou "as
  ferramentas disponíveis não incluem busca na web ou acesso à internet", mesmo a sessão tendo um
  projeto configurado (então não era o palpite de "falta pasta").
- **Causa raiz encontrada**: `subagent_tool_specs()` (`agent/subagent.rs`) montava o toolset do
  sub-agente só com `tools::project_tool_specs()` — nunca incluía `tools::always_tool_specs()`
  (onde `web_search`/`web_fetch`/`load_skill`/`todo_list`/`ask` vivem). O loop principal
  (`agent/mod.rs:629-631`) monta o toolset da sessão como `always_tool_specs() + project_tool_specs()`
  — o sub-agente só copiava a segunda metade.
- **Correção aplicada**: `subagent_tool_specs()` agora monta `always_tool_specs()` (menos `ask` —
  sub-agente não pode pausar esperando o usuário, travaria a fila) + `project_tool_specs()` (menos
  `task`/`verify_completion`, guarda de profundidade). Teste `subagent_toolset_excludes_task_but_keeps_everything_else`
  atualizado pra virar regressão de verdade (antes comparava só contra `project_tool_specs()`, não
  pegava esse bug — agora tem asserções específicas de que `web_search`/`web_fetch`/`load_skill`/
  `todo_list` estão presentes e `ask` está ausente).
- `cargo check --lib`/`--tests` limpos (execução de `cargo test` continua bloqueada pelo mesmo
  problema de ambiente desta máquina, ver notas anteriores — não é o código).

**T46 — Bug: `web_search` falhando em paralelo com sub-agentes — CORRIGIDO (2026-08-16)**
- Achado pelo usuário testando o prompt de "5 chamadas simultâneas": `web_search` falhava
  ("todas as fontes de busca falharam") quando rodava no mesmo turno que `task`s em paralelo, mas
  funcionava chamado sozinho ("se chamar direto funciona, mas por skill falhou").
- **Causa raiz**: o modo `Auto` de busca (`websearch.rs::search_auto`) usa 3 fontes SEM API oficial
  (scraping: DuckDuckGo, página pública do Brave, Mojeek) em paralelo via `tokio::join!` — e
  `search_many` já roda várias queries da MESMA chamada em paralelo também. Com os fixes T32 (task
  em paralelo) e T45 (sub-agente ganhou acesso a `web_search`) do mesmo dia, um turno agora pode
  disparar o `web_search` do agente principal AO MESMO TEMPO que 2+ sub-agentes também chamam
  `web_search` — multiplicando rápido quantas requisições simultâneas batem nas mesmas 3 fontes
  gratuitas, aumentando muito a chance de bloqueio anti-bot (scraping sem chave é frágil por
  natureza a rajadas).
- **Correção aplicada**: `search_auto_semaphore()` — `tokio::sync::Semaphore` estático com 2 vagas,
  adquirido no início de `search_auto`. Chamadas em excesso ficam na fila em vez de disparar tudo de
  uma vez; não afeta a concorrência real de `task` (só a parte de busca na web que passa por ele).
  Só afeta o modo `Auto`/keyless — Brave/Tavily via API key e SearXNG (auto-hospedado) não precisam
  desse throttling.
- `cargo check --lib`/`--tests` limpos. **Não testado na janela real ainda** — build novo pendente.

### Prioridade Média

| # | Tarefa | Descrição | Complexidade |
|---|--------|-----------|-------------|
| T11 | Visualização de agentes/ferramentas no chat | ~~Quando o agente usa sub-agentes (`task`) ou verificador (`verify_completion`), mostrar no chat de forma visual (card expansível). Ao clicar, abre modal com detalhes completos: quais ferramentas foram usadas, output de cada uma, tempo gasto, resultado final.~~ **Superado pela Fase C2** (`AgentExecutionsPanel.vue`, implementado 2026-08-14) — já cobre esse caso de uso, com painel dedicado em vez de card inline no chat. | Alta |
| T12 | Painel lateral flutuante de arquivos | ~~Botão no canto superior direito abre painel flutuante com: dropdown para escolher pastas da sessão, árvore de arquivos expandível, botão "+" para enviar conteúdo de um arquivo pro chat (como anexo textual), botão "x" para fechar o painel.~~ **Superado pela Fase D2** (`FileBrowser.vue`/`FileTreeNode.vue`, implementado 2026-08-15) — mesma ideia, já entregue. | Alta |
| T29 | Pastas na lista de sessões (2 níveis, expansível/recolhível) | ~~Ver detalhe granular abaixo.~~ ✅ IMPLEMENTADO — 2026-08-16, ver detalhe abaixo (agora na seção "Implementadas nesta sessão"). | Média |

#### Detalhe de T29 — Pastas na lista de sessões (implementado)

> Pedido pelo usuário em 2026-08-14, olhando a lista de sessões (`Sidebar.vue`) crescida demais pra rolar.
> Estrutura: pasta pode conter sessões e subpastas; subpasta só pode conter sessões (**2 níveis, sem
> pastas dentro de pastas dentro de pastas**). Cada pasta tem uma seta expandir/recolher, escondendo o
> conteúdo — mesma ideia de um explorador de arquivos comum.

**Backend [BE]** — tudo implementado conforme o desenho original:
- `folders.rs` novo módulo: `Folder{id,name,parent_id}` (struct em `models.rs`), persistência em `{app_data_dir}/folders.json`, mesmo padrão de `personas.rs`. `create_folder` recusa `parent_id` apontando pra uma pasta que já tem `parent_id` próprio (erro claro, não silencioso) — impede o 3º nível.
- `Session` ganhou `folder_id: Option<String>` (`#[serde(default)]`, retrocompatível — sessões existentes continuam soltas na raiz).
- Comandos Tauri: `list_folders`, `create_folder`, `rename_folder`, `delete_folder` (devolve os ids das subpastas que ficaram órfãs, pra UI atualizar a árvore local sem recarregar tudo), `update_session_folder` (espelha `update_session_persona`). `delete_folder` move conteúdo pra raiz automaticamente, sem confirmação extra — decisão já recomendada no roteiro original, mantida.
- `sessions::create_session` NÃO ganhou parâmetro `folder_id` (evitaria tocar numa assinatura já com 7 parâmetros usada em vários lugares) — em vez disso, "nova sessão dentro da pasta X" cria a sessão normal e chama `update_session_folder` logo em seguida (`App.vue::createNewSession(folderId)`).
- 5 testes novos em `folders.rs` (roundtrip, subpasta válida, subpasta-de-subpasta rejeitada, rename, delete move filhos pra raiz).

**Frontend [FE]** — implementado com uma redução deliberada:
- `api.ts`: tipo `Folder` + wrappers; `Session.folder_id` adicionado à interface.
- `stores/session.ts`: estado `folders: Folder[]`, ações `loadFolders`/`createFolder`/`renameFolder`/`deleteFolder`/`moveSessionToFolder`. Carregado uma vez no `onMounted` do `App.vue`, junto de `loadSessions`.
- `Sidebar.vue`: árvore de verdade — pastas de raiz (ordenadas por nome) primeiro, cada uma com seta expandir/recolher (estado em `localStorage`, chave `cerne-sidebar-expanded-folders`, não vai pro backend), depois sessões soltas. Criar pasta é inline (mesmo padrão de renomear sessão que já existia: clica, aparece um input no lugar, confirma no Enter/blur). Botões de ação aparecem no hover da linha da pasta: nova sessão aqui, nova subpasta (só em pasta de raiz — subpasta não tem esse botão), renomear, excluir.
- **Extraído `SidebarSessionItem.vue`** (novo componente pequeno) pra não triplicar a lógica de renomear/excluir/mover em 3 lugares da árvore (raiz, dentro de pasta, dentro de subpasta).
- Busca: digitar no campo de busca "achata" a árvore — mostra sessões soltas ignorando a hierarquia de pastas, exatamente como o roteiro original sugeria.
- **Redução deliberada**: mover sessão entre pastas usa um botão "mover para" com uma lista suspensa (não drag-and-drop). O roteiro original já cogitava isso como a alternativa mais simples pra uma primeira passada, avaliando drag-and-drop como próxima iteração se valer a pena — mantive essa recomendação em vez de implementar DnD sem poder testar interação de mouse na janela real.

**Perguntas do roteiro original, resolvidas na implementação**:
- Ordenação de pastas: por nome (mais simples e previsível; não por "uso recente" como as sessões soltas).
- Excluir pasta com conteúdo: move tudo pra raiz automaticamente, sem confirmação extra além do próprio diálogo de "excluir pasta?" — reversível manualmente via "mover para".

**Não testado na janela** — feito sem build, a pedido do usuário. `cargo check --lib`/`--tests` e `npx vue-tsc --noEmit` (typecheck do Vue/TS, não gera build) limpos.

### Prioridade Baixa

| # | Tarefa | Descrição | Complexidade |
|---|--------|-----------|-------------|
| T13 | Agentes e Skills podem usar MCPs do usuário | ✅ CORRIGIDO — 2026-08-16. **Achado**: `subagent.rs::run` já tinha MCP, mas SEM filtrar por `session.enabled_mcp_servers` (usava TODOS os servidores globais, ignorando o toggle por sessão do composer). Corrigido: `subagent::run` ganhou parâmetro `enabled_mcp_servers: Option<&[String]>`, filtra igual o loop principal já faz. `verifier.rs` continua sem MCP de propósito (decisão já existente, documentada no código — verificador é só leitura/observação). `cargo check --lib`/`--tests` limpos. | Baixa |

---

## Fases futuras (do roteiro `13_roteiro_agentes_skills_fases.md`)

### Parte I — papéis e orquestração

| Fase | Nome | Status |
|------|------|--------|
| Fase 3 | Pipeline Dev → QA → Analista (orquestração determinística) | ✅ IMPLEMENTADA — 2026-08-16, ver detalhe abaixo. `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos; `cargo test` não roda nesta máquina. Não testado na janela real |
| Fase 4 | Skills de produtividade (file_organizer, email_triage, english_tutor) | ✅ IMPLEMENTADA — 2026-08-16, ver detalhe abaixo. `cargo check --lib`/`--tests` limpos. Não testado na janela real |
| Fase 5 | Multi-agente gerenciado (gerente dinâmico) | ✅ IMPLEMENTADA — 2026-08-16, skill `project-manager` seedada. `cargo check --lib`/`--tests` limpos |
| Fase 6 | Skill Store / Comunidade | ✅ IMPLEMENTADA (Degrau 1) — 2026-08-16, ver detalhe abaixo. `cargo check --lib`/`--tests` e `vue-tsc --noEmit` limpos |

### Parte II — infraestrutura de execução e UX (adicionada em 2026-08-14)

| Fase | Nome | Status |
|------|------|--------|
| Fase A | Infraestrutura de execução (UUID, isolamento, fila local/paralelo API, YOLO/Manual p/ agentes) | ✅ Concluída (com reduções pontuais) — A1/A4/A6 concluídos (T27/T32/T30), A2 quase concluído (T25+T28, falta só o listener de UI que abre o canal por execução), A5 concluído reduzido (T31, sem "um a um"), A3 parcial (Personas/T24, sem tools/model/skills por agente) |
| Fase B | Catálogo de Agentes/Skills (tela) + import picoClaw + criação de agente próprio | ✅ Concluída (reduzida) — B1 (T36), B2 (T47, reduzido) e B3 (2026-08-16, reduzido: Persona ganhou allowlist de skills além de tools; `model`/`max_turns` continuam adiados por risco) concluídos. Ver detalhe do B3 abaixo |
| Fase C | Painel de execução em tempo real (background tasks expandido + sessão de agente/skill) | ✅ Concluída (com gaps documentados) — C1 (T34) e C2 (T35); falta cancelamento individual e posição na fila (escopo de A4) |
| Fase D | Diff de repositório + navegador de arquivos clicável | ✅ Concluída — D1 (T38) e D2 (T37) |
| Fase E | Composer (fable/skill, atalho `/`, modal MCPs, ícone de visão, copiar código) | ✅ Concluída — 2026-08-16. E1 (investigação, sem reorg forçada), E2 (atalho `/`), E3 (modal único de MCPs + desabilitados por padrão), E4 (ícone de visão com cache), E5 (prompt de 1 comando por bloco). Ver detalhe abaixo |
| Fase F | Modais de Ajuda/Configurações/Sobre (Settings deixa de ser view) | ✅ Concluída e testada (T33) |

---

## Repositórios clonados para referência

Todos em `F:\AgentesESkills\` (pode excluir quando quiser):

| Repo | O que tem de útil |
|------|------------------|
| MetaGPT | Prompts de PM/Architect/Engineer/QA, pipeline pub/sub |
| CrewAI + examples | Formato YAML declarativo de agentes, output estruturado Pydantic |
| AutoGen | Termination conditions (token budget, timeout), GroupChat patterns |
| PR-Agent | ⭐ Prompt de code review completo, schema YAML, ticket compliance |
| Open Interpreter | AGENTS.md real, regras de contexto (10K cap) |
| Composio | 17 skills SKILL.md reais, formato validado |
| awesome-ai-agents | Lista curada de 300+ recursos |
| picoClaw (Sipeed) | ⭐ Formato SKILL.md idêntico ao Cerne, `AgentFrontmatter`, `SubTurnConfig` (sub-turno síncrono/assíncrono com profundidade/concorrência limitada) — referência principal da Fase A |

---

## Análise detalhada dos repos

Ver `PLANOS/13_roteiro_agentes_skills_fases.md` → seção "Análise Detalhada dos Repositórios"

---

## Ideias futuras (ainda não detalhadas em tarefas)

> Coisas que valem registrar pra não esquecer, mas que precisam de mais pensar antes de virarem
> tarefa granular como as acima — geralmente porque têm uma decisão de arquitetura em aberto.

### Modelo de visão separado (fallback pra imagem quando o modelo principal não vê)

Pedido pelo usuário em 2026-08-17. Ideia central: hoje, quando o modelo ativo da sessão NÃO suporta
visão (`providers::supports_vision`, já usado pro ícone de visão do composer, E4), duas coisas ruins
acontecem — (1) se o usuário anexa uma imagem na mensagem, ela é **descartada silenciosamente**
(`agent/mod.rs`: "Modelo sem visão: remove imagens do histórico pra não enviar multimodal data que o
provider rejeitaria") — o modelo nunca fica sabendo que uma imagem existia; (2) `computer_use_*` que
depende de screenshot fica bloqueado com erro direto ("computer_use requer um modelo com suporte a
visao"). Pedido: em vez de descartar/bloquear, deixar o usuário escolher um **modelo de visão
dedicado** (local ou API, independente do modelo principal da conversa) — quando o modelo principal
não vê, o Cerne manda a imagem + uma pergunta pra esse modelo de visão, pega a resposta em TEXTO, e
devolve isso pro modelo principal continuar (ele nunca vê a imagem crua, só a descrição em texto).

**Os dois gatilhos que passariam a funcionar em vez de descartar/bloquear**:
- Usuário anexa imagem numa sessão com modelo sem visão → em vez de descartar, manda a imagem +
  (a pergunta do usuário, ou um prompt genérico tipo "descreva esta imagem em detalhes, incluindo
  qualquer texto visível") pro modelo de visão configurado, injeta a resposta como texto no
  histórico ANTES de mandar pro modelo principal.
- `computer_use_screenshot` (e qualquer outra tool de `computer.rs` que dependa de imagem) numa
  sessão sem visão → em vez de recusar a chamada, roteia o screenshot pro modelo de visão, devolve a
  descrição como se fosse o "OUT" da tool.

**O que já existe pra reaproveitar**: `providers::supports_vision`/`chat_stream` já sabem montar e
mandar mensagem multimodal pra qualquer provider (usado pelo E4, testado contra o modelo ativo da
sessão) — o mecanismo de "mandar imagem + pergunta, receber texto" é o MESMO fluxo de uma chamada
de chat normal, só que de um turno curto e descartável (parecido com `subagent.rs`: mensagem única,
sem histórico persistido, só a resposta final importa). `ProviderPicker.vue`/`CustomProviderConfig`
já resolvem "usuário escolhe provider+modelo" pra outros contextos (fork de llama.cpp, provider
customizado) — o seletor de modelo de visão reaproveitaria o mesmo componente.

**Decisões a tomar antes de implementar**:
- Config GLOBAL (`AppConfig`, tipo `vision_provider`/`vision_model`/`vision_fork_id`/
  `vision_custom_provider_id`) ou por sessão? Provavelmente global — é mais uma configuração de
  infraestrutura ("qual modelo eu tenho disponível pra ver imagem") do que uma escolha por
  conversa, mesmo espírito de "Endpoints locais" em Configurações.
- Custo/latência: cada imagem vira uma chamada de API EXTRA (roundtrip pro modelo de visão antes do
  modelo principal responder) — se o modelo de visão for local (ex: um modelo pequeno com mmproj),
  bate na mesma limitação de "uma GPU só, uma coisa por vez" se a sessão principal TAMBÉM for local;
  se for API paga, é custo real por imagem que o usuário devia poder desligar facilmente (nem todo
  mundo vai querer pagar duas chamadas por mensagem com imagem).
- O que exatamente perguntar pro modelo de visão: usar o texto que o usuário escreveu junto da
  imagem como a pergunta (mais preciso, mas se o usuário só mandou a imagem sem texto, precisa de um
  prompt default) ou sempre um prompt genérico de "descreva tudo" (mais seguro, mas pode perder
  nuance do que o usuário queria saber especificamente)?
- Aplicar também quando o modelo principal JÁ suporta visão, mas o usuário quer um modelo de visão
  mais barato/especializado de propósito (ex: rodar tudo em Claude mas usar um modelo local só pra
  OCR de screenshot)? Ficaria mais flexível, mas também mais uma decisão de "quando usar qual" — a
  v1 provavelmente só ativa o fallback quando o principal genuinamente não vê, não como escolha de
  custo.

### Ler o chat em voz alta (text-to-speech)

**✅ IMPLEMENTADO (escopo reduzido) — 2026-08-17.** Pedido explícito do usuário, autorizando
trabalho autônomo durante a noite ("pode implementar na ordem de importância que você achar
relevante" + resposta reduzindo o escopo da Voz completa pra só "ler texto, e falar texto" —
ver seção "Voz completa" abaixo pro que ficou de fora). Implementado:
- **Backend (`src-tauri/src/audio.rs`, novo módulo)**: `synthesize_speech(api_key, text, model)`
  chama `POST https://openrouter.ai/api/v1/audio/speech` (endpoint confirmado na documentação
  oficial da OpenRouter, consultada ao vivo em 2026-08-17 — nunca usado no app antes de hoje).
  Devolve os bytes de áudio (mp3) já em base64. Comandos Tauri `tts_speak(text)` (usa a chave
  OpenRouter já configurada, erro claro se não tiver) registrados em `lib.rs`.
- **Modelo default: `openai/gpt-4o-mini-tts-2025-12-15`, NÃO Kokoro** — mudança de plano
  importante: a documentação OFICIAL da OpenRouter (consultada ao vivo, 2026-08-17) lista só 4
  modelos no endpoint de TTS (`openai/gpt-4o-mini-tts-2025-12-15`,
  `mistralai/voxtral-mini-tts-2603`, `microsoft/mai-voice-2`, `fish-audio/s2.1-pro`) — Kokoro
  NÃO aparece nessa lista, apesar da pesquisa de preço registrada abaixo (2026-08-17, mesma
  sessão) ter achado uma página de modelo `hexgrad/kokoro-82m` no catálogo geral da OpenRouter.
  Pode ser que Kokoro exista no catálogo mas não esteja habilitado nesse endpoint específico de
  áudio, ou a doc oficial estar desatualizada — não dava pra confirmar sem testar contra uma
  chave real (não fiz nenhuma chamada de teste ao vivo). Usei o modelo OFICIALMENTE documentado
  como default pra não arriscar quebrar a feature inteira com um model slug não confirmado —
  **vale testar se `hexgrad/kokoro-82m` funciona nesse endpoint amanhã, e trocar o default em
  `audio.rs::DEFAULT_TTS_MODEL` se for mais barato e funcionar.**
- **Frontend**: botão de voz (ícone `volume_up`) em cada mensagem do assistente
  (`MessageBubble.vue`) — **NÃO é o "botão flutuante ao selecionar trecho de texto" do pedido
  original**, é um botão fixo que lê a mensagem inteira. Escolha deliberada: um botão fixo por
  mensagem é muito mais simples e testável sem risco (a lógica de detectar seleção de texto e
  posicionar um botão flutuante certo tem mais chance de sair errada sem poder testar ao vivo).
  Se o usuário realmente quiser o botão-ao-selecionar depois de testar este, é uma iteração
  pequena a partir daqui.
- **Não testado contra uma chave real** — é a primeira vez que o app fala com esse endpoint.

**✅ CONFIRMADO E AMPLIADO NA JANELA REAL — 2026-08-19/20.** TTS e STT testados de ponta a ponta
pelo usuário, com vários problemas reais encontrados e corrigidos ao vivo:

- **`openai/gpt-4o-mini-tts-2025-12-15` não existe mais na OpenRouter** (usuário mandou print da
  própria página de modelos filtrada por "Speech" — só aparecia em "Transcription"). Trocado o
  default pra **`hexgrad/kokoro-82m`** (confirmado ao vivo na doc: existe, $0,62/M caracteres, o
  mais barato listado). Descoberto junto: a OpenRouter exige `voice` explícito pra providers sem
  voz default própria (Kokoro é um desses) — sem isso a chamada seria rejeitada. Voz default
  `af_heart`, configurável.
- **Timeout de 30s era curto demais** — Kokoro via DeepInfra (serverless) pode ter cold start de
  dezenas de segundos; aumentado pra 90s.
- **Erro de rede sem causa raiz**: `reqwest::Error` sozinho só mostra "error sending request for
  url (...)", cortando a causa de verdade — `describe_reqwest_error` (`audio.rs`) agora percorre a
  cadeia `source()` inteira, foi assim que se descobriu que era `operation timed out`.
- **Configurável em Configurações → "Voz"**: provider+modelo+voz pra TTS e STT, cada um
  independente (reaproveita `ProviderPicker`-like: OpenRouter/llama.cpp+fork/Ollama/LM
  Studio/custom). Endereça os itens 4/5 da lista "Voz completa" abaixo (escolha de modelo
  destravada, sem depender de nomenclatura tipo "Whisper"/"Kokoro" forçada — mas também não
  esconde o nome do modelo, já que o campo é texto livre).
- **Detecção automática de idioma pra TTS** (pedido do usuário: "está em inglês, tem como falar na
  língua correta"): `audio::detect_language` — heurística leve (palavras comuns pt/en/es + faixa
  Unicode CJK pra zh), sem baixar nenhum modelo de detecção. Com o modelo Kokoro, troca a voz
  automaticamente por idioma (`pf_dora` pt, `ef_dora` es, `zf_xiaobei` zh, `af_heart` en/default) —
  só tem efeito com Kokoro (única conexão com esse mapeamento voz↔idioma); desligável (checkbox
  "detectar idioma automaticamente"). 9 testes unitários cobrindo a detecção e o mapeamento.
- **Botão "Ler" ao selecionar texto — item 1 da lista de ideias do usuário abaixo, IMPLEMENTADO**
  (pedido explícito: "selecionar um texto, e aparecer um autofalante no final da seleção pra
  aprender inglês"): `MessageBubble.vue` escuta `selectionchange` global, cada instância confere
  se a seleção está dentro do PRÓPRIO texto (`contentEl`), e mostra um botão circular flutuante
  (`Teleport to="body"`, `position: fixed` na borda da seleção) que lê só o trecho selecionado.
  `@mousedown.prevent` no botão evita que clicar nele colapse a seleção antes do `@click` disparar
  (técnica padrão de "toolbar de seleção").
- **Integração com Voicebox local** (github.com/jamiepine/voicebox) — pedido do usuário depois de
  achar o app e perguntar se dava pra usar. Investigação real (não suposição): li o
  `/openapi.json` do servidor rodando local na máquina do usuário — API própria, INCOMPATÍVEL com
  o formato OpenAI (`POST /speak` devolve JSON com `status`/`id`, não áudio direto; precisa
  acompanhar via SSE `GET /generate/{id}/status` até `completed`; STT é multipart, não JSON+base64).
  Novo módulo `voicebox.rs` fala esse protocolo próprio. Configurável como "Motor" alternativo
  (por TTS e STT, independente) em Configurações → Voz, ao lado do "API" (OpenAI-compatible)
  já existente — `VoiceBackend::OpenaiCompatible | Voicebox` no `AppConfig`. Criado (via API, sem
  precisar gravar áudio) um perfil de voz "Cerne Code" no Voicebox do usuário (`voice_type:
  "preset"`, engine Kokoro, voz `af_heart`) pra testar.
- **Bug real encontrado testando: fala duplicada**. Causa raiz confirmada por eliminação
  (chamando `/speak` direto por `curl`, sem o Cerne no meio): o Voicebox tem uma opção própria
  "Autoplay on generate" (ligada por padrão) que já toca o áudio sozinho assim que termina de
  gerar — o Cerne baixava esse MESMO áudio e tocava de novo, as duas reproduções ficavam
  desencontradas no tempo soando como eco. Correção: quando o backend é Voicebox, o Cerne agora só
  DISPARA a geração e espera terminar (`voicebox::speak`, sem baixar bytes) — quem toca é só o
  Voicebox. `TtsResult.play_locally: bool` no retorno do comando `tts_speak` diz pro frontend se
  deve criar/tocar um `<audio>` ou só refletir o estado (o `mime` do áudio também virou dinâmico
  nesse mesmo retorno — Voicebox manda WAV, OpenRouter manda MP3, o frontend não assume mais MP3
  sempre).

Pedido pelo usuário em 2026-08-14, retomado com mais detalhe em 2026-08-17. Poder ouvir o que está
no chat em vez de ler — útil tanto pra praticar inglês (complementa a ideia do `english_tutor` da
Fase 4 do roteiro) quanto pra quando o usuário não quer/não pode ler na tela.

**Terminologia acertada em 2026-08-17**: usuário perguntou por "Whisper barato via OpenRouter" —
Whisper é *speech-to-text* (áudio → texto, ex: transcrição), o OPOSTO do que essa feature precisa
(texto → áudio). O usuário já se corrigiu sozinho ("acho que é Kokoro e outros, né?") — sim,
correto: pra texto→voz os nomes certos são **Kokoro**, Piper, Coqui/XTTS.

**Achado que muda a viabilidade — via API fica bem barato, resolve o problema de GPU**: pesquisei
preço real (2026-08-17). Kokoro (82M, 8 idiomas incl. inglês/espanhol/português, 54 vozes) tá
disponível no **OpenRouter por ~$0.62 por MILHÃO de caracteres** — uma frase de pronúncia (poucas
dezenas de caracteres) custa uma fração de centavo, praticamente de graça no uso real. Isso muda a
recomendação da nota técnica anterior: rodar Kokoro **via API** (OpenRouter ou outro provider que
hospede) evita de vez o problema de "uma GPU só, uma coisa por vez" que preocupava quando a ideia
era rodar localmente — só compete por GPU se o usuário insistir em rodar Kokoro localmente também
(op-in explícito, não o caminho recomendado como default).
- Referência de preço: Kokoro 82M no OpenRouter, [openrouter.ai/hexgrad/kokoro-82m](https://openrouter.ai/hexgrad/kokoro-82m).
- (De brinde, já que a pergunta original foi sobre Whisper: caso o Cerne algum dia queira ENTRADA de
  voz — usuário fala, vira texto — Whisper no OpenRouter também é barato: Whisper-1 padrão a
  $0.006/minuto, Whisper Large V3 Turbo a $0.000003/segundo (~$0.00018/min), e detecta o idioma
  sozinho sem precisar informar. Não é o que foi pedido agora, mas fica registrado pra quando/se
  "falar com o Cerne" virar pedido.)

**Como o usuário imagina disparar (2026-08-17, duas ideias concretas)**:
1. **Botão "Ler" ao selecionar texto** — usuário seleciona um trecho de texto na tela (mensagem do
   assistente, ou qualquer texto), aparece uma opção "Ler" perto da seleção (like um menu de
   contexto de seleção), manda só aquele trecho pro TTS.
2. **Prática de pronúncia sob demanda** — o modelo gera um trecho curto (frase/palavra em inglês
   pra praticar), manda pro TTS, toca o áudio — fluxo pensado especificamente pra aprendizado de
   idioma (liga direto com a Persona `english_tutor` já documentada na ajuda, Fase 4). Diferente de
   "ler a resposta inteira", aqui o INICIADOR pode ser o próprio fluxo de ensino, não só o usuário
   clicando em algo.

**Sobre idioma (pergunta do usuário)**: pra TTS (Kokoro), não tem "auto-detecção" no sentido de
Whisper — a voz escolhida já determina o idioma/sotaque (54 vozes cobrindo os 8 idiomas
suportados), então o Cerne só precisa saber QUE VOZ usar (configurável, ou inferida do idioma da
resposta/da UI) — não precisa adivinhar o idioma do texto separadamente.

**Direções possíveis a avaliar quando for implementar** (atualizado — via API agora é a
recomendação, local vira opção secundária):
- **Via API (recomendado)**: campo em Configurações pra escolher provider+modelo de TTS (mesmo
  componente `ProviderPicker.vue` que a ideia do "modelo de visão" acima também reaproveitaria) —
  OpenRouter/Kokoro como default sugerido pelo preço.
- **Via URL de servidor próprio** (local ou remoto) — mesmo padrão já usado pra custom
  provider/llama fork: o Cerne não empacota nem roda o motor de voz, só fala HTTP com o que o
  usuário já tiver rodando (Piper server, etc.).
- **Web Speech API do navegador/SO** (o usuário já testou — funciona sem instalar nada, mas
  qualidade de voz é mediana/robótica) — fallback "burro" sempre disponível de graça, sem depender
  de nenhum provider configurado.
- Rodar localmente de verdade (Kokoro roda até em CPU, é só 82M parâmetros) continua uma opção, mas
  deixou de ser a única via — só decidir CPU vs GPU se o usuário optar por esse caminho
  especificamente, não como default.
- Escopo de UI: dado que agora tem DOIS gatilhos concretos pedidos (seleção de texto → "Ler"; e
  prática de pronúncia sob demanda), talvez nem precise do modo "ler a sessão inteira" tipo podcast
  cogitado antes — os dois casos de uso reais já não pedem isso.

### Voz completa: entrada por microfone, modo conversa, e reorganização do composer

**Item 1 (microfone) ✅ IMPLEMENTADO — 2026-08-17, CONFIRMADO na janela real em 2026-08-19/20**
(funcionou, sem os riscos de permissão/formato listados abaixo se materializarem) — ver também a
seção "Ler o chat em voz alta" acima pro trabalho grande de STT/TTS feito junto (troca de modelo
default, backend Voicebox local, etc., compartilha a mesma infraestrutura). Itens 2-5 continuam SEM
código, escopo
reduzido a pedido explícito do usuário ("Seria legal focar no ler texto, e no falar texto (mic
pequeno do lado do enviar mensagem do composer) nesse momento"). Implementado: botão de
microfone pequeno no rodapé do `ComposerBar.vue`, ao lado do botão de enviar (exatamente onde o
usuário pediu). Clique inicia gravação via `MediaRecorder` (API do navegador — a Webview do
Tauri no Windows é Chromium/WebView2, que já suporta isso nativamente, sem plugin novo); clique
de novo para e transcreve. Áudio vira base64 (`FileReader.readAsDataURL`) e vai pro novo comando
Tauri `stt_transcribe(audioBase64, format)` (`src-tauri/src/audio.rs::transcribe_audio`, `POST
https://openrouter.ai/api/v1/audio/transcriptions`, modelo default `openai/whisper-1` —
confirmado na documentação oficial). Texto transcrito é ANEXADO ao que já estiver no composer
(nunca substitui, nunca envia sozinho — usuário revisa/edita/manda manualmente, mesmo espírito
do pedido original). **Riscos não testáveis sem a janela real**: (a) permissão de microfone no
WebView2 — o navegador deveria mostrar um prompt nativo pedindo permissão na primeira vez, mas
não dá pra confirmar sem testar; (b) formato `webm` do `MediaRecorder` sendo aceito de verdade
pelo endpoint (está na lista oficial de formatos suportados, mas nunca testado contra uma chave
real). Itens 2 (redesenho do "+"), 3 (modo conversa em tempo real — ainda precisa da pesquisa de
mercado registrada abaixo antes de desenhar), 4 (Configurações sugerir modelo) e 5 (destravar
escolha de modelo STT/TTS na UI) continuam como ideias registradas, sem código.

Pedido pelo usuário em 2026-08-17, cinco ideias relacionadas que chegaram juntas — todas dependem
da mesma infraestrutura base (config de provider+modelo de STT/TTS, ver ideia de TTS acima), então
ficam registradas juntas em vez de espalhadas.

**1 — Botão de microfone no composer (entrada de voz)**: quando um modelo de STT (Whisper ou
equivalente) estiver configurado em Configurações, o composer ganha um ícone de microfone. Clicar
grava áudio, transcreve via STT, e insere o TEXTO no composer (usuário ainda revisa/edita/envia
manualmente, não é envio automático) — poupa digitar, mas mantém o controle de revisar antes de
mandar pro LLM.

**2 — Redesenho do "+"**: hoje o "+" do composer abre um menu picado (cada opção — pasta, modo,
pensamento, modelo — vive num lugar diferente do composer). Pedido: o "+" abrir TUDO num lugar só
(pasta, modo de execução, pensamento, modelo de chat). Junto disso, um texto CINZA fora da área do
composer mostrando o que está em uso agora (modelo de chat, modelo de STT, modelo de TTS, quando
configurados) — visibilidade constante sem precisar abrir nada. Se a sessão ainda não tem modelo de
chat escolhido, abrir a janela de escolha automaticamente; se já escolheu antes, usar a última
escolha salva **(usuário acredita que isso já existe — confirmar como parte do trabalho, não
assumir)**.

**3 — Modo "conversa" em tempo real (voz↔voz)**: pergunta em aberto do usuário, precisa de pesquisa
antes de desenhar — o OpenRouter (ou outro provider) tem uma API nativa de conversa por voz em
tempo real (tipo a Realtime API da OpenAI, latência baixa, turno de fala detectado automaticamente),
ou o Cerne precisaria montar isso na mão encadeando STT→LLM→TTS (grava até detectar pausa na fala →
manda pro STT → texto vai pro LLM → resposta do LLM vai pro TTS → toca o áudio)? **Precisa de
pesquisa de mercado antes de comprometer arquitetura** — as duas abordagens têm custo/latência bem
diferentes (uma API nativa de voz-a-voz é mais fluida mas mais cara e mais provider-specific; a
cadeia manual funciona com qualquer combinação de provider mas tem mais round-trips = mais delay
entre o usuário parar de falar e ouvir a resposta). Escopo de UI: um botão especial e distinto no
composer pra esse modo (diferente do botão de microfone do item 1, que só transcreve pro texto).

**4 — Configurações sugerir os melhores modelos de STT/TTS no OpenRouter**: usuário não tem certeza
se Kokoro é de fato a melhor opção barata de TTS hoje (perguntou explicitamente) — em vez do usuário
ter que pesquisar sozinho, a tela de Configurações (seção de voz, quando existir) devia sugerir
opções já avaliadas por custo/qualidade, tipo o dropdown de search providers já sugere. Precisa de
pesquisa periódica (o "melhor barato" muda com o tempo, mesma observação já registrada na pesquisa
de modelos de visão do OpenRouter) — não é uma lista estática pra sempre.

**5 — Destravar escolha de modelo quando já tem chave OpenRouter configurada**: se o usuário já
colocou uma chave de API do OpenRouter (em Configurações), libera a escolha de modelo pras três
categorias — chat (já existe), STT e TTS — sem precisar de configuração extra separada pra cada
uma. Nomenclatura pro usuário final NÃO deve usar "Whisper"/"Kokoro" (nomes de modelo, não
conceito) — usar rótulos tipo "Áudio → Texto" e "Texto → Áudio" (mesmo espírito do resto da UI, que
já é traduzida pros 4 idiomas do app — `pt-BR`/`en`/`es`/`zh` — esses rótulos novos precisam entrar
nos 4 arquivos de locale também, não só português).

### Backup/exportar sessão como .zip

**✅ IMPLEMENTADO — 2026-08-17** (junto com o backup via git logo abaixo — resumo de implementação
real ao final da seção de git). Pedido pelo usuário em 2026-08-17. Ideia central: dar um jeito de tirar uma sessão (ou todas) do
Cerne pra fora — backup antes de mexer em algo arriscado, levar pra outra máquina, compartilhar uma
sessão específica com alguém, ou simplesmente ter uma cópia de segurança fora do `app_data_dir`.

**O que já existe pra reaproveitar**: cada sessão já vive isolada em
`{app_data_dir}/sessions/{id}/`, com só 3 arquivos — `session.json` (metadados), `chat_log.json`
(mensagens, incluindo imagens já embutidas como data URI inline, não tem anexo solto em arquivo
separado pra se preocupar) e `tasks.json` (histórico de tool calls). Ou seja, um zip por sessão já
seria auto-contido sem precisar agregar nada de fora dessa pasta. A crate `zip` já é dependência de
verdade do projeto (promovida de `[dev-dependencies]` pro T16, `create_pptx`) — mesma crate serve
tanto pra escrever quanto pra ler (já usada em teste pra reabrir um `.pptx` gerado e conferir o
conteúdo), então não precisa de dependência nova.

**Dois escopos diferentes, provavelmente as duas fazem sentido**:
- **Exportar/importar UMA sessão** — botão a mais na linha da sessão (`SidebarSessionItem.vue`,
  mesmo lugar de renomear/mover/excluir) chamando "Exportar", salva um `.zip` via diálogo nativo do
  SO (mesmo padrão já usado em `FileBrowser.vue`/`ExtraReadPaths.vue`). "Importar sessão" fica um
  botão novo perto do "+ Nova sessão" no topo da sidebar. Caso de uso: compartilhar uma sessão
  específica, ou levar uma conversa importante pra outra máquina.
- **Backup de tudo** — zipar a pasta `sessions/` inteira de uma vez (todas as sessões), like um botão
  em Configurações "Baixar backup completo". Caso de uso mais parecido com "backup" de verdade
  (proteção contra perder tudo), diferente do caso de uso de "exportar uma sessão pra compartilhar".

**Cuidado de segurança ao IMPORTAR** (não é opcional, é o ponto que mais importa acertar): zip
malicioso pode ter entradas com `../` tentando escrever fora da pasta de destino (zip-slip clássico)
— toda entrada extraída precisa ser validada (rejeitar caminho absoluto ou que contenha `..` depois
de normalizado) antes de escrever, igual o cuidado que `websearch::validate_public_url` já tem pra
bloquear rede interna na importação de skill por URL (mesma categoria de problema: dado de fora
tentando escapar do sandbox esperado).

**Decisões a tomar antes de implementar**:
- Sessão importada sempre ganha um UUID NOVO (nunca reusa o id do zip) — evita colisão com sessão já
  existente nesta máquina e evita sobrescrever sem querer.
- `parent_session_id` (Fase G) do `session.json` importado deve ser limpo (`None`) — a sessão pai
  quase certamente não existe na máquina de destino, manter a referência ia só apontar pra nada.
- `persona_id`: manter como está (referência solta se a persona não existir no destino) ou limpar
  também? Provavelmente limpar, mesmo raciocínio do `parent_session_id`.
- Vale um manifesto simples dentro do zip (ex: `cerne_export.json` com versão do formato/timestamp
  de exportação) pra validar antes de tentar importar, em vez de só confiar que os 3 arquivos batem
  o formato esperado — se o formato de `session.json`/`chat_log.json` mudar no futuro (novos campos
  obrigatórios, por exemplo), um manifesto versionado ajuda a decidir se um zip antigo ainda é
  importável ou precisa de migração.

**Pastas e subpastas (T29) — não pode só descartar, os dois escopos pedem tratamento diferente**:
`folders.json` é GLOBAL (`{app_data_dir}/folders.json`, fora de qualquer pasta de sessão — uma
lista de pastas compartilhada por todas as sessões, `Folder{id,name,parent_id}`), diferente de
`session.json`/`chat_log.json`/`tasks.json` que são por sessão. Isso muda a estratégia conforme o
escopo:
- **Backup completo**: inclui `folders.json` inteiro no zip junto das pastas de sessão. Na
  restauração, os `id` de pasta batem exatamente com os `folder_id` de cada `session.json` (foram
  exportados juntos, do mesmo estado) — restaurar os dois de uma vez preserva a árvore inteira sem
  precisar de nenhum matching esperto. Cuidado só se o destino JÁ tiver pastas próprias com os
  MESMOS ids (pouco provável, mas então precisa decidir: sobrescrever, pular, ou gerar novos ids e
  remapear `folder_id` de cada sessão restaurada pro novo id).
- **Exportar/importar UMA sessão**: `folders.json` não faz parte da pasta da sessão, então o zip de
  uma sessão só não carrega a definição da pasta sozinho — só o `folder_id` (um UUID que não
  significa nada na máquina de destino). Pra não simplesmente jogar a sessão pra raiz sempre (perde
  a organização que o usuário tinha), exportar deveria gravar o NOME da pasta (e o nome da pasta-mãe,
  se for subpasta — T29 é só 2 níveis) como metadado extra no zip, não o id. Na importação: procurar
  no destino uma pasta com esse NOME (mesmo nível/hierarquia); se existir, anexa a sessão nela; se
  não existir, cria a pasta (e a subpasta, se for o caso) e anexa. Resolve o caso comum (mesma
  pessoa importando na própria máquina, ou pastas com nomes previsíveis tipo "trabalho"/"pessoal")
  sem exigir que os ids batam.

### Backup/sincronização via git (em vez de .zip manual)

Pedido pelo usuário em 2026-08-17, junto com a ideia de backup em .zip acima — mesma necessidade
(tirar as sessões do Cerne pra um lugar seguro fora do `app_data_dir`), transporte diferente. O
usuário já tem um exemplo funcionando de um app próprio (`F:\vaultMD`) com esse padrão: puxa do
repositório remoto ao abrir, e manda pro repositório quando o usuário clica em "sincronizar" — sem
zipar nada, o conteúdo em si (aqui: a árvore de pastas + sessões, mesmo escopo de dados do backup em
.zip registrado acima) é commitado direto num repositório git.

**Como se relaciona com a ideia do .zip**: é o MESMO escopo de dados (pastas de sessão +
`folders.json`, ver decisões já registradas ali em cima sobre como tratar cada um) — só muda o
transporte. Provavelmente as duas ideias convergem numa única decisão de arquitetura: "exportar
gera os arquivos formatados pra fora do `app_data_dir`" vira o passo comum, e depois ou zipa (pro
.zip) ou faz `git add`/`commit`/`push` num diretório de trabalho apontando pro repositório
configurado (pro sync).

**Diferenças importantes de trazer git pra dentro do Cerne** (não é só "trocar zip por commit"):
- Precisa de um diretório de trabalho separado (clone do repositório de backup) — não dá pra
  simplesmente rodar `git` dentro de `app_data_dir` misturado com o resto (config, chaves no
  keyring, etc.) sem risco de vazar algo sensível num commit por engano.
- Autenticação com o remoto (push/pull) — usuário precisaria configurar isso fora do Cerne (SSH key,
  token) ou o Cerne precisaria de um jeito de gerenciar credencial git, que é escopo bem maior
  (e mais um lugar pra guardar segredo com cuidado, mesma categoria de risco que já existe pra API
  key/keyring).
- Conflito: e se o usuário sincronizar de duas máquinas diferentes e as duas mudarem a mesma sessão
  entre uma sincronização e outra? Git resolve merge de texto, mas `session.json`/`chat_log.json`
  são JSON — um merge automático malfeito pode corromper o arquivo. Precisa decidir uma estratégia
  clara (último a sincronizar vence? merge por sessão individual, já que cada uma é uma pasta
  separada, reduz a chance de conflito real dentro do MESMO arquivo? avisar o usuário e deixar ele
  resolver manualmente, tipo um merge conflict de verdade?).
- Botão "Sincronizar" (like o exemplo do usuário) é mais simples que automatizar (ex: sincronizar
  sozinho a cada N minutos, ou ao fechar o app) — comportamento explícito, sem surpresa, primeira
  versão provavelmente só isso.

**✅ IMPLEMENTADO — 2026-08-17 (.zip e git juntos, resumo real)**:
- **`src-tauri/src/backup.rs`** (novo módulo): `export_sessions_zip(app_data_dir, session_ids, dest)`
  (vazio = todas, conta como "backup completo") e `import_sessions_zip(app_data_dir, source)`.
  Decisões seguidas à risca das notas acima: sessão importada SEMPRE ganha UUID novo (nunca reusa o
  id do zip — reimportar o mesmo zip cria uma segunda cópia em vez de "já existe"),
  `parent_session_id` sempre limpo, `folder_id` só é preservado quando o zip é backup completo
  (indicado por um manifesto `cerne_backup_manifest.json` dentro do zip — export de UMA sessão não
  leva `folders.json`, então zera `folder_id` pra não deixar referência pendurada). Backup completo
  também leva `folders.json` (mescla no destino via novo `folders::merge_folders`, nunca sobrescreve
  pasta cujo id já existe). Zip-slip mitigado com `ZipFile::enclosed_name()` (API da própria crate
  `zip`) + checagem redundante de `starts_with`. 8 testes novos (`backup.rs`), incluindo um que gera
  um zip malicioso de propósito e confirma que a entrada é rejeitada.
- **`git.rs`**: 5 funções novas (`is_repo`/`init_repo`/`set_remote`/`get_remote`/`sync`) — repo do
  backup fica em `<app_data_dir>/sessions` (NÃO o app_data_dir inteiro — isola de `config.json`/
  `mcp_servers.json`, que podem ter segredo em texto puro no campo `env`). `sync()` faz
  `add -A` + commit condicional + `pull --no-rebase --allow-unrelated-histories` + `push` — decisão
  de conflito registrada acima ("avisar o usuário") seguida à risca: erro de merge do próprio git
  sobe pro chamador sem tentar resolver sozinho. Autenticação 100% delegada ao git/SO (nenhuma
  credencial gerenciada pelo Cerne). 5 testes novos.
- **Frontend (`Settings.vue`)**: nova seção "Backup de sessões" com 3 botões (exportar sessão atual,
  exportar todas, importar de um .zip — usando os diálogos nativos `save`/`open`) + bloco de git
  (iniciar backup, definir remoto, sincronizar) logo abaixo, mesma seção. `api.ts` ganhou 6 funções
  novas. i18n nos 4 idiomas.
- **Não testado na janela real ainda** — implementado e validado só por `cargo test`/`vue-tsc`.
- **Bug real achado testando ao vivo — ✅ CORRIGIDO — 2026-08-18**: usuário clicou em "Sincronizar"
  sem querer, sem nome/email/token configurados, e o modal de Configurações travou — `run_git` não
  tinha timeout nenhum (diferente de `run_command`/teste de conexão MCP, que já tinham), então
  `git pull`/`push` esperando credencial que não tinha como chegar (sem terminal pra pedir de
  verdade) travava pra sempre, e a Promise do frontend nunca resolvia. Dois fixes em `git.rs`:
  (a) `GIT_TERMINAL_PROMPT=0` no ambiente de toda chamada git (falha na hora com erro claro em vez
  de tentar pedir credencial interativa); (b) timeout de 20s via watchdog numa thread separada que
  mata o processo (`taskkill /T /F` no Windows) se passar do prazo — implementado com
  `wait_with_output()` em vez de poll com `try_wait`, pra não arriscar deadlock de pipe se a saída
  passar do buffer do SO antes do processo terminar. Junto, resolvendo a causa raiz de fundo
  (pedido do usuário: "o certo é eu dar usuário/email/token, pode ser fake, e o token
  encriptado"): `git::set_local_identity`/`get_local_identity` (nome/email `--local`, nunca
  `--global` — pode ser fake, git só precisa de algo consistente pra commitar) e um token guardado
  no cofre de credenciais do sistema via `keyring` (`config.rs`, mesmo mecanismo da chave do
  OpenRouter, novo `KEYRING_USER_BACKUP_GIT_TOKEN`) — nunca gravado em texto puro no
  `.git/config`; injetado só no ARGUMENTO da URL de cada `pull`/`push` (`authenticated_url`, só
  pra remoto `https://` — SSH continua usando o agente SSH do sistema normalmente).
  `sync()` agora exige identidade configurada antes de tentar commitar (erro claro em vez de falha
  confusa do git). Novos comandos Tauri `backup_git_set_identity`/`set_backup_git_token`/
  `clear_backup_git_token`; `Settings.vue` ganhou campos de nome/email + token (mesmo padrão visual
  da chave do OpenRouter — chip com prévia mascarada, "Trocar"/"Remover"). 5 testes novos em
  `git.rs` (identidade local, injeção de token só em https, erro sem identidade configurada).
  `cargo test --lib` (202 passando) e `vue-tsc --noEmit` limpos.

**Reaproveita**: a `zip`/`git`-related infra já existente no projeto — `git.rs` (novo módulo desta
sessão, hoje só lê status/diff do repositório do PROJETO do usuário, mas o mecanismo de rodar
`git` via subprocesso com `CREATE_NO_WINDOW` já está pronto e testado) poderia ganhar mais
funções (`clone`/`add`/`commit`/`push`/`pull`) em vez de escrever isso do zero.

### Sessões paralelas orquestradas pelo próprio LLM

Pedido pelo usuário em 2026-08-16 — detalhado como **Fase G** em
`PLANOS/13_roteiro_agentes_skills_fases.md`. **✅ IMPLEMENTADA — 2026-08-16** (mesmo dia, ver
detalhe completo no roteiro), **✅ TESTADA NA JANELA REAL — 2026-08-17** (G3, modo local vs
paralelo), com 3 bugs achados e corrigidos ao vivo:

1. **Loop apertado ignorando o hint de espera**: `check_agent_session` só sugeria em texto quanto
   esperar antes de checar de novo, e um modelo local pequeno (qwen3.5-9b-mtp) simplesmente
   ignorou a sugestão e ficou chamando a ferramenta centenas de vezes seguidas sem pausa (usuário
   reportou "está verificando infinitamente", com print mostrando dezenas de `check_agent_session`
   em sequência), sobrecarregando o router do llama.cpp. Corrigido em `agent/mod.rs`: (a) a
   própria chamada agora `sleep`s server-side pelo tempo necessário (mín. 4s, máx. 30s, baseado no
   hint de G2) antes de responder "running" de novo — força a pausa de verdade, independente do
   modelo respeitar o texto ou não; (b) um contador `poll_count` em `OrchestratedSessionInfo`
   corta o loop de vez depois de `MAX_ORCHESTRATED_POLLS = 8` checagens, instruindo o modelo a
   parar e avisar o usuário em vez de continuar sondando pra sempre.
2. **Sessão filha sem pasta ficava sem NENHUMA ferramenta de arquivo/comando**: sessões criadas
   sem `project_root` (nem passado, nem herdado do pai) ficavam sem `read_file`/`write_file`/
   `run_command` (só entram no toolset quando há pasta), e o modelo não sabia disso — ficou
   tentando contornar via busca na web em vez de simplesmente dizer que precisava de uma pasta.
   Corrigido: (a) a descrição de `start_agent_session` agora avisa explicitamente essa
   consequência de omitir `project_root`; (b) o system prompt da sessão filha, quando fica sem
   pasta, agora diz isso de forma explícita em vez de ficar em silêncio (e o aviso de "shell
   disponível pra run_command" — que antes aparecia sempre, mesmo sem a ferramenta — só aparece
   quando ela realmente existe no toolset).
3. **Herança incompleta do pai**: a sessão filha só herdava provider/modelo/execution_mode/
   llama_fork/custom_provider_id/project_root — perdia pastas extras, MCPs habilitados e persona
   ativa que o usuário esperava por padrão. Pedido explícito do usuário ("deve levar tudo do
   PAI"): agora também herda `extra_read_paths`, `enabled_mcp_servers`, `persona_id` e
   `fable_method`.

Confirmado pelo usuário: "Agora sim, terminou os dois. pode dar como concluido".

Resumo: o LLM principal (orquestrador) cria sessões completas e independentes — não sub-agentes
efêmeros como a `task` que já existe — cada uma identificada pelo próprio UUID da `Session`, e
consulta o progresso delas sob demanda em vez de bloquear o turno esperando. Exemplo do usuário:
uma sessão monta o frontend Angular, outra o backend Spring REST, uma terceira confere se teve
erro, uma quarta confere a integração entre elas.

Pontos centrais do desenho (ver Fase G no roteiro pra detalhe completo):
- Reaproveita o padrão que `send_message` já usa (`lib.rs`) — `tauri::async_runtime::spawn` +
  `state.running_turns` — só que disparado por uma tool nova (`start_agent_session`) em vez do
  botão de enviar mensagem do usuário.
- `check_agent_session` devolve status + uma dica de tempo de espera calculada a partir de quanto
  demorou a PRIMEIRA resposta daquela sessão (pedido explícito do usuário: se a 1ª resposta levou
  30s, sugerir esperar ~30s+folga antes de checar de novo, não ficar checando toda hora).
  `check_agent_session`/`list_agent_sessions` entram no `DOOM_LOOP_EXEMPT_TOOLS` desde o início.
  Precisa entrar em `PLANOS/13_roteiro_agentes_skills_fases.md`.
- Guarda de profundidade: sessão orquestrada não ganha a tool de criar OUTRA sessão orquestrada
  (nível único), mesmo espírito do guard que a `task` já tem hoje.
- Local (llama.cpp) continua serializado por natureza (`ensure_llama_ready` já mata forks
  concorrentes) — bate com a memória "uma GPU só, uma coisa por vez"; API roda de verdade em
  paralelo, mesmo modelo que a Fase A4 já validou pra `task`s paralelas.
- Sessão orquestrada é uma `Session` de verdade — já aparece na sidebar normal, sem painel especial
  novo; só falta um `parent_session_id` (`#[serde(default)]`) pra UI marcar "criada por outra
  sessão" e futuramente agrupar (ligar com o T29, pastas de sessão, quando os dois forem
  implementados).
- Decisão em aberto mais importante: aviso ativo quando a sessão filha termina (injetar no próximo
  turno do pai, como o T14 cogitado pra background jobs) vs. ficar só no modelo de polling
  (orquestrador decide quando chamar `check_agent_session`). Recomendação registrada no roteiro:
  começar só com polling, é bem mais simples e reaproveita tudo que já existe.

### Estudo de viabilidade mobile

Pedido pelo usuário em 2026-08-17. **Isto é um ESTUDO, não uma tarefa de implementação** — o próprio
usuário foi explícito: precisa entender o que dá e o que não dá pra fazer antes de mexer em
qualquer código, pra não quebrar o que já funciona no desktop. Nada abaixo deve virar código sem
esse levantamento acontecer primeiro.

**Escopo pedido pelo usuário**: separar o Cerne em duas categorias de uso —
- **Cabe em mobile**: chat + Persona — conversar com o modelo, trocar de persona, sem nenhuma
  ferramenta de projeto envolvida (sem editar arquivo, sem rodar comando, sem navegador de arquivo).
  Mais parecido com um cliente de chat comum.
- **Precisa de estudo separado**: tudo que envolve ferramentas (`write_file`/`run_command`/
  `computer_use`/skills que rodam script/etc.) — o usuário reconhece que parte disso pode não fazer
  sentido ou não ser tecnicamente viável em mobile, e pede pra mapear exatamente o que dá e o que
  não dá, em vez de assumir.

**Por que isso é genuinamente um estudo antes de virar plano** (motivos técnicos concretos que já dá
pra antecipar, mas que precisam ser confirmados/aprofundados):
- **Tauri em mobile é uma plataforma diferente**: Tauri 2 suporta Android/iOS, mas nem toda API do
  Rust desktop está disponível igual — em especial tudo que assume um filesystem "normal" tipo
  desktop (escrever em qualquer caminho absoluto que o usuário escolher, `std::process::Command`
  pra rodar `git`/shell/etc., `tauri-plugin-dialog` de escolher pasta nativa) tem equivalente
  bem mais restrito ou inexistente em mobile — sandboxing de app é a norma lá (iOS principalmente).
- **`run_command`/`computer_use`/servidor `llama.cpp` local** dependem de rodar processo arbitrário
  ou binário nativo — mobile normalmente não permite isso pra apps de terceiros (App Store/Play
  Store têm regra contra isso, e o SO restringe de qualquer forma). Provavelmente fica de fora por
  completo, não é só questão de UI.
- **MCP (servidores via stdio, `npx`/`node`)** tem o mesmo problema de "rodar processo arbitrário".
- **O que provavelmente sobrevive sem mudança de arquitetura, só de UI**: chat puro, Personas,
  histórico de sessões, talvez `web_search`/`web_fetch` (só HTTP, sem processo local) — bate
  exatamente com o que o usuário já separou como "cabe em mobile".
- **Usabilidade**: layout inteiro hoje assume mouse+teclado+tela larga (sidebar fixa, composer com
  toolbar de vários ícones, modais largos tipo Settings/RepoDiffViewer) — mobile precisa de outra
  disposição (navegação por telas em vez de painel lateral fixo, touch em vez de hover pra revelar
  ações — `.session-actions`/`.step-row` de vários componentes hoje só aparecem no `:hover`, que não
  existe em touch).

**O que o levantamento devia produzir antes de qualquer código**:
1. Lista concreta de que ferramentas/tools funcionam sem mudança nenhuma, quais precisam de
   adaptação, e quais são estruturalmente inviáveis em mobile (com o motivo técnico de cada uma).
2. Confirmação de que Tauri 2 mobile realmente builda esse projeto como está hoje (rodar um build
   Android de teste, nem que seja só a build vazia, antes de prometer qualquer feature) — tem risco
   real de dependência Rust usada aqui (`zip`, `keyring`, etc.) não compilar pra `aarch64-linux-android`/
   iOS sem ajuste.
3. Proposta de como o MESMO código-base serve os dois layouts sem duplicar tudo — provavelmente
   esconder/adaptar componentes por breakpoint/plataforma em vez de duas árvores de UI separadas,
   mas isso também precisa de desenho próprio depois do levantamento.
4. Confirmação explícita de que nada disso quebra o desktop — cada mudança de UI precisa continuar
   funcionando exatamente igual no desktop, guardada atrás de detecção de plataforma, não uma
   reescrita que arrisca os dois ao mesmo tempo.

### Inspiração do Hermes Agent (NousResearch)

Pedido pelo usuário em 2026-08-17: cloná-lo (`github.com/NousResearch/hermes-agent`) e ver o que ele
tem que o Cerne não tem. Achado central: Hermes é uma plataforma bem maior em escopo — gateway de
mensageria (Telegram/Discord/Slack/WhatsApp/Signal), 7 backends de execução incluindo nuvem
serverless, geração de imagem/vídeo, voz completa, cron entregando em qualquer plataforma, e
ferramentas de geração de dados de treinamento (isso último é específico da Nous treinar os próprios
modelos, sem relação nenhuma com o propósito do Cerne) — nada disso é comparável 1:1, o Cerne é um
assistente de código local, desktop, não uma plataforma de agente pessoal multi-superfície. Os 5
pontos abaixo são os que genuinamente valem considerar, dentro do escopo do Cerne:

1. **Skills que se auto-melhoram**: Hermes cria skill nova sozinho depois de uma tarefa complexa
   (não só quando o usuário pede) E melhora skills existentes durante o uso — um "loop fechado" de
   aprendizado. O Cerne só cria/importa skill sob comando explícito (`create_python_tool` pro LLM,
   "Importar de URL" pro usuário) — nunca decide sozinho "essa tarefa merece virar skill" nem refina
   uma skill com base em como ela foi usada depois.
2. **Memória entre sessões**: Hermes mantém um perfil de usuário curado pelo próprio agente
   (`MEMORY.md`/`USER.md`, com "empurrões" periódicos pra persistir o que aprendeu), e busca
   full-text (FTS5) nas conversas passadas com resumo via LLM pra achar contexto relevante de sessões
   antigas. O Cerne não tem NENHUMA memória entre sessões — cada sessão começa do zero, sem saber
   nada do que já foi conversado em outra.
3. **Sub-agentes isolados por git worktree**: cada sub-agente do Hermes trabalha numa worktree git
   separada (mesmo repositório, working directory própria) — evita um sub-agente pisar no arquivo
   que outro está editando ao mesmo tempo. O Cerne roda `task`s em paralelo (Fase A4) todas na MESMA
   sandbox, sem isolamento nenhum — risco real de conflito se duas tocarem o mesmo arquivo.
   **Ressalva discutida com o usuário**: isolamento só é vantagem pura pra trabalho genuinamente
   INDEPENDENTE (mesma orientação que a skill `project-manager` já dá — "quando NÃO usar" cobre
   exatamente sub-tarefas em cadeia/dependentes, tipo front-end que precisa saber o que o back-end
   está fazendo). Pra trabalho dependente, a resposta não é desisolar, é não paralelizar essas em
   primeiro lugar — usar sequencial, ou a Fase G (sessão de verdade, visível, acompanhável ao vivo)
   em vez de `task`s cegas em paralelo.
4. **Checagem de vulnerabilidade de dependência** (`osv_check.py` no Hermes, usa a OSV database) —
   antes de instalar uma dependência nova (ex: numa ferramenta Python criada via `create_python_tool`,
   T17), confere se tem vulnerabilidade conhecida. O Cerne não faz nenhuma checagem desse tipo hoje.
5. **Conectores MCP pré-curados**: Hermes tem uma lista de ~20 integrações prontas (Notion, Linear,
   Stripe, Sentry, Figma, Vercel, Datadog, etc.) que o usuário só liga, sem digitar comando/args na
   mão. O Cerne exige configuração manual completa (comando, argumentos, variáveis de ambiente) pra
   qualquer servidor MCP, mesmo os mais comuns.

**Status em 2026-08-17 (madrugada, trabalho autônomo autorizado pelo usuário — "Faz todas as 5")**:
- **1 (skills que se auto-melhoram) — ✅ IMPLEMENTADO e TESTADO — 2026-08-18** (usuário: "3 - perfeito, pode dar como concluida"): nova tool `improve_skill(name, new_content)`
  (`tools.rs`/`agent/mod.rs`), reaproveita `skills::write_skill_file` (já existia, usada pelo
  `SkillEditorModal.vue`) — SUBSTITUI o `SKILL.md` inteiro (frontmatter + corpo), então a
  descrição da tool instrui explicitamente a chamar `load_skill` primeiro pra não perder metadado
  na reescrita. Respeita a allowlist de skills da persona ativa, igual `load_skill` já fazia.
  Gate de segurança é o mesmo que qualquer outra tool já tem: `execution_mode` da sessão (Manual
  pausa pra aprovação, Auto/Yolo aplica direto) — decisão deliberada de NÃO inventar uma
  confirmação especial só pra essa tool, consistente com como `create_python_tool`/
  `update_python_tool` (que também criam/editam capacidade reutilizável) já funcionam.
- **2 (memória entre sessões) — ✅ IMPLEMENTADO e TESTADO — 2026-08-18**: novo módulo `memory.rs` — `MEMORY.md` global em
  `app_data_dir` (fora de qualquer sessão), lido no início de TODA sessão (injetado no system
  prompt como "## Memoria entre sessoes" quando não vazio) e escrito só por `append` (tool nova
  `remember(fact)`, sempre acrescenta, nunca edita/apaga uma entrada existente — controle fino
  fica com o usuário, que pode editar o arquivo livremente numa textarea nova em Configurações,
  seção "Memória entre sessões"). Escopo reduzido em relação ao Hermes: sem busca full-text em
  conversas antigas (isso seria uma feature bem maior, indexação de todo o histórico) — só o
  arquivo de fatos curados, que já cobre o caso de uso central (lembrar preferência/convenção sem
  reexplicar). 5 testes novos. **Confirmado pelo usuário**: "2 - Funcionou perfeito nas sessões, só
  não apareceu em configurações" — achado testando ao vivo: a textarea de "Memória entre sessões"
  em `Settings.vue` carregava o conteúdo só em `onMounted` (uma vez, quando o modal é criado pela
  primeira vez) — reabrir o modal depois disso NUNCA recarregava nada. MCP/busca/backup git só
  mudam através do próprio modal (autoconsistente, o bug não afeta eles), mas a memória pode ser
  escrita de FORA (pela tool `remember` durante um turno de chat, com o modal fechado) — por isso
  só ela ficava visivelmente desatualizada. Corrigido com um `watch(() => props.visible)` que
  recarrega a memória toda vez que o modal reabre (precisou capturar `defineProps` numa variável
  `props`, que antes não era usada em lugar nenhum do `<script setup>`). `vue-tsc --noEmit` limpo.
- **4 (checagem OSV) — ✅ IMPLEMENTADO e TESTADO — 2026-08-18**: novo módulo `osv.rs` — nova tool
  `check_dependencies_osv` (sem argumento) lê `package.json`/`Cargo.toml`/`requirements.txt` da
  raiz do projeto (só dependências DIRETAS declaradas, sem resolver árvore transitiva nem ler
  lockfile), consulta a API pública do OSV.dev (`POST /v1/query`, sem chave) em paralelo pra cada
  dependência (`futures_util::future::join_all`, teto de 60 consultas por chamada) e resume o que
  achar. Registrada em `project_tool_specs()` (`tools.rs`), só disponível quando a sessão tem pasta
  de projeto. 5 testes novos (parsing dos 3 formatos de manifesto, incluindo pular aliases tipo
  `workspace:*`/`file:../x` e dependências locais/git do Cargo.toml que não têm versão publicada
  pra checar). **Confirmado pelo usuário**: "Checagem de vulnerabilidade OSV - ok".
- **5 (conectores MCP pré-curados) — ✅ IMPLEMENTADO e TESTADO — 2026-08-18**: novo arquivo
  `content/mcpConnectors.ts` — lista fixa de 10 servidores MCP conhecidos e estáveis do
  repositório oficial `modelcontextprotocol/servers` (Filesystem, GitHub, Brave Search, Slack,
  PostgreSQL, SQLite, Puppeteer, Memory, Google Maps, EverArt), cada um com comando/args prontos
  (todos via `npx`) e as variáveis de ambiente necessárias (nomes só, sem valor — usuário completa
  o segredo). Em Configurações → seção MCP, botão "Usar" em cada conector preenche o formulário de
  adicionar servidor (mesmo padrão de "preview antes de salvar" do import de skill por URL — nunca
  salva direto). **Confirmado pelo usuário**: "Conectores MCP pré-curados - ok".
  - **Pedido de seguimento do usuário — ✅ IMPLEMENTADO — 2026-08-18**: adicionar também os
    conectores "estilo Hermes" (Notion, Linear, Stripe, Sentry, Figma, Vercel, Datadog, etc.).
    Isso exigiu primeiro o pré-requisito real identificado antes ("todos"): **suporte a
    transporte MCP remoto (HTTP streamable) em `mcp.rs`**, já que a maioria desses serviços hoje
    publica o MCP deles como servidor hospedado, não pacote npm local.
    - **Backend**: `rmcp` ganhou a feature `transport-streamable-http-client-reqwest`
      (`Cargo.toml`). `McpServerConfig` ganhou `url: Option<String>` (servidor é "remoto" quando
      preenchido — `command`/`args`/`env` são ignorados nesse caso, ver `McpServerConfig::is_remote`)
      e `bearer_token: Option<String>` (token simples, sem fluxo OAuth completo — usuário cola um
      token já gerado no site do serviço, mesmo padrão manual da chave do OpenRouter). Nova função
      `connect_server` unifica a lógica de conectar (stdio OU `StreamableHttpClientTransport` via
      `StreamableHttpClientTransportConfig::with_uri(url).auth_header(token)`), reaproveitada tanto
      por `ensure_connected` (pool persistente) quanto `test_connection` (teste descartável antes
      de salvar) — sem duplicar a branch stdio/remoto nos dois lugares. 4 testes novos (`is_remote`,
      roundtrip de config remota, e os 2 antigos de `save_then_load`/`json_shape` atualizados pros
      campos novos).
    - **Frontend**: `Settings.vue` ganhou um toggle "Servidor remoto (HTTP)" no formulário de MCP —
      marcado, troca comando/args/env por campos de url + token; residual mostra badge "remoto" na
      lista de servidores configurados. `McpConnector` (`content/mcpConnectors.ts`) ganhou um campo
      `url` opcional — presente = conector remoto, `useMcpConnector` preenche o formulário
      certo conforme o tipo.
    - **Conectores adicionados, só os que têm um caminho real de token estático verificado
      (pesquisado um por um antes de adicionar)**: **Notion** (via servidor LOCAL oficial
      `@makenotion/notion-mcp-server` + `NOTION_TOKEN` — o servidor REMOTO da Notion foi checado e
      só aceita OAuth interativo, sem bearer token, então não foi usado) e **Linear** (servidor
      remoto oficial `https://mcp.linear.app/mcp`, aceita Personal API Key como bearer token,
      confirmado na documentação oficial). **Deliberadamente deixados de fora** (checados e
      confirmados OAuth-only no modo hospedado, sem token estático disponível): Sentry
      (`mcp.sentry.dev`, doc oficial confirma OAuth). Stripe/Figma/Vercel/Datadog não foram
      verificados individualmente por falta de tempo nesta leva — ficam pra quando alguém checar
      o método de auth de cada um (mesma regra: só adicionar se tiver token estático real).
    - **Miniaturas por conector — ✅ IMPLEMENTADO — 2026-08-18**: pedido do usuário depois de ver a
      lista de conectores sem nenhum ícone. `McpConnector` ganhou um campo `icon` — `{ kind: "svg",
      path, viewBox, color }` pra marca reconhecível (path vetorial oficial do projeto Simple Icons,
      github.com/simple-icons/simple-icons, licença CC0, buscado e embutido no arquivo SEM
      dependência de rede em tempo de execução — nada de CDN externo) pros 10 conectores com marca
      própria (GitHub, Brave, Slack, PostgreSQL, SQLite, Puppeteer, Google Maps, Notion, Linear), ou
      `{ kind: "material", name }` reaproveitando o Material Symbols que o resto do app já usa pros
      3 sem marca própria (Filesystem, Memory, EverArt). `Settings.vue` renderiza um `<svg>` inline
      ou um `<span class="msi">` conforme o `kind`.
  - **✅ CONFIRMADO NA JANELA REAL — 2026-08-20, com um bug real encontrado e corrigido**: usuário
    testou o conector Notion (local) e bateu num timeout de 15s no handshake — investigado (`npm
    view`/`npx` direto no terminal): o pacote registrado, `@makenotion/notion-mcp-server`, **não
    existe no npm** (404 — escopo errado, o GitHub é `makenotion/notion-mcp-server` mas o pacote
    publicado é `@notionhq/notion-mcp-server`, confirmado existindo via `npm view`). Corrigido em
    `content/mcpConnectors.ts`. De brinde, dois fixes de diagnóstico no mesmo teste: (a) erro do
    teste de conexão MCP (local E remoto) só mostrava a camada mais externa da mensagem (ex:
    "error sending request for url..."), escondendo a causa raiz — nova `describe_error_chain`
    (`mcp.rs`, mesmo padrão do `describe_reqwest_error` já usado em `audio.rs`) percorre a cadeia
    `source()` inteira; (b) testado também o remoto do Notion com token colado — confirma, como já
    documentado acima, que rejeita mesmo (erro de rede na hora de mandar o `initialize`, consistente
    com "só aceita OAuth interativo").
- **3 (sub-agentes isolados por git worktree) — DELIBERADAMENTE NÃO IMPLEMENTADO, mesmo com "faz
  todas as 5"**: decisão minha, não pedido do usuário. Diferente dos outros 4 (leitura/escrita
  contida em arquivos próprios do Cerne — skill, MEMORY.md, dependências do projeto, config de
  MCP), isolamento por worktree mexe DIRETO no repositório git REAL do usuário — criar
  `git worktree add`/branch nova por sessão orquestrada, e mais importante, precisa de uma
  estratégia de LIMPEZA (remover worktree + branch quando a sessão termina/é excluída) pra não
  deixar o repositório do usuário sujo de branches/worktrees órfãos pra sempre. Errar a limpeza
  (ex: `git worktree remove` falhando por causa de mudança não commitada, deixando a branch pra
  trás) é um problema que só aparece rodando de verdade contra um repositório real, e essa mudança
  ia acontecer sem o usuário poder testar/revisar enquanto dormia. Prefiro deixar isso registrado
  como ideia pronta pra implementar (desenho abaixo) numa sessão em que o usuário possa acompanhar
  o primeiro teste ao vivo, em vez de arriscar mexer no git real dele sem supervisão.
  - **Desenho pra quando for implementar**: opção nova em `start_agent_session`
    (`isolate_worktree: bool`, default `false`) — quando `true` E `project_root` é repo git, roda
    `git worktree add <app_data_dir>/worktrees/<session_id> -b cerne-agent-<id-curto>` (fora da
    árvore do projeto, pra nunca aparecer como pasta suja no `git status` do usuário) e usa esse
    caminho como `project_root` da sessão filha em vez do original. `Session` ganharia
    `worktree_path`/`worktree_branch` (`Option<String>`) pra `sessions::delete_session` saber que
    precisa rodar `git worktree remove --force` + `git branch -D` na exclusão — e decidir o que
    fazer se a sessão for excluída com mudança não commitada na worktree (perguntar antes? avisar
    e recusar?). Reaproveita os helpers já existentes em `git.rs` (`run_git`), só precisa de
    `worktree add`/`worktree remove` novos.

Nenhum dos 5 tinha desenho técnico feito antes desta sessão.
