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
    thinkingText: "",
    todoSnapshots: [] as { turn: number; todos: TodoItem[] }[],
    activeToolLabel: "",
    listenersReady: false,
    error: "",
    contextUsage: null as ContextUsage | null,
    lastCompactionNote: "",
  }),
  actions: {
    async initListeners() {
      if (this.listenersReady) return;
      this.listenersReady = true;

      await onChatToken((sessionId, delta) => {
        if (sessionId !== this.currentId) return;
        this.thinkingText = "";
        this.streamingText += delta;
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
        } else {
          this.status = "idle";
        }
      });

      await onToolCall((sessionId, tool, args) => {
        if (sessionId !== this.currentId) return;
        this.status = "running_tool";
        this.activeToolLabel = `${tool}(${args.slice(0, 60)})`;
      });

      await onPendingEdit((edit) => {
        if (edit.session_id !== this.currentId) return;
        this.pendingEdits.push(edit);
        this.acceptEdit(edit.id);
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
      this.thinkingText = "";
      this.todoSnapshots = [];
      this.status = "idle";
      this.lastCompactionNote = "";
      this.pendingQuestion = null;
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
    async updateExtraReadPaths(paths: string[]) {
      if (!this.currentId) return;
      const updated = await api.updateSessionReadPaths(this.currentId, paths);
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
      this.status = "thinking";
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
  },
});
