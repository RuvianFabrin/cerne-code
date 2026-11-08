// Monta o historico de um processo em segundo plano como markdown, pro modal.
// Funcao pura (sem Vue) pra poder testar direto.

export interface ProcessLabels {
  title: string;
  status: string;
  started: string;
  command: string;
  output: string;
  noOutput: string;
  running: string;
  exited: string;
  unknown: string;
}

export interface ProcessSnapshot {
  command: string;
  output: string;
  started_at_ms: number;
  status: { kind: "running" } | { kind: "exited"; code: number | null } | { kind: "unknown" };
}

/** Cerca de codigo que nao quebra se o conteudo tiver crases. */
export function fenceFor(content: string): string {
  const longest = (content.match(/`+/g) ?? []).reduce((max, run) => Math.max(max, run.length), 0);
  return "`".repeat(Math.max(3, longest + 1));
}

export function statusText(status: ProcessSnapshot["status"], labels: ProcessLabels): string {
  if (status.kind === "running") return labels.running;
  if (status.kind === "exited") return `${labels.exited} (${status.code ?? "?"})`;
  return labels.unknown;
}

export function processToMarkdown(job: ProcessSnapshot, labels: ProcessLabels, locale = "pt-BR"): string {
  const started = job.started_at_ms
    ? new Date(job.started_at_ms).toLocaleString(locale)
    : "-";
  const output = job.output.trimEnd();
  const outputFence = fenceFor(output || " ");
  const outputBlock = output
    ? `${outputFence}text\n${output}\n${outputFence}`
    : `*${labels.noOutput}*`;
  const commandFence = fenceFor(job.command);
  return [
    `# ${labels.title}`,
    "",
    `**${labels.status}:** ${statusText(job.status, labels)}  `,
    `**${labels.started}:** ${started}`,
    "",
    `## ${labels.command}`,
    "",
    `${commandFence}\n${job.command}\n${commandFence}`,
    "",
    `## ${labels.output}`,
    "",
    outputBlock,
    "",
  ].join("\n");
}
