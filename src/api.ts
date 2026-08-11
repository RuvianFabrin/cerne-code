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
  reasoning_effort: "off" | "low" | "medium" | "high" | null;
  enabled_mcp_servers: string[] | null;
  fable_method: boolean;
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

export interface McpServerConfig {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
}

export type SearchProviderKind = "auto" | "brave" | "tavily" | "searxng";

export interface SearchConfigView {
  provider: SearchProviderKind;
  searxng_url: string;
  has_brave_key: boolean;
  has_tavily_key: boolean;
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
  updateSessionContextLength: (id: string, contextLength: number | null) =>
    invoke<Session>("update_session_context_length", { id, contextLength }),
  updateSessionReasoningEffort: (id: string, effort: "off" | "low" | "medium" | "high" | null) =>
    invoke<Session>("update_session_reasoning_effort", { id, effort }),
  updateSessionFableMethod: (id: string, enabled: boolean) =>
    invoke<Session>("update_session_fable_method", { id, enabled }),
  updateSessionMcpServers: (id: string, enabledNames: string[] | null) =>
    invoke<Session>("update_session_mcp_servers", { id, enabledNames }),
  checkPathIsDirectory: (path: string) => invoke<boolean>("check_path_is_directory", { path }),
  extractAttachmentText: (path: string) => invoke<string>("extract_attachment_text", { path }),
  checkVisionSupport: (sessionId: string) => invoke<boolean>("check_vision_support", { sessionId }),
  testVision: (kind: string, customProviderId: string | null, model: string) =>
    invoke<boolean>("test_vision", { kind, customProviderId, model }),
  readImageAsDataUrl: (path: string) => invoke<string>("read_image_as_data_url", { path }),
  getSession: (id: string) => invoke<Session>("get_session", { id }),
  getSessionMessages: (id: string) => invoke<ChatMessage[]>("get_session_messages", { id }),
  getSessionTasks: (id: string) => invoke<TaskItem[]>("get_session_tasks", { id }),
  getSessionContextUsage: (id: string) => invoke<ContextUsage>("get_session_context_usage", { id }),
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

  listSkills: (projectRoot: string | null) => invoke<SkillMeta[]>("list_skills", { projectRoot }),
  createSkill: (name: string, description: string, language: SkillLanguage) =>
    invoke<string>("create_skill", { name, description, language }),
  skillTemplateBody: (language: SkillLanguage) => invoke<string>("skill_template_body", { language }),
  readSkill: (dir: string) => invoke<string>("read_skill", { dir }),
  saveSkill: (dir: string, content: string) => invoke<void>("save_skill", { dir, content }),
  openSkillsFolder: () => invoke<void>("open_skills_folder"),
  openExternalUrl: (url: string) => invoke<void>("open_external_url", { url }),

  listMcpServers: () => invoke<McpServerConfig[]>("list_mcp_servers"),
  addMcpServer: (server: McpServerConfig) => invoke<void>("add_mcp_server", { server }),
  removeMcpServer: (name: string) => invoke<void>("remove_mcp_server", { name }),
  testMcpServer: (server: McpServerConfig) => invoke<string[]>("test_mcp_server", { server }),

  getSearchConfig: () => invoke<SearchConfigView>("get_search_config"),
  saveSearchConfig: (provider: SearchProviderKind, searxngUrl: string, apiKey?: string) =>
    invoke<SearchConfigView>("save_search_config", { provider, searxngUrl, apiKey }),
  clearSearchApiKey: (provider: SearchProviderKind) => invoke<SearchConfigView>("clear_search_api_key", { provider }),
  testSearchProvider: (provider: SearchProviderKind, apiKey?: string, searxngUrl?: string) =>
    invoke<number>("test_search_provider", { provider, apiKey, searxngUrl }),
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

export function onToolCall(cb: (sessionId: string, tool: string, args: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; tool: string; args: string }>("agent:tool_call", (e) =>
    cb(e.payload.session_id, e.payload.tool, e.payload.args),
  );
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
