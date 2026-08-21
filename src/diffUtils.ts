// Parser de diff compartilhado entre TaskStepGroup.vue (diff por passo, no
// histórico do chat) e RepoDiffViewer.vue (diff agregado da sessão inteira,
// Fase D1 do roteiro de Agentes/Skills) — extraído de TaskStepGroup.vue pra
// não duplicar a lógica entre os dois.

// t.detail pras ferramentas de escrita vem como "<frase>. Diff:\n<diff
// unificado>" (ver agent/tools.rs) — separa a frase (nota) do diff de
// verdade, que a gente colore linha a linha em vez de jogar tudo cru
// num bloco de texto só.
export function splitDiffDetail(detail: string | null | undefined): { note: string; diffText: string } {
  const raw = detail ?? "";
  const marker = "Diff:\n";
  const idx = raw.indexOf(marker);
  if (idx === -1) return { note: raw, diffText: "" };
  return { note: raw.slice(0, idx + "Diff:".length), diffText: raw.slice(idx + marker.length) };
}

export type DiffLineKind = "add" | "del" | "hunk" | "header" | "context";

// Extraído pra função própria (2026-08-17) pra `RepoDiffViewer.vue` poder
// colorir um diff vindo direto do `git` (sem o prefixo "nota. Diff:\n" que
// `t.detail` das tool calls usa) reaproveitando a mesma lógica de
// classificação linha-a-linha.
export function classifyDiffLines(diffText: string): { kind: DiffLineKind; text: string }[] {
  if (!diffText) return [];
  return diffText
    .split("\n")
    .filter((line, idx, arr) => !(line === "" && idx === arr.length - 1))
    .map((line) => {
      if (line.startsWith("+++") || line.startsWith("---")) return { kind: "header" as const, text: line };
      if (line.startsWith("@@")) return { kind: "hunk" as const, text: line };
      if (line.startsWith("+")) return { kind: "add" as const, text: line };
      if (line.startsWith("-")) return { kind: "del" as const, text: line };
      return { kind: "context" as const, text: line };
    });
}

export function diffLines(detail: string | null | undefined): { kind: DiffLineKind; text: string }[] {
  const { diffText } = splitDiffDetail(detail);
  return classifyDiffLines(diffText);
}
