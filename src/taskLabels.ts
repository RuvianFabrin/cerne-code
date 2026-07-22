// Traduz o nome cru de uma ferramenta (`web_search`, `read_file`, etc.) pra
// uma frase curta em português, usada na timeline do chat — o rótulo cru
// completo (`nome(args truncados)`) continua disponível pra quem expandir o
// passo, isso aqui é só o resumo de uma linha.
const TOOL_VERBS: Record<string, string> = {
  web_search: "Buscou na web",
  web_fetch: "Leu uma página da web",
  load_skill: "Carregou uma skill",
  ask: "Perguntou algo pra você",
  read_file: "Leu um arquivo",
  list_dir: "Listou uma pasta",
  grep: "Buscou um padrão nos arquivos",
  ast_grep: "Buscou na estrutura do código",
  run_command: "Executou um comando",
  check_background_output: "Conferiu um processo em segundo plano",
  stop_background: "Encerrou um processo em segundo plano",
  list_background: "Listou processos em segundo plano",
  write_file: "Criou/sobrescreveu um arquivo",
  edit_file: "Editou um arquivo",
  ast_edit: "Reescreveu estrutura de código",
  task: "Delegou a um sub-agente",
  verify_completion: "Verificou a conclusão da tarefa",
  todo_list: "Atualizou a lista de tarefas",
  computer_use_screenshot: "Capturou a tela",
  computer_use_click: "Clicou na tela",
  computer_use_type_text: "Digitou texto na tela",
  computer_use_press_key: "Pressionou tecla",
  computer_use_list_windows: "Listou janelas abertas",
  computer_use_scroll: "Rolou a tela",
};

export function toolNameFromLabel(rawLabel: string): string {
  return rawLabel.split("(")[0];
}

export function friendlyStepLabel(rawLabel: string): string {
  const name = toolNameFromLabel(rawLabel);
  if (name.startsWith("mcp__")) {
    const parts = name.split("__");
    return `Chamou ferramenta MCP: ${parts.slice(2).join("__") || name}`;
  }
  return TOOL_VERBS[name] ?? `Executou ${name}`;
}
