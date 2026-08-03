export interface ReadyPrompt {
  id: string;
  tools: string[];
  scope: "chat" | "code" | "both";
}

// Titulo/toolsLabel/preview/full de cada prompt ficam nos arquivos de
// locale (chave `readyPrompts.<id>.*`) - só o metadado que não muda por
// idioma (id, ferramentas usadas, escopo) mora aqui.
export const READY_PROMPTS: ReadyPrompt[] = [
  { id: "search-summarize", tools: ["web_search", "web_fetch"], scope: "both" },
  { id: "read-large-file", tools: ["read_file"], scope: "code" },
  { id: "create-file", tools: ["write_file"], scope: "code" },
  { id: "edit-file", tools: ["edit_file"], scope: "code" },
  { id: "run-command", tools: ["run_command"], scope: "code" },
  { id: "computer-use", tools: ["computer_use_screenshot", "computer_use_click", "computer_use_type_text"], scope: "both" },
  { id: "create-excel", tools: ["create_excel"], scope: "both" },
  { id: "create-word", tools: ["create_word"], scope: "both" },
  { id: "create-pdf", tools: ["create_pdf"], scope: "both" },
  { id: "refactor-code", tools: ["ast_grep", "ast_edit"], scope: "code" },
  { id: "subagent-task", tools: ["task"], scope: "code" },
  { id: "plan-todo", tools: ["todo_list"], scope: "both" },
  { id: "grep-search", tools: ["grep"], scope: "code" },
  { id: "fetch-page", tools: ["web_fetch"], scope: "both" },
];
