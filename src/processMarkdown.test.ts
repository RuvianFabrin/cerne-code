// Testes do markdown do historico de processo. Rodar com: node --test src/processMarkdown.test.ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { fenceFor, processToMarkdown, type ProcessLabels } from "./processMarkdown.ts";

const labels: ProcessLabels = {
  title: "Processo",
  status: "Status",
  started: "Iniciado",
  command: "Comando",
  output: "Saída",
  noOutput: "Sem saída",
  running: "rodando",
  exited: "encerrado",
  unknown: "desconhecido",
};

test("cerca usa mais crases quando o conteudo ja tem crases", () => {
  assert.equal(fenceFor("sem crase"), "```");
  assert.equal(fenceFor("tem ``` aqui"), "````");
});

test("historico encerrado mostra codigo e saida em blocos", () => {
  const md = processToMarkdown(
    { command: "npm run build", output: "ok\nfeito", started_at_ms: 0, status: { kind: "exited", code: 0 } },
    labels,
  );
  assert.ok(md.startsWith("# Processo"));
  assert.ok(md.includes("**Status:** encerrado (0)"));
  assert.ok(md.includes("```\nnpm run build\n```"));
  assert.ok(md.includes("```text\nok\nfeito\n```"));
});

test("processo rodando sem saida", () => {
  const md = processToMarkdown(
    { command: "vite", output: "", started_at_ms: 0, status: { kind: "running" } },
    labels,
  );
  assert.ok(md.includes("**Status:** rodando"));
  assert.ok(md.includes("*Sem saída*"));
});

test("saida com crases nao quebra a cerca", () => {
  const md = processToMarkdown(
    { command: "echo", output: "```js\nx\n```", started_at_ms: 0, status: { kind: "exited", code: 1 } },
    labels,
  );
  assert.ok(md.includes("````text\n```js"));
});
