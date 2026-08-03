import { i18n } from "./i18n";

// Nomes crus de ferramenta (`web_search`, `read_file`, etc.) que tem uma
// chave de locale dedicada em `toolLabels.*` — o rótulo cru completo
// (`nome(args truncados)`) continua disponível pra quem expandir o passo,
// isso aqui é só o resumo de uma linha.
const TOOL_NAMES = [
  "web_search",
  "web_fetch",
  "load_skill",
  "ask",
  "read_file",
  "list_dir",
  "grep",
  "ast_grep",
  "run_command",
  "check_background_output",
  "stop_background",
  "list_background",
  "write_file",
  "edit_file",
  "ast_edit",
  "task",
  "verify_completion",
  "todo_list",
  "computer_use_screenshot",
  "computer_use_click",
  "computer_use_type_text",
  "computer_use_press_key",
  "computer_use_list_windows",
  "computer_use_focus_window",
  "computer_use_scroll",
  "computer_use_authorize",
  "computer_use_browser_execute",
  "computer_use_get_window_state",
  "computer_use_click_element",
  "create_excel",
] as const;

export function toolNameFromLabel(rawLabel: string): string {
  return rawLabel.split("(")[0];
}

export function friendlyStepLabel(rawLabel: string): string {
  const name = toolNameFromLabel(rawLabel);
  if (name.startsWith("mcp__")) {
    const parts = name.split("__");
    return i18n.global.t("toolLabels.mcpTool", { tool: parts.slice(2).join("__") || name });
  }
  if ((TOOL_NAMES as readonly string[]).includes(name)) {
    return i18n.global.t(`toolLabels.${name}`);
  }
  return i18n.global.t("toolLabels.genericTool", { name });
}

export function formatElapsed(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const d = Math.floor(totalSec / 86400);
  const h = Math.floor((totalSec % 86400) / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (d > 0) return `${d}d ${h}h ${m}m ${s}s`;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}
