import { defineStore } from "pinia";
import {
  api,
  onAgentDone,
  onAgentError,
  onAgentStatus,
  onPipelineStatus,
  onBackgroundDone,
  onAutoContinueStopped,
  onSessionCreated,
  onAgentsSkillsPlan,
  onAskQuestion,
  onChatToken,
  onThinkingToken,
  onTodoUpdate,
  onContextCompacted,
  onContextUsage,
  onPendingEdit,
  onPermissionRequest,
  onTaskQueueStatus,
  onToolCall,
  onToolResult,
  onTurnStats,
  onSessionRenamed,
  type AgentsSkillsPlan,
  type AskQuestion,
  type ChatMessage,
  type CliBackendId,
  type ContextUsage,
  type ExecutionMode,
  type Folder,
  type PendingEdit,
  type PermissionRequest,
  type Persona,
  type PipelineStatus,
  type ProviderKind,
  type Session,
  type SkillMeta,
  type TaskItem,
  type TaskQueueStatusEvent,
  type TodoItem,
  type TurnStats,
} from "../api";
import { modelContextOverrideKey } from "./provider";

interface ReloadData {
  session: Session;
  messages: ChatMessage[];
  tasks: TaskItem[];
  pendingEdits: PendingEdit[];
  contextUsage: ContextUsage | null;
  // Carga lenta do histórico (2026-09-20): quando os dois vêm presentes,
  // `messages` é a janela INTEIRA (não só o que mudou) e estes dois valores
  // atualizam o cursor de paginação. `undefined` = não mexer no cursor já
  // existente (usado no caminho "since", que reconstrói a mesma janela sem
  // mover a borda de onde ela começa).
  messagesOldestIndex?: number;
  hasMoreMessages?: boolean;
}

// Quantas mensagens a carga inicial (ou "carregar anteriores") busca de uma
// vez — pedido do usuário (2026-09-20): sessões muito longas (modo Long
// Horizon, que nunca poda o histórico) não devem carregar/renderizar tudo
// de uma vez só de abrir a sessão.
const MESSAGES_PAGE_SIZE = 60;

export const useSessionStore = defineStore("session", {
  state: () => ({
    sessions: [] as Session[],
    folders: [] as Folder[],
    // Ids de sessão com turno rodando AGORA, mesmo em background (não só a
    // sessão aberta) — pedido do usuário (2026-08-18): bolinha piscando na
    // sidebar indicando processamento enquanto o LLM não termina, útil já
    // que dá pra ter várias sessões rodando ao mesmo tempo (API roda em
    // paralelo de verdade; local serializa, mas ainda mostra "processando"
    // até a fila resolver). Atualizado pelos listeners globais (registrados
    // uma vez em initListeners, recebem evento de QUALQUER sessão) — nunca
    // filtrado por currentId, ao contrário da maioria dos outros handlers.
    processingSessionIds: new Set<string>() as Set<string>,
    // Ids de sessão com a fila de tarefas rodando AGORA (mesmo padrão de
    // `processingSessionIds` acima) — atualizado pelo listener global de
    // `agent:task_queue_status`, nunca filtrado por currentId na escrita.
    taskQueueRunningIds: new Set<string>() as Set<string>,
    taskQueueLastEvent: null as TaskQueueStatusEvent | null,
    // Compartilhados entre ComposerBar.vue (menu `/`, seletor de persona) e
    // AgentsSkillsPanel.vue (aba de gerenciamento) — cada um tinha sua
    // PRÓPRIA cópia local antes, carregada só uma vez ao montar, então uma
    // persona/skill criada num painel nunca aparecia no outro sem recarregar
    // a sessão inteira (bug real encontrado testando ao vivo, 2026-08-16).
    personas: [] as Persona[],
    skills: [] as SkillMeta[],
    currentId: null as string | null,
    currentSession: null as Session | null,
    currentFork: "turboquant",
    currentCustomProviderId: "",
    messages: [] as ChatMessage[],
    // Carga lenta do histórico (2026-09-20): índice absoluto da PRIMEIRA
    // mensagem hoje em `messages` — `null` até a primeira carga acontecer.
    // `hasMoreMessages` diz se existe algo mais antigo pra "carregar
    // mensagens anteriores" buscar (ver `loadOlderMessages`).
    messagesOldestIndex: null as number | null,
    hasMoreMessages: false,
    loadingOlderMessages: false,
    tasks: [] as TaskItem[],
    pendingEdits: [] as PendingEdit[],
    // Por SESSÃO (não um valor único) — bug real encontrado testando ao
    // vivo, 2026-08-20: o usuário tinha uma sessão em modo Manual esperando
    // aprovação de uma tool call, trocou pra outra sessão, e a pergunta
    // ficou PERDIDA pra sempre (o handler antigo só guardava o pedido se
    // fosse da sessão atualmente aberta — `if (x.session_id !== currentId)
    // return`, descartando o resto). A sessão em background ficava travada
    // esperando uma resposta que nunca chegaria (o canal oneshot no backend
    // não tem timeout), com a bolinha de "processando" piscando pra sempre
    // porque o turno de fato nunca termina. Agora todo pedido é guardado
    // por `session_id`; os getters abaixo (`pendingQuestion` etc.) só
    // projetam o da sessão atual — voltar pra sessão A mostra o pedido dela
    // certinho, em vez de ele ter sumido.
    pendingQuestionBySession: {} as Record<string, AskQuestion>,
    pendingPermissionBySession: {} as Record<string, PermissionRequest>,
    pendingAgentsSkillsPlanBySession: {} as Record<string, AgentsSkillsPlan>,
    status: "idle" as "idle" | "thinking" | "running_tool" | "starting_server" | "compacting",
    streamingText: "",
    // Espelha texto e chamadas de ferramenta do turno em andamento na ORDEM
    // real em que aconteceram (texto, ferramenta, texto, ferramenta...) —
    // ao contrario de streamingText (uma string so, tudo concatenado) e
    // tasks (todas as chamadas do turno num bloco so), que nao davam pra
    // intercalar corretamente enquanto o turno ainda estava rolando.
    liveBlocks: [] as Array<
      | { kind: "text"; id: string; text: string }
      | { kind: "tools"; id: string; tasks: TaskItem[] }
    >,
    thinkingText: "",
    todoSnapshots: [] as { turn: number; todos: TodoItem[] }[],
    activeToolLabel: "",
    // Fase 3: progresso do pipeline determinístico Dev→QA→Analista, quando
    // `run_pipeline` está rodando nesta sessão — null fora de um pipeline.
    pipelineStatus: null as PipelineStatus | null,
    listenersReady: false,
    error: "",
    contextUsage: null as ContextUsage | null,
    lastCompactionNote: "",
    draftText: "",
    computerUseWarned: false,
    showComputerUseWarning: false,
    // Checado uma vez no início do app (2026-08-17, pedido do usuário) —
    // `null` = ainda não checou. Aviso mostrado (ChatView.vue) só quando a
    // sessão atual tem `project_root` e `git` não foi encontrado, já que é
    // aí que o visualizador de diff de repositório (T42) depende dele.
    gitAvailable: null as boolean | null,
    gitWarningDismissed: false,
    screenshotCount: 0,
    thinkingStartedAt: null as number | null,
    turnStartedAt: null as number | null,
    turnStats: {} as Record<number, TurnStats>,
  }),
  getters: {
    // Projeção da sessão ATUAL sobre os mapas por-sessão acima — mantém a
    // mesma API (`sessionStore.pendingQuestion` etc.) que `AskCard.vue`/
    // `PermissionCard.vue`/`AgentsSkillsPlanCard.vue`/`ChatView.vue` já
    // usavam, sem precisar mexer neles.
    pendingQuestion(state): AskQuestion | null {
      return (state.currentId && state.pendingQuestionBySession[state.currentId]) || null;
    },
    pendingPermission(state): PermissionRequest | null {
      return (state.currentId && state.pendingPermissionBySession[state.currentId]) || null;
    },
    pendingAgentsSkillsPlan(state): AgentsSkillsPlan | null {
      return (state.currentId && state.pendingAgentsSkillsPlanBySession[state.currentId]) || null;
    },
  },
  actions: {
    async initListeners() {
      if (this.listenersReady) return;
      this.listenersReady = true;

      await onChatToken((sessionId, delta) => {
        if (sessionId !== this.currentId) return;
        this.thinkingText = "";
        this.streamingText += delta;
        const last = this.liveBlocks[this.liveBlocks.length - 1];
        if (last && last.kind === "text") {
          last.text += delta;
        } else {
          this.liveBlocks.push({ kind: "text", id: `live-text-${Date.now()}-${this.liveBlocks.length}`, text: delta });
        }
      });

      await onThinkingToken((sessionId, delta) => {
        if (sessionId !== this.currentId) return;
        this.thinkingText += delta;
      });

      await onTodoUpdate((sessionId, todos) => {
        if (sessionId !== this.currentId) return;
        const userTurns = this.messages.filter((m) => m.role === "user").length;
        this.todoSnapshots.push({ turn: userTurns, todos });
      });

      await onTaskQueueStatus((event) => {
        if (event.status === "processing") {
          this.taskQueueRunningIds.add(event.session_id);
        } else {
          // "confirmed" também tira do rodando? Não — só os estados
          // terminais (finished/stuck/error) encerram o loop de verdade;
          // "confirmed" é só o fim de UM item, a fila continua pro próximo.
          if (event.status !== "confirmed") this.taskQueueRunningIds.delete(event.session_id);
        }
        if (event.session_id === this.currentId) this.taskQueueLastEvent = event;
      });

      await onAgentStatus((sessionId, status) => {
        if (status === "thinking" || status === "starting_server" || status === "compacting") {
          this.processingSessionIds.add(sessionId);
        } else {
          this.processingSessionIds.delete(sessionId);
        }
        if (sessionId !== this.currentId) return;
        if (status === "thinking" || status === "starting_server" || status === "compacting") {
          this.status = status;
          if (status === "thinking" && !this.thinkingStartedAt) {
            this.thinkingStartedAt = Date.now();
          }
        } else {
          this.status = "idle";
          this.thinkingStartedAt = null;
        }
      });

      await onPipelineStatus((status) => {
        if (status.session_id !== this.currentId) return;
        this.pipelineStatus = status;
      });

      // T14: quando um job em segundo plano termina, o backend já injeta a
      // nota no historico salvo em disco sozinho — só falta recarregar as
      // mensagens da sessão aberta pra ela aparecer sem precisar trocar de
      // sessão e voltar (bug encontrado testando ao vivo, 2026-08-16).
      await onBackgroundDone((e) => {
        if (e.session_id !== this.currentId) return;
        this.reloadCurrent();
      });

      // Guard anti-loop do auto-continue bateu o teto — o backend já
      // injetou uma nota explicando isso no histórico, só falta recarregar
      // pra ela aparecer sem trocar de sessão e voltar (mesmo motivo de
      // `onBackgroundDone` acima).
      await onAutoContinueStopped((sessionId) => {
        if (sessionId !== this.currentId) return;
        this.reloadCurrent();
      });

      // Fase G: sessão orquestrada criada pelo backend (fora do fluxo normal
      // de criação) — só insere na lista local em vez de recarregar tudo via
      // `loadSessions()`, mais barato e não perde a posição de scroll/estado
      // da sidebar.
      await onSessionCreated((session) => {
        if (!this.sessions.some((s) => s.id === session.id)) {
          this.sessions = [session, ...this.sessions];
        }
      });

      await onToolCall((payload) => {
        const { session_id: sessionId, id, tool, args, command, file_path } = payload;
        this.processingSessionIds.add(sessionId);
        if (sessionId !== this.currentId) return;
        this.status = "running_tool";
        this.thinkingStartedAt = null;
        this.activeToolLabel = `${tool}(${args.slice(0, 60)})`;
        const userTurns = this.messages.filter((m) => m.role === "user").length;
        const task: TaskItem = {
          id,
          label: `${tool}(${args.slice(0, 80)})`,
          status: "running" as const,
          detail: null,
          turn: userTurns,
          started_at_ms: Date.now(),
          duration_ms: null,
          command,
          file_path,
        };
        this.tasks = [...this.tasks, task];
        const lastBlock = this.liveBlocks[this.liveBlocks.length - 1];
        if (lastBlock && lastBlock.kind === "tools") {
          lastBlock.tasks.push(task);
        } else {
          this.liveBlocks.push({ kind: "tools", id: `live-tools-${Date.now()}`, tasks: [task] });
        }
        if (tool.startsWith("computer_use_")) {
          if (["computer_use_screenshot", "computer_use_click", "computer_use_type_text", "computer_use_press_key", "computer_use_scroll", "computer_use_drag", "computer_use_right_click", "computer_use_double_click"].includes(tool)) {
            this.screenshotCount++;
          }
          if (!this.computerUseWarned) {
            this.computerUseWarned = true;
            this.showComputerUseWarning = true;
          }
        }
      });

      await onToolResult((payload) => {
        const { session_id: sessionId, id, status, detail, additions, deletions, duration_ms, images } = payload;
        if (sessionId !== this.currentId) return;
        const applyResult = (t: TaskItem) => {
          t.status = status;
          t.detail = detail;
          t.additions = additions;
          t.deletions = deletions;
          t.duration_ms = duration_ms;
          t.images = images;
        };
        const task = this.tasks.find((t: TaskItem) => t.id === id);
        if (task) applyResult(task);
        for (const block of this.liveBlocks) {
          if (block.kind !== "tools") continue;
          const liveTask = block.tasks.find((t: TaskItem) => t.id === id);
          if (liveTask) applyResult(liveTask);
        }
      });

      await onPendingEdit((edit) => {
        if (edit.session_id !== this.currentId) return;
        this.pendingEdits.push(edit);
        // YOLO: ja aplicado direto, remove da lista de pendentes.
        // Auto/Manual: fica pendente pra o usuario aceitar/rejeitar.
        if (edit.already_applied) {
          this.pendingEdits = this.pendingEdits.filter((e) => e.id !== edit.id);
        }
      });

      await onAskQuestion((question) => {
        this.pendingQuestionBySession[question.session_id] = question;
        if (question.session_id !== this.currentId) return;
        this.status = "idle";
      });

      await onPermissionRequest((request) => {
        this.pendingPermissionBySession[request.session_id] = request;
      });

      await onAgentsSkillsPlan((plan) => {
        this.pendingAgentsSkillsPlanBySession[plan.session_id] = plan;
      });

      await onAgentDone(async (sessionId) => {
        this.processingSessionIds.delete(sessionId);
        delete this.pendingPermissionBySession[sessionId];
        delete this.pendingAgentsSkillsPlanBySession[sessionId];
        if (sessionId !== this.currentId) return;
        // Busca tudo ANTES de mexer em qualquer estado — limpar
        // streamingText/liveBlocks logo de cara (como era antes) deixava um
        // buraco visual entre o texto "ao vivo" sumir e o histórico
        // persistido ainda não ter chegado (5 chamadas sequenciais ao
        // backend), um flash rápido de tela vazia toda vez que um turno
        // terminava (achado testando ao vivo, 2026-08-17). Buscando em
        // paralelo e só then aplicando tudo de uma vez (mesmo tick), o
        // Vue troca o "ao vivo" pelo "persistido" numa render só, sem
        // buraco no meio.
        const reload = this.currentId ? await this.fetchReloadData(this.currentId) : null;
        if (sessionId !== this.currentId) return; // trocou de sessão enquanto buscava
        this.status = "idle";
        this.thinkingText = "";
        this.activeToolLabel = "";
        this.pipelineStatus = null;
        if (reload) this.applyReloadData(reload);
        this.streamingText = "";
        this.liveBlocks = [];
      });

      await onAgentError((sessionId, message) => {
        this.processingSessionIds.delete(sessionId);
        delete this.pendingPermissionBySession[sessionId];
        delete this.pendingAgentsSkillsPlanBySession[sessionId];
        if (sessionId !== this.currentId) return;
        if (this.streamingText.trim()) {
          this.messages.push({
            role: "assistant",
            content: this.streamingText,
            tool_calls: undefined,
            tool_call_id: undefined,
            name: undefined,
            images: [],
            display_content: undefined,
          });
        }
        this.status = "idle";
        this.streamingText = "";
        this.liveBlocks = [];
        this.thinkingText = "";
        this.error = message;
      });

      await onContextUsage((usage) => {
        if (usage.session_id !== this.currentId) return;
        this.contextUsage = usage;
      });

      await onContextCompacted((sessionId, summarizedMessages) => {
        if (sessionId !== this.currentId) return;
        this.lastCompactionNote = `Contexto compactado — ${summarizedMessages} mensagens antigas resumidas`;
      });

      await onTurnStats((stats) => {
        if (stats.session_id !== this.currentId) return;
        this.turnStats = { ...this.turnStats, [stats.turn]: stats };
        this.thinkingStartedAt = null;
      });

      await onSessionRenamed((sessionId, title) => {
        const idx = this.sessions.findIndex((s) => s.id === sessionId);
        if (idx !== -1) this.sessions[idx] = { ...this.sessions[idx], title };
        if (this.currentSession?.id === sessionId) {
          this.currentSession = { ...this.currentSession, title };
        }
      });
    },

    async loadSessions() {
      this.sessions = await api.listSessions();
    },

    async loadFolders() {
      this.folders = await api.listFolders();
    },

    async loadPersonas() {
      try {
        this.personas = await api.listPersonas();
      } catch {
        this.personas = [];
      }
    },

    async checkGitAvailable() {
      try {
        this.gitAvailable = await api.checkCommandAvailable("git");
      } catch {
        this.gitAvailable = null;
      }
    },

    async loadSkills(projectRoot: string | null) {
      try {
        this.skills = await api.listSkills(projectRoot);
      } catch {
        this.skills = [];
      }
    },

    async createFolder(name: string, parentId: string | null) {
      const folder = await api.createFolder(name, parentId);
      this.folders.push(folder);
      return folder;
    },

    async renameFolder(id: string, name: string) {
      if (!name.trim()) return;
      const updated = await api.renameFolder(id, name.trim());
      const idx = this.folders.findIndex((f) => f.id === id);
      if (idx !== -1) this.folders[idx] = updated;
    },

    async deleteFolder(id: string) {
      const orphanedSubfolderIds = await api.deleteFolder(id);
      this.folders = this.folders.filter((f) => f.id !== id);
      for (const subId of orphanedSubfolderIds) {
        const sub = this.folders.find((f) => f.id === subId);
        if (sub) sub.parent_id = null;
      }
      // Sessões que estavam nessa pasta (ou nas subpastas dela, já órfãs
      // acima) voltam pra raiz — backend não mexe em `Session.folder_id` das
      // subpastas devoradas, só das sessões que apontavam direto pra `id`.
      for (const s of this.sessions) {
        if (s.folder_id === id) s.folder_id = null;
      }
    },

    async moveSessionToFolder(sessionId: string, folderId: string | null) {
      const updated = await api.updateSessionFolder(sessionId, folderId);
      if (this.currentId === sessionId) this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === sessionId);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async createSession(
      title: string,
      provider: ProviderKind,
      model: string,
      projectRoot: string | null,
      forkId: string | null,
      customProviderId?: string | null,
      externalCliBackend?: CliBackendId | null,
    ) {
      const session = await api.createSession(
        title,
        provider,
        model,
        projectRoot,
        forkId,
        customProviderId,
        externalCliBackend,
      );
      this.sessions.unshift(session);
      await this.selectSession(session.id);
      if (forkId) this.currentFork = forkId;
      if (customProviderId) this.currentCustomProviderId = customProviderId;
      return session;
    },

    async selectSession(id: string) {
      this.currentId = id;
      // Cursor de paginação é POR SESSÃO — trocar de sessão sem zerar faria
      // `fetchReloadData` pedir "desde o índice X" na sessão NOVA, um índice
      // que não tem nada a ver com o histórico dela.
      this.messagesOldestIndex = null;
      this.hasMoreMessages = false;
      this.streamingText = "";
      this.liveBlocks = [];
      this.thinkingText = "";
      this.todoSnapshots = [];
      this.status = "idle";
      this.error = "";
      this.lastCompactionNote = "";
      this.computerUseWarned = false;
      this.showComputerUseWarning = false;
      this.screenshotCount = 0;
      this.thinkingStartedAt = null;
      this.turnStartedAt = null;
      this.turnStats = {};
      this.pipelineStatus = null;
      this.taskQueueLastEvent = null;
      await this.reloadCurrent();
      await this.applyRememberedContextLength();
      // Sincroniza com o backend (não só com eventos já vistos) — cobre o
      // caso de trocar pra uma sessão cuja fila já estava rodando antes
      // desta janela abrir (ou reabrir o app com uma fila deixada rodando).
      if (await api.isTaskQueueRunning(id)) this.taskQueueRunningIds.add(id);
      else this.taskQueueRunningIds.delete(id);
    },

    // Busca os pedaços do estado de uma sessão em PARALELO — separado de
    // `applyReloadData` pra quem precisa buscar primeiro e só trocar o
    // estado depois num momento preciso (`onAgentDone` acima, pra não deixar
    // um buraco visual entre limpar o streaming e o histórico persistido
    // chegar).
    //
    // Mensagens (2026-09-20, carga lenta): se já existe uma janela carregada
    // pra esta sessão (`messagesOldestIndex !== null` — `selectSession` zera
    // isso ao trocar de sessão), busca só "desde ali" em vez de recarregar
    // tudo — o histórico do Long Horizon nunca é podado, então uma sessão
    // longa pode ter milhares de mensagens; recarregar tudo a cada turno
    // seria cada vez mais lento E perderia as páginas antigas que o usuário
    // já tinha carregado via "carregar mensagens anteriores" (o `since`
    // reconstrói a MESMA janela, borda antiga incluída, só que atualizada
    // até o fim — nada some do que já estava visível).
    async fetchReloadData(sessionId: string): Promise<ReloadData> {
      const janelaExistente = this.messagesOldestIndex;
      const [session, messagesResult, tasks, pendingEdits, contextUsage] = await Promise.all([
        api.getSession(sessionId),
        janelaExistente !== null
          ? api.getSessionMessagesSince(sessionId, janelaExistente)
          : api.getSessionMessagesPage(sessionId, undefined, MESSAGES_PAGE_SIZE),
        api.getSessionTasks(sessionId),
        api.listPendingEdits(sessionId),
        api.getSessionContextUsage(sessionId),
      ]);
      if (Array.isArray(messagesResult)) {
        // Caminho "since": mesma janela, cursor (oldestIndex/hasMore) não muda.
        return { session, messages: messagesResult, tasks, pendingEdits, contextUsage };
      }
      return {
        session,
        messages: messagesResult.messages,
        tasks,
        pendingEdits,
        contextUsage,
        messagesOldestIndex: messagesResult.next_before,
        hasMoreMessages: messagesResult.has_more,
      };
    },

    applyReloadData(data: ReloadData) {
      this.currentSession = data.session;
      if (data.session.llama_fork) this.currentFork = data.session.llama_fork;
      if (data.session.custom_provider_id) this.currentCustomProviderId = data.session.custom_provider_id;
      this.messages = data.messages;
      // undefined = caminho "since", a janela não mudou de borda, não mexe.
      if (data.messagesOldestIndex !== undefined) this.messagesOldestIndex = data.messagesOldestIndex;
      if (data.hasMoreMessages !== undefined) this.hasMoreMessages = data.hasMoreMessages;
      this.tasks = data.tasks;
      this.pendingEdits = data.pendingEdits;
      this.contextUsage = data.contextUsage;
    },

    async reloadCurrent() {
      if (!this.currentId) return;
      const data = await this.fetchReloadData(this.currentId);
      this.applyReloadData(data);
    },

    // "Carregar mensagens anteriores" (2026-09-20, pedido do usuário): busca
    // a página mais antiga que a já carregada e PREPENDE — nunca troca
    // `this.messages` inteiro, só cresce por cima. Existe pra sessões muito
    // longas (Long Horizon nunca poda o histórico) não precisarem carregar
    // tudo de uma vez só de abrir a sessão.
    //
    // Botão manual em vez de scroll infinito automático: decisão consciente
    // por tempo/risco desta rodada, não definitivo — dá pra trocar por
    // scroll automático depois se fizer falta na prática.
    async loadOlderMessages() {
      if (!this.currentId || this.messagesOldestIndex === null || this.loadingOlderMessages) return;
      if (!this.hasMoreMessages) return;
      this.loadingOlderMessages = true;
      try {
        const page = await api.getSessionMessagesPage(
          this.currentId,
          this.messagesOldestIndex,
          MESSAGES_PAGE_SIZE,
        );
        this.messages = [...page.messages, ...this.messages];
        this.messagesOldestIndex = page.next_before;
        this.hasMoreMessages = page.has_more;
      } finally {
        this.loadingOlderMessages = false;
      }
    },

    /** The only way to actually change what an existing session sends to —
     * editing global config alone does nothing, sessions pin provider+model
     * at creation. */
    async updateProviderModel(provider: ProviderKind, model: string, forkId: string | null, customProviderId?: string | null) {
      if (!this.currentId || !model) return;
      // `customProviderId` reusa o mesmo slot pra "qual CLI externo" quando
      // provider === "cli" (ver ProviderPicker.vue/ComposerBar.vue) — separa
      // aqui antes de gravar, pra não misturar os dois campos persistidos.
      const isCli = provider === "cli";
      const updated = await api.updateSessionProviderModel(
        this.currentId,
        provider,
        model,
        forkId,
        isCli ? null : customProviderId,
        isCli ? (customProviderId as CliBackendId | null | undefined) : null,
      );
      this.currentSession = updated;
      if (forkId) this.currentFork = forkId;
      if (customProviderId) this.currentCustomProviderId = customProviderId;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
      this.contextUsage = await api.getSessionContextUsage(updated.id);
    },

    async updateTitle(id: string, title: string) {
      if (!title.trim()) return;
      const updated = await api.updateSessionTitle(id, title.trim());
      if (this.currentId === id) this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    /** Pastas extras (fora do project_root) que read_file/list_dir/grep/ast_grep
     * desta sessão podem acessar via caminho absoluto. write_file/edit_file/
     * ast_edit continuam restritos ao project_root — a sandbox só espelha ele. */
    async updateExtraReadPaths(entries: Array<{ path: string; mode: "read" | "read_write" }>) {
      if (!this.currentId) return;
      const updated = await api.updateSessionReadPaths(this.currentId, entries);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    /** `displayText` (defaults to `text`) is what shows up in the user's own
     * bubble — lets the composer send a bigger payload (attachment content
     * inlined) to the model while the chat history stays readable, showing
     * just what the user actually typed. Persisted server-side too
     * (`ChatMessage.display_content`), not just in this in-memory push —
     * otherwise reloading the session (e.g. leaving and coming back from
     * Settings) re-fetches from disk and shows the raw attachment dump
     * instead, with a giant scroll for a big document. `images` (data URIs)
     * ride along on both the displayed bubble and the outgoing request. */
    async send(text: string, displayText?: string, images: string[] = []) {
      if (!this.currentId || !text.trim()) return;
      this.messages.push({ role: "user", content: displayText ?? text, images });
      this.liveBlocks = [];
      this.status = "thinking";
      this.thinkingStartedAt = Date.now();
      this.turnStartedAt = Date.now();
      this.error = "";
      await api.sendMessage(this.currentId, text, images, displayText);
    },

    async acceptEdit(editId: string) {
      await api.acceptEdit(editId);
      this.pendingEdits = this.pendingEdits.filter((e) => e.id !== editId);
    },

    async rejectEdit(editId: string) {
      await api.rejectEdit(editId);
      this.pendingEdits = this.pendingEdits.filter((e) => e.id !== editId);
    },

    async answerQuestion(answer: string) {
      if (!this.currentId || !this.pendingQuestion) return;
      await api.answerAsk(this.pendingQuestion.id, answer);
      delete this.pendingQuestionBySession[this.currentId];
      this.status = "thinking";
    },

    async updateProjectRoot(projectRoot: string | null) {
      if (!this.currentId) return;
      const updated = await api.updateSessionProjectRoot(this.currentId, projectRoot);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async answerPermission(approved: boolean) {
      if (!this.currentId || !this.pendingPermission) return;
      await api.answerPermission(this.pendingPermission.id, approved);
      delete this.pendingPermissionBySession[this.currentId];
    },

    async answerAgentsSkillsPlan(approved: boolean) {
      if (!this.currentId || !this.pendingAgentsSkillsPlan) return;
      await api.answerAgentsSkillsPlan(this.pendingAgentsSkillsPlan.id, approved);
      delete this.pendingAgentsSkillsPlanBySession[this.currentId];
    },

    /** "manual" (toda tool call pausa pedindo aprovação) ou "auto" (roda
     * livre, cancelável a qualquer momento). Muda o comportamento do
     * PRÓXIMO turno em diante — não afeta uma execução já em andamento. */
    async updateExecutionMode(mode: ExecutionMode) {
      if (!this.currentId) return;
      const updated = await api.updateSessionExecutionMode(this.currentId, mode);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    /** Aborta o turno inteiro em andamento (modo "Auto") — a chamada HTTP
     * pro provider e qualquer tool call em curso são derrubadas na hora,
     * sem esperar o próximo checkpoint. */
    async cancelTurn() {
      if (!this.currentId) return;
      await api.cancelTurn(this.currentId);
    },

    /** Override manual do tamanho de contexto — usado quando o provider
     * (geralmente customizado) não expõe isso via API e a sessão fica presa
     * no fallback de 8192. `null` volta pra resolução automática.
     *
     * Também grava (ou limpa) o mesmo valor como "lembrado" pro modelo desta
     * sessão (`model_context_overrides.json`, chaveado por conexão+modelo) —
     * pedido do usuário (2026-08-18): configurar o contexto de um modelo uma
     * vez deve valer em QUALQUER sessão futura com o mesmo modelo, não só a
     * atual. `applyRememberedContextLength` (chamada ao entrar numa sessão)
     * é quem lê esse valor de volta. */
    // Botão "compactar agora" (ContextGauge.vue) — mesma compactação que já
    // acontece sozinha perto do teto, só que na hora. `messages` encolhe
    // (igual à compactação automática), então zera o cursor de paginação e
    // recarrega do zero em vez de tentar "continuar de onde parou" com um
    // índice que não existe mais depois de compactar.
    async compactNow(): Promise<boolean> {
      if (!this.currentId) return false;
      const compacted = await api.compactSessionNow(this.currentId);
      if (compacted) {
        this.messagesOldestIndex = null;
        this.hasMoreMessages = false;
        await this.reloadCurrent();
        this.contextUsage = await api.getSessionContextUsage(this.currentId);
        this.lastCompactionNote = "Contexto compactado";
      }
      return compacted;
    },

    async updateContextLength(contextLength: number | null) {
      if (!this.currentId || !this.currentSession) return;
      const updated = await api.updateSessionContextLength(this.currentId, contextLength);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
      this.contextUsage = await api.getSessionContextUsage(updated.id);

      const key = modelContextOverrideKey(
        updated.provider,
        updated.model,
        updated.llama_fork ?? undefined,
        updated.custom_provider_id ?? undefined,
      );
      await api.setModelContextOverride(key, contextLength);
    },

    /** Aplica o contexto lembrado pro modelo desta sessão, se ainda não tiver
     * um valor próprio — chamada ao entrar numa sessão (nova ou existente)
     * pra "vir preenchido" sem o usuário precisar reconfigurar toda vez.
     * Nunca sobrescreve um `context_length` que a sessão já tenha (respeita
     * override específico dessa sessão, se algum dia divergir do lembrado). */
    async applyRememberedContextLength() {
      if (!this.currentSession || this.currentSession.context_length != null) return;
      const session = this.currentSession;
      const key = modelContextOverrideKey(
        session.provider,
        session.model,
        session.llama_fork ?? undefined,
        session.custom_provider_id ?? undefined,
      );
      const remembered = await api.getModelContextOverride(key).catch(() => null);
      if (remembered == null || !this.currentId || this.currentId !== session.id) return;
      const updated = await api.updateSessionContextLength(this.currentId, remembered);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
      this.contextUsage = await api.getSessionContextUsage(updated.id);
    },

    async updateReasoningEffort(effort: "off" | "on" | "low" | "medium" | "high" | null) {
      if (!this.currentId) return;
      const updated = await api.updateSessionReasoningEffort(this.currentId, effort);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async updateFableMethod(enabled: boolean) {
      if (!this.currentId) return;
      const updated = await api.updateSessionFableMethod(this.currentId, enabled);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async updateLongHorizonEnabled(enabled: boolean) {
      if (!this.currentId) return;
      const updated = await api.updateSessionLongHorizonEnabled(this.currentId, enabled);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async updateTaskQueueEnabled(enabled: boolean) {
      if (!this.currentId) return;
      const updated = await api.updateSessionTaskQueueEnabled(this.currentId, enabled);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async startTaskQueue() {
      if (!this.currentId) return;
      this.taskQueueRunningIds.add(this.currentId);
      this.taskQueueLastEvent = null;
      try {
        await api.startTaskQueue(this.currentId);
      } catch (e) {
        this.taskQueueRunningIds.delete(this.currentId);
        throw e;
      }
    },

    async stopTaskQueue() {
      if (!this.currentId) return;
      await api.stopTaskQueue(this.currentId);
      this.taskQueueRunningIds.delete(this.currentId);
    },

    async updatePersona(personaId: string | null) {
      if (!this.currentId) return;
      const updated = await api.updateSessionPersona(this.currentId, personaId);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async updateMcpServers(enabledNames: string[] | null) {
      if (!this.currentId) return;
      const updated = await api.updateSessionMcpServers(this.currentId, enabledNames);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
    },

    async deleteSession(id: string) {
      await api.deleteSession(id);
      this.sessions = this.sessions.filter((s) => s.id !== id);
      if (this.currentId === id) {
        this.currentId = null;
        this.currentSession = null;
        this.messages = [];
        this.messagesOldestIndex = null;
        this.hasMoreMessages = false;
        this.tasks = [];
      }
    },

    setDraft(text: string) {
      this.draftText = text;
    },
  },
});
