import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ProviderKind = "openrouter" | "llama_cpp" | "ollama" | "lm_studio" | "custom";
export type ExecutionMode = "manual" | "auto" | "yolo";

export interface AppConfig {
  active_provider: ProviderKind;
  active_model: string | null;
  openrouter_base_url: string;
  llama_cpp_base_url: string;
  ollama_base_url: string;
  lmstudio_base_url: string;
  active_llama_fork: string;
  active_custom_provider_id: string | null;
  tts_provider: ProviderKind;
  tts_llama_fork: string | null;
  tts_custom_provider_id: string | null;
  tts_model: string;
  tts_voice: string;
  tts_auto_language: boolean;
  stt_provider: ProviderKind;
  stt_llama_fork: string | null;
  stt_custom_provider_id: string | null;
  stt_model: string;
  tts_backend: VoiceBackend;
  stt_backend: VoiceBackend;
  voicebox_base_url: string;
  voicebox_tts_profile: string;
  voicebox_stt_language: string;
}

export type VoiceBackend = "openai_compatible" | "voicebox";

export interface TtsResult {
  audio_base64: string;
  mime: string;
  play_locally: boolean;
}

export interface ModelInfo {
  id: string;
  label: string;
  context_length?: number | null;
  name?: string | null;
  description?: string | null;
  size_bytes?: number | null;
  parameter_size?: string | null;
  price_prompt?: number | null;
  price_completion?: number | null;
  supports_vision?: boolean | null;
  vision_hint?: string | null;
  supports_tools?: boolean | null;
  supports_audio?: boolean | null;
}

export interface ContextUsage {
  session_id: string;
  used_tokens: number;
  context_length: number;
  is_estimated_length: boolean;
  percent: number;
  total_prompt_tokens: number;
  total_completion_tokens: number;
  total_requests: number;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  tool_calls?: { id: string; type: string; function: { name: string; arguments: string } }[];
  tool_call_id?: string;
  name?: string;
  images?: string[];
  display_content?: string;
}

export interface Session {
  id: string;
  title: string;
  created_at: string;
  provider: ProviderKind;
  model: string;
  project_root: string | null;
  context_length: number | null;
  llama_fork: string | null;
  custom_provider_id: string | null;
  extra_read_paths: Array<{ path: string; mode: "read" | "read_write" }>;
  execution_mode: ExecutionMode;
  reasoning_effort: "off" | "on" | "low" | "medium" | "high" | null;
  enabled_mcp_servers: string[] | null;
  fable_method: boolean;
  persona_id: string | null;
  folder_id: string | null;
  parent_session_id: string | null;
}

export interface TaskItem {
  id: string;
  label: string;
  status: "pending" | "running" | "done" | "failed";
  detail?: string | null;
  turn: number;
  file_path?: string | null;
  additions?: number;
  deletions?: number;
  started_at_ms?: number;
  duration_ms?: number | null;
  command?: string | null;
  execution_id?: string | null;
}

export interface TurnStats {
  session_id: string;
  turn: number;
  elapsed_ms: number;
  prompt_tokens: number;
  completion_tokens: number;
}

export interface PendingEdit {
  id: string;
  session_id: string;
  target_path: string;
  sandbox_path: string;
  diff: string;
  is_new_file: boolean;
  already_applied?: boolean;
}

export interface AskQuestion {
  session_id: string;
  id: string;
  question: string;
  options: string[];
}

export interface PermissionRequest {
  session_id: string;
  id: string;
  tool: string;
  args: string;
}

export interface AgentSkillPlanItem {
  id: string;
  tool: "task" | "load_skill" | "verify_completion";
  name: string;
}

export interface AgentsSkillsPlan {
  session_id: string;
  id: string;
  items: AgentSkillPlanItem[];
  // Fase A4: true quando 2+ `task` deste plano vão rodar em PARALELO
  // (provider de API, não local) em vez de um de cada vez.
  parallel: boolean;
}

export interface ParallelExecutionInfo {
  session_id: string;
  count: number;
}

export type BackgroundJobStatus =
  | { kind: "running" }
  | { kind: "exited"; code: number | null }
  | { kind: "unknown" };

export interface BackgroundJobInfo {
  id: string;
  command: string;
  status: BackgroundJobStatus;
  output: string;
  started_at_ms: number;
  session_id: string;
}

export interface BackgroundOutputEvent {
  id: string;
  output: string;
}

export interface DirEntryInfo {
  name: string;
  path: string;
  is_dir: boolean;
}

export interface GitFileChange {
  path: string;
  status: "modified" | "added" | "deleted" | "renamed" | "untracked";
  additions: number;
  deletions: number;
}

export interface LlamaForkConfig {
  id: string;
  label: string;
  server_exe: string;
  models_ini: string;
  port: number;
}

export interface CustomProviderConfig {
  id: string;
  label: string;
  base_url: string;
  supports_vision: boolean;
  context_length: number | null;
}

export interface SkillMeta {
  name: string;
  description: string;
  scope: "global" | "project";
  dir: string;
}

export type SkillLanguage = "pt-br" | "en";

export type PersonaKind = "agent" | "persona";

export interface Persona {
  id: string;
  name: string;
  content: string;
  // Fase A3: allowlist de ferramentas — vazio (default) = sem restrição.
  tools: string[];
  // B3/Fase A3: allowlist de skills que essa persona pode carregar via
  // load_skill — vazio (default) = sem restrição.
  skills: string[];
  // Distingue "Agente" (orquestrador, passos explícitos) de "Persona"
  // (especialista/professor, tom e conhecimento) só na apresentação da UI —
  // mesmo mecanismo por baixo. Default "persona" (pedido do usuário,
  // 2026-08-18).
  kind: PersonaKind;
}

// T17: ferramenta Python criada pelo próprio LLM (`create_python_tool`/
// `update_python_tool`), disponível como skill em qualquer sessão futura.
// `name` é o slug — chave usada em update/delete. `script` é o corpo SEM o
// cabeçalho PEP 723 (esse é gerado no backend a partir de `dependencies`).
export interface PythonTool {
  name: string;
  description: string;
  dependencies: string[];
  script: string;
  tool_path: string;
}

// T29: pastas na lista de sessões da sidebar. Só 2 níveis — parent_id null =
// pasta de raiz (pode ter subpastas), parent_id preenchido = subpasta (só
// pode conter sessões, backend recusa criar um 3º nível).
export interface Folder {
  id: string;
  name: string;
  parent_id: string | null;
}

export interface McpServerConfig {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  // Servidor REMOTO (HTTP streamable) quando preenchido — command/args/env
  // são ignorados nesse caso. `bearer_token` é opcional (sem prefixo
  // "Bearer "), alguns servidores remotos não exigem token.
  url?: string | null;
  bearer_token?: string | null;
  enabled: boolean;
}

export interface McpToolInfo {
  name: string;
  description: string;
}

export type SearchProviderKind = "auto" | "brave" | "tavily" | "searxng" | "serper" | "exa" | "google_cse" | "bing";

export interface SearchConfigView {
  provider: SearchProviderKind;
  searxng_url: string;
  google_cse_id: string;
  bing_endpoint: string;
  has_brave_key: boolean;
  has_tavily_key: boolean;
  has_serper_key: boolean;
  has_exa_key: boolean;
  has_google_key: boolean;
  has_bing_key: boolean;
}

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  setConfig: (new_config: AppConfig) => invoke<void>("set_config", { newConfig: new_config }),
  setOpenrouterKey: (key: string) => invoke<void>("set_openrouter_key", { key }),
  hasOpenrouterKey: () => invoke<boolean>("has_openrouter_key"),
  openrouterKeyPreview: () => invoke<string | null>("openrouter_key_preview"),
  clearOpenrouterKey: () => invoke<void>("clear_openrouter_key"),
  getDisclaimerAccepted: () => invoke<boolean>("get_disclaimer_accepted"),
  setDisclaimerAccepted: (accepted: boolean) => invoke<void>("set_disclaimer_accepted", { accepted }),
  listProviderModels: (kind: ProviderKind, customProviderId?: string | null) =>
    invoke<ModelInfo[]>("list_provider_models", { kind, customProviderId }),
  getModelFavorites: (providerKey: string) => invoke<string[]>("get_model_favorites", { providerKey }),
  setModelFavorites: (providerKey: string, modelIds: string[]) =>
    invoke<void>("set_model_favorites", { providerKey, modelIds }),
  getModelContextOverride: (key: string) => invoke<number | null>("get_model_context_override", { key }),
  setModelContextOverride: (key: string, contextLength: number | null) =>
    invoke<void>("set_model_context_override", { key, contextLength }),
  resolveContextLength: (kind: ProviderKind, model: string, forkId: string | null, customProviderId?: string | null) =>
    invoke<number | null>("resolve_context_length", { kind, model, forkId, customProviderId }),
  listLlamaForks: () => invoke<LlamaForkConfig[]>("list_llama_forks"),
  addLlamaFork: (fork: LlamaForkConfig) => invoke<LlamaForkConfig[]>("add_llama_fork", { fork }),
  removeLlamaFork: (id: string) => invoke<LlamaForkConfig[]>("remove_llama_fork", { id }),
  listLlamaPresets: (forkId: string) => invoke<ModelInfo[]>("list_llama_presets", { forkId }),
  llamaServerHealth: (forkId: string) => invoke<boolean>("llama_server_health", { forkId }),
  startLlamaServer: (forkId: string) => invoke<void>("start_llama_server", { forkId }),
  stopLlamaServer: (forkId: string) => invoke<void>("stop_llama_server", { forkId }),

  listCustomProviders: () => invoke<CustomProviderConfig[]>("list_custom_providers"),
  testCustomProvider: (baseUrl: string, apiKey?: string) =>
    invoke<ModelInfo[]>("test_custom_provider", { baseUrl, apiKey }),
  addCustomProvider: (provider: CustomProviderConfig, apiKey?: string) =>
    invoke<CustomProviderConfig[]>("add_custom_provider", { provider, apiKey }),
  removeCustomProvider: (id: string) => invoke<CustomProviderConfig[]>("remove_custom_provider", { id }),
  hasCustomProviderKey: (id: string) => invoke<boolean>("has_custom_provider_key", { id }),

  listSessions: () => invoke<Session[]>("list_sessions"),
  createSession: (
    title: string,
    provider: ProviderKind,
    model: string,
    project_root: string | null,
    forkId: string | null,
    customProviderId?: string | null,
  ) =>
    invoke<Session>("create_session", {
      title,
      provider,
      model,
      projectRoot: project_root,
      forkId,
      customProviderId,
    }),
  updateSessionProviderModel: (
    id: string,
    provider: ProviderKind,
    model: string,
    forkId: string | null,
    customProviderId?: string | null,
  ) => invoke<Session>("update_session_provider_model", { id, provider, model, forkId, customProviderId }),
  updateSessionTitle: (id: string, title: string) => invoke<Session>("update_session_title", { id, title }),
  updateSessionExecutionMode: (id: string, executionMode: ExecutionMode) =>
    invoke<Session>("update_session_execution_mode", { id, executionMode }),
  updateSessionReadPaths: (id: string, extraReadPaths: Array<{ path: string; mode: "read" | "read_write" }>) =>
    invoke<Session>("update_session_read_paths", { id, extraReadPaths }),
  updateSessionProjectRoot: (id: string, projectRoot: string | null) =>
    invoke<Session>("update_session_project_root", { id, projectRoot }),
  updateSessionContextLength: (id: string, contextLength: number | null) =>
    invoke<Session>("update_session_context_length", { id, contextLength }),
  updateSessionReasoningEffort: (
    id: string,
    effort: "off" | "on" | "low" | "medium" | "high" | null,
  ) => invoke<Session>("update_session_reasoning_effort", { id, effort }),
  updateSessionFableMethod: (id: string, enabled: boolean) =>
    invoke<Session>("update_session_fable_method", { id, enabled }),
  updateSessionMcpServers: (id: string, enabledNames: string[] | null) =>
    invoke<Session>("update_session_mcp_servers", { id, enabledNames }),
  updateSessionPersona: (id: string, personaId: string | null) =>
    invoke<Session>("update_session_persona", { id, personaId }),
  updateSessionFolder: (id: string, folderId: string | null) =>
    invoke<Session>("update_session_folder", { id, folderId }),
  checkPathIsDirectory: (path: string) => invoke<boolean>("check_path_is_directory", { path }),
  listDirEntries: (path: string) => invoke<DirEntryInfo[]>("list_dir_entries", { path }),
  gitRepoStatus: (projectRoot: string) => invoke<GitFileChange[]>("git_repo_status", { projectRoot }),
  gitFileDiff: (projectRoot: string, path: string) => invoke<string>("git_file_diff", { projectRoot, path }),
  checkCommandAvailable: (name: string) => invoke<boolean>("check_command_available", { name }),
  exportSessionsBackup: (sessionIds: string[], destPath: string) =>
    invoke<{ exported: number }>("export_sessions_backup", { sessionIds, destPath }),
  importSessionsBackup: (sourcePath: string) =>
    invoke<{ imported: string[]; skipped_invalid: string[]; folders_added: number }>(
      "import_sessions_backup",
      { sourcePath },
    ),
  backupGitStatus: () =>
    invoke<{
      is_repo: boolean;
      remote: string | null;
      identity_name: string | null;
      identity_email: string | null;
      has_token: boolean;
      token_preview: string | null;
    }>("backup_git_status"),
  backupGitInit: () => invoke<void>("backup_git_init"),
  backupGitSetRemote: (url: string) => invoke<void>("backup_git_set_remote", { url }),
  backupGitSetIdentity: (name: string, email: string) =>
    invoke<void>("backup_git_set_identity", { name, email }),
  setBackupGitToken: (token: string) => invoke<void>("set_backup_git_token", { token }),
  clearBackupGitToken: () => invoke<void>("clear_backup_git_token"),
  backupGitSync: () => invoke<string>("backup_git_sync"),
  ttsSpeak: (text: string) => invoke<TtsResult>("tts_speak", { text }),
  sttTranscribe: (audioBase64: string, format: string) =>
    invoke<string>("stt_transcribe", { audioBase64, format }),
  getMemory: () => invoke<string>("get_memory"),
  setMemory: (content: string) => invoke<void>("set_memory", { content }),
  extractAttachmentText: (path: string) => invoke<string>("extract_attachment_text", { path }),
  checkVisionSupport: (sessionId: string) => invoke<boolean>("check_vision_support", { sessionId }),
  testVision: (kind: string, customProviderId: string | null, model: string) =>
    invoke<boolean>("test_vision", { kind, customProviderId, model }),
  readImageAsDataUrl: (path: string) => invoke<string>("read_image_as_data_url", { path }),
  getSession: (id: string) => invoke<Session>("get_session", { id }),
  getSessionMessages: (id: string) => invoke<ChatMessage[]>("get_session_messages", { id }),
  getSessionTasks: (id: string) => invoke<TaskItem[]>("get_session_tasks", { id }),
  getSessionContextUsage: (id: string) => invoke<ContextUsage>("get_session_context_usage", { id }),
  listAgentExecutions: () => invoke<AgentExecution[]>("list_agent_executions"),
  listBackgroundJobs: () => invoke<BackgroundJobInfo[]>("list_background_jobs"),
  stopBackgroundJob: (id: string) => invoke<string>("stop_background_job", { id }),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),

  sendMessage: (sessionId: string, text: string, images: string[] = [], displayText?: string) =>
    invoke<void>("send_message", { sessionId, text, images, displayText }),
  cancelTurn: (sessionId: string) => invoke<void>("cancel_turn", { sessionId }),

  listPendingEdits: (sessionId: string) => invoke<PendingEdit[]>("list_pending_edits", { sessionId }),
  acceptEdit: (editId: string) => invoke<void>("accept_edit", { editId }),
  rejectEdit: (editId: string) => invoke<void>("reject_edit", { editId }),
  saveAttachmentMd: (sessionId: string, filename: string, text: string) =>
    invoke<string>("save_attachment_md", { sessionId, filename, text }),
  answerAsk: (id: string, answer: string) => invoke<void>("answer_ask", { id, answer }),
  answerPermission: (id: string, approved: boolean) => invoke<void>("answer_permission", { id, approved }),
  answerAgentsSkillsPlan: (id: string, approved: boolean) =>
    invoke<void>("answer_agents_skills_plan", { id, approved }),

  listSkills: (projectRoot: string | null) => invoke<SkillMeta[]>("list_skills", { projectRoot }),
  createSkill: (name: string, description: string, language: SkillLanguage) =>
    invoke<string>("create_skill", { name, description, language }),
  skillTemplateBody: (language: SkillLanguage) => invoke<string>("skill_template_body", { language }),
  readSkill: (dir: string) => invoke<string>("read_skill", { dir }),
  saveSkill: (dir: string, content: string) => invoke<void>("save_skill", { dir, content }),
  openSkillsFolder: () => invoke<void>("open_skills_folder"),
  fetchSkillFromUrl: (url: string) => invoke<string>("fetch_skill_from_url", { url }),
  importSkill: (content: string) => invoke<string>("import_skill", { content }),
  openExternalUrl: (url: string) => invoke<void>("open_external_url", { url }),

  listPersonas: () => invoke<Persona[]>("list_personas"),
  createPersona: (name: string, content: string, tools: string[] = [], skills: string[] = [], kind: PersonaKind = "persona") =>
    invoke<Persona>("create_persona", { name, content, tools, skills, kind }),
  updatePersona: (id: string, name: string, content: string, tools: string[] = [], skills: string[] = [], kind: PersonaKind = "persona") =>
    invoke<Persona>("update_persona", { id, name, content, tools, skills, kind }),
  deletePersona: (id: string) => invoke<void>("delete_persona", { id }),

  listPythonTools: () => invoke<PythonTool[]>("list_python_tools"),
  updatePythonTool: (name: string, description: string, script: string, dependencies: string[]) =>
    invoke<PythonTool>("update_python_tool", { name, description, script, dependencies }),
  deletePythonTool: (name: string) => invoke<void>("delete_python_tool", { name }),

  listFolders: () => invoke<Folder[]>("list_folders"),
  createFolder: (name: string, parentId: string | null) =>
    invoke<Folder>("create_folder", { name, parentId }),
  renameFolder: (id: string, name: string) => invoke<Folder>("rename_folder", { id, name }),
  deleteFolder: (id: string) => invoke<string[]>("delete_folder", { id }),

  listMcpServers: () => invoke<McpServerConfig[]>("list_mcp_servers"),
  addMcpServer: (server: McpServerConfig) => invoke<void>("add_mcp_server", { server }),
  removeMcpServer: (name: string) => invoke<void>("remove_mcp_server", { name }),
  testMcpServer: (server: McpServerConfig) => invoke<McpToolInfo[]>("test_mcp_server", { server }),
  listMcpServerTools: (server: McpServerConfig) => invoke<McpToolInfo[]>("list_mcp_server_tools", { server }),

  getSearchConfig: () => invoke<SearchConfigView>("get_search_config"),
  saveSearchConfig: (
    provider: SearchProviderKind,
    searxngUrl: string,
    apiKey?: string,
    googleCseId?: string,
    bingEndpoint?: string,
  ) => invoke<SearchConfigView>("save_search_config", { provider, searxngUrl, apiKey, googleCseId, bingEndpoint }),
  clearSearchApiKey: (provider: SearchProviderKind) => invoke<SearchConfigView>("clear_search_api_key", { provider }),
  testSearchProvider: (
    provider: SearchProviderKind,
    apiKey?: string,
    searxngUrl?: string,
    googleCseId?: string,
    bingEndpoint?: string,
  ) => invoke<number>("test_search_provider", { provider, apiKey, searxngUrl, googleCseId, bingEndpoint }),
};

export function onChatToken(cb: (sessionId: string, delta: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; delta: string }>("chat:token", (e) => cb(e.payload.session_id, e.payload.delta));
}

export function onThinkingToken(cb: (sessionId: string, delta: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; delta: string }>("chat:thinking_token", (e) => cb(e.payload.session_id, e.payload.delta));
}

export interface TodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed";
}

export function onTodoUpdate(cb: (sessionId: string, todos: TodoItem[]) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; todos: TodoItem[] }>("agent:todo_update", (e) =>
    cb(e.payload.session_id, e.payload.todos),
  );
}

export function onAgentStatus(cb: (sessionId: string, status: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; status: string }>("agent:status", (e) => cb(e.payload.session_id, e.payload.status));
}

// Fase 3: progresso do pipeline determinístico Dev→QA→Analista (run_pipeline).
export interface PipelineStatus {
  session_id: string;
  step: "dev" | "qa" | "analista";
  round: number;
  max_rounds: number;
}

export function onPipelineStatus(cb: (status: PipelineStatus) => void): Promise<UnlistenFn> {
  return listen<PipelineStatus>("agent:pipeline_status", (e) => cb(e.payload));
}

export interface ToolCallPayload {
  session_id: string;
  id: string;
  tool: string;
  args: string;
  command: string | null;
  file_path: string | null;
  // UUID da execução de agente/skill (task/verify_completion) dona deste
  // passo, quando aplicável — null pro loop principal da sessão.
  execution_id?: string | null;
}

export function onToolCall(cb: (payload: ToolCallPayload) => void): Promise<UnlistenFn> {
  return listen<ToolCallPayload>("agent:tool_call", (e) => cb(e.payload));
}

export interface ToolResultPayload {
  session_id: string;
  id: string;
  status: "done" | "failed";
  detail: string | null;
  additions: number;
  deletions: number;
  duration_ms: number | null;
  execution_id?: string | null;
}

export interface AgentExecution {
  id: string;
  parent_id: string | null;
  session_id: string;
  kind: "task" | "verify_completion" | "pipeline";
  name: string;
  status: "running" | "done" | "failed";
  started_at_ms: number;
  finished_at_ms: number | null;
  steps: TaskItem[];
}

export function onToolResult(cb: (payload: ToolResultPayload) => void): Promise<UnlistenFn> {
  return listen<ToolResultPayload>("agent:tool_result", (e) => cb(e.payload));
}

export function onPendingEdit(cb: (edit: PendingEdit) => void): Promise<UnlistenFn> {
  return listen<PendingEdit>("agent:pending_edit", (e) => cb(e.payload));
}

export function onAskQuestion(cb: (question: AskQuestion) => void): Promise<UnlistenFn> {
  return listen<AskQuestion>("agent:ask", (e) => cb(e.payload));
}

export function onPermissionRequest(cb: (request: PermissionRequest) => void): Promise<UnlistenFn> {
  return listen<PermissionRequest>("agent:permission_request", (e) => cb(e.payload));
}

export function onAgentsSkillsPlan(cb: (plan: AgentsSkillsPlan) => void): Promise<UnlistenFn> {
  return listen<AgentsSkillsPlan>("agent:agents_skills_plan", (e) => cb(e.payload));
}

// Fase A4: aviso informativo (não bloqueia) disparado nos modos Auto/YOLO
// quando o turno vai rodar `task`s em paralelo via API — no modo Manual o
// mesmo aviso já vem embutido em `AgentsSkillsPlan.parallel` acima. Ainda
// sem um componente de toast consumindo isso (ver PLANOS/14_backlog_pendente.md).
export function onParallelExecutionInfo(cb: (info: ParallelExecutionInfo) => void): Promise<UnlistenFn> {
  return listen<ParallelExecutionInfo>("agent:parallel_execution_info", (e) => cb(e.payload));
}

// Fase C1: push de output de job em segundo plano, em tempo real — antes só
// dava pra saber via poll (`check_background_output` chamado pelo LLM).
export function onBackgroundOutput(cb: (e: BackgroundOutputEvent) => void): Promise<UnlistenFn> {
  return listen<BackgroundOutputEvent>("agent:background_output", (e) => cb(e.payload));
}

// T14: job em segundo plano terminou de vez — o backend já injeta uma nota
// no historico salvo em disco sozinho, mas sem escutar isso a tela aberta
// não sabia que precisava recarregar as mensagens pra mostrar a nota
// (bug real encontrado testando ao vivo, 2026-08-16 — a nota só aparecia
// trocando de sessão e voltando).
export interface BackgroundDoneEvent {
  id: string;
  session_id: string;
  command: string;
  output: string;
}

export function onBackgroundDone(cb: (e: BackgroundDoneEvent) => void): Promise<UnlistenFn> {
  return listen<BackgroundDoneEvent>("agent:background_done", (e) => cb(e.payload));
}

// Fase G: sessão orquestrada (`start_agent_session`) criada pelo backend
// sem passar pelo fluxo normal de "+ Nova sessão" — sem esse evento ela
// ficaria invisível na sidebar até o usuário recarregar o app na mão.
export function onSessionCreated(cb: (session: Session) => void): Promise<UnlistenFn> {
  return listen<Session>("agent:session_created", (e) => cb(e.payload));
}

export function onAgentDone(cb: (sessionId: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string }>("agent:done", (e) => cb(e.payload.session_id));
}

export function onAgentError(cb: (sessionId: string, message: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; message: string }>("agent:error", (e) => cb(e.payload.session_id, e.payload.message));
}

export function onContextUsage(cb: (usage: ContextUsage) => void): Promise<UnlistenFn> {
  return listen<ContextUsage>("agent:context", (e) => cb(e.payload));
}

export function onContextCompacted(cb: (sessionId: string, summarizedMessages: number) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; summarized_messages: number }>("agent:context_compacted", (e) =>
    cb(e.payload.session_id, e.payload.summarized_messages),
  );
}

export function onTurnStats(cb: (stats: TurnStats) => void): Promise<UnlistenFn> {
  return listen<TurnStats>("agent:turn_stats", (e) => cb(e.payload));
}

export function onSessionRenamed(cb: (sessionId: string, title: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; title: string }>("agent:session_renamed", (e) =>
    cb(e.payload.session_id, e.payload.title),
  );
}
