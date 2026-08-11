import { defineStore } from "pinia";
import {
  api,
  onAgentDone,
  onAgentError,
  onAgentStatus,
  onAskQuestion,
  onChatToken,
  onThinkingToken,
  onTodoUpdate,
  onContextCompacted,
  onContextUsage,
  onPendingEdit,
  onPermissionRequest,
  onToolCall,
  onTurnStats,
  onSessionRenamed,
  type AskQuestion,
  type ChatMessage,
  type ContextUsage,
  type ExecutionMode,
  type PendingEdit,
  type PermissionRequest,
  type ProviderKind,
  type Session,
  type TaskItem,
  type TodoItem,
  type TurnStats,
} from "../api";

export const useSessionStore = defineStore("session", {
  state: () => ({
    sessions: [] as Session[],
    currentId: null as string | null,
    currentSession: null as Session | null,
    currentFork: "turboquant",
    currentCustomProviderId: "",
    messages: [] as ChatMessage[],
    tasks: [] as TaskItem[],
    pendingEdits: [] as PendingEdit[],
    pendingQuestion: null as AskQuestion | null,
    pendingPermission: null as PermissionRequest | null,
    status: "idle" as "idle" | "thinking" | "running_tool" | "starting_server",
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
    listenersReady: false,
    error: "",
    contextUsage: null as ContextUsage | null,
    lastCompactionNote: "",
    draftText: "",
    computerUseWarned: false,
    showComputerUseWarning: false,
    screenshotCount: 0,
    thinkingStartedAt: null as number | null,
    turnStartedAt: null as number | null,
    turnStats: {} as Record<number, TurnStats>,
  }),
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

      await onAgentStatus((sessionId, status) => {
        if (sessionId !== this.currentId) return;
        if (status === "thinking" || status === "starting_server") {
          this.status = status;
          if (status === "thinking" && !this.thinkingStartedAt) {
            this.thinkingStartedAt = Date.now();
          }
          const lastRunning = [...this.tasks].reverse().find((t: TaskItem) => t.status === "running");
          if (lastRunning) lastRunning.status = "done";
        } else {
          this.status = "idle";
          this.thinkingStartedAt = null;
        }
      });

      await onToolCall((sessionId, tool, args) => {
        if (sessionId !== this.currentId) return;
        this.status = "running_tool";
        this.thinkingStartedAt = null;
        this.activeToolLabel = `${tool}(${args.slice(0, 60)})`;
        const userTurns = this.messages.filter((m) => m.role === "user").length;
        const task: TaskItem = {
          id: `live-${Date.now()}`,
          label: `${tool}(${args.slice(0, 80)})`,
          status: "running" as const,
          detail: null,
          turn: userTurns,
          started_at_ms: Date.now(),
          duration_ms: null,
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
        if (question.session_id !== this.currentId) return;
        this.pendingQuestion = question;
        this.status = "idle";
      });

      await onPermissionRequest((request) => {
        if (request.session_id !== this.currentId) return;
        this.pendingPermission = request;
      });

      await onAgentDone(async (sessionId) => {
        if (sessionId !== this.currentId) return;
        this.status = "idle";
        this.streamingText = "";
        this.liveBlocks = [];
        this.thinkingText = "";
        this.activeToolLabel = "";
        this.pendingPermission = null;
        await this.reloadCurrent();
      });

      await onAgentError((sessionId, message) => {
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
        this.pendingPermission = null;
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

    async createSession(
      title: string,
      provider: ProviderKind,
      model: string,
      projectRoot: string | null,
      forkId: string | null,
      customProviderId?: string | null,
    ) {
      const session = await api.createSession(title, provider, model, projectRoot, forkId, customProviderId);
      this.sessions.unshift(session);
      await this.selectSession(session.id);
      if (forkId) this.currentFork = forkId;
      if (customProviderId) this.currentCustomProviderId = customProviderId;
      return session;
    },

    async selectSession(id: string) {
      this.currentId = id;
      this.streamingText = "";
      this.liveBlocks = [];
      this.thinkingText = "";
      this.todoSnapshots = [];
      this.status = "idle";
      this.error = "";
      this.lastCompactionNote = "";
      this.pendingQuestion = null;
      this.computerUseWarned = false;
      this.showComputerUseWarning = false;
      this.screenshotCount = 0;
      this.thinkingStartedAt = null;
      this.turnStartedAt = null;
      this.turnStats = {};
      await this.reloadCurrent();
    },

    async reloadCurrent() {
      if (!this.currentId) return;
      this.currentSession = await api.getSession(this.currentId);
      if (this.currentSession.llama_fork) this.currentFork = this.currentSession.llama_fork;
      if (this.currentSession.custom_provider_id) this.currentCustomProviderId = this.currentSession.custom_provider_id;
      this.messages = await api.getSessionMessages(this.currentId);
      this.tasks = await api.getSessionTasks(this.currentId);
      this.pendingEdits = await api.listPendingEdits(this.currentId);
      this.contextUsage = await api.getSessionContextUsage(this.currentId);
    },

    /** The only way to actually change what an existing session sends to —
     * editing global config alone does nothing, sessions pin provider+model
     * at creation. */
    async updateProviderModel(provider: ProviderKind, model: string, forkId: string | null, customProviderId?: string | null) {
      if (!this.currentId || !model) return;
      const updated = await api.updateSessionProviderModel(this.currentId, provider, model, forkId, customProviderId);
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
      if (!this.pendingQuestion) return;
      await api.answerAsk(this.pendingQuestion.id, answer);
      this.pendingQuestion = null;
      this.status = "thinking";
    },

    async answerPermission(approved: boolean) {
      if (!this.pendingPermission) return;
      await api.answerPermission(this.pendingPermission.id, approved);
      this.pendingPermission = null;
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
     * no fallback de 8192. `null` volta pra resolução automática. */
    async updateContextLength(contextLength: number | null) {
      if (!this.currentId) return;
      const updated = await api.updateSessionContextLength(this.currentId, contextLength);
      this.currentSession = updated;
      const idx = this.sessions.findIndex((s) => s.id === updated.id);
      if (idx !== -1) this.sessions[idx] = updated;
      this.contextUsage = await api.getSessionContextUsage(updated.id);
    },

    async updateReasoningEffort(effort: "off" | "low" | "medium" | "high" | null) {
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
        this.tasks = [];
      }
    },

    setDraft(text: string) {
      this.draftText = text;
    },
  },
});
