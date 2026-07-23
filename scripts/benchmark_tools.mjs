// Benchmark de TOOL USE — testa se os modelos conseguem usar as ferramentas do Cerne Code.
// Simula o agent loop: envia prompt + tool specs, avalia se a tool call é correta.
// Uso: node scripts/benchmark_tools.mjs

const OLLAMA = "http://127.0.0.1:11434/v1/chat/completions";

const MODELS = [
  "ornith:9b",
  "gemma4:12b-it-qat",
  "qwen3-coder:30b",
  "ministral-3:8b",
  "devstral-small-2:24b",
  "gpt-oss:20b",
];

// Tool specs reais do Cerne (simplificados mas funcionais)
const TOOLS = [
  {
    type: "function",
    function: {
      name: "grep",
      description: "Busca um padrao regex em arquivos do projeto. Retorna linhas com o caminho e numero da linha.",
      parameters: {
        type: "object",
        properties: {
          pattern: { type: "string", description: "Padrao regex de busca" },
          path: { type: "string", description: "Pasta ou arquivo onde buscar (relativo ao projeto)" },
          include: { type: "string", description: "Filtro glob de arquivos (ex: '*.rs')" },
        },
        required: ["pattern"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "read_file",
      description: "Le o conteudo de um arquivo. Use offset e limit para ler arquivos grandes por partes.",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Caminho do arquivo (relativo ao projeto ou absoluto)" },
          offset: { type: "integer", description: "Linha inicial (0-based)" },
          limit: { type: "integer", description: "Quantidade de linhas para ler" },
        },
        required: ["path"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "list_dir",
      description: "Lista os arquivos e pastas de um diretorio.",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Caminho da pasta" },
        },
        required: ["path"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "write_file",
      description: "Cria ou sobrescreve um arquivo com o conteudo fornecido.",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Caminho do arquivo" },
          content: { type: "string", description: "Conteudo do arquivo" },
        },
        required: ["path", "content"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "edit_file",
      description: "Edita um trecho especifico de um arquivo, substituindo old_string por new_string.",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Caminho do arquivo" },
          old_string: { type: "string", description: "Texto exato a ser substituido" },
          new_string: { type: "string", description: "Texto novo no lugar" },
        },
        required: ["path", "old_string", "new_string"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "web_search",
      description: "Pesquisa na internet e retorna resultados com titulo, URL e resumo.",
      parameters: {
        type: "object",
        properties: {
          query: { type: "string", description: "Termo de busca" },
        },
        required: ["query"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "run_command",
      description: "Executa um comando shell e retorna stdout/stderr.",
      parameters: {
        type: "object",
        properties: {
          command: { type: "string", description: "Comando para executar" },
          timeout: { type: "integer", description: "Timeout em segundos (default: 120)" },
        },
        required: ["command"],
      },
    },
  },
];

// Testes: cada um espera uma tool call específica
const TESTS = [
  {
    id: "list_dir",
    label: "Listar pasta",
    prompt: "Liste os arquivos da pasta D:\\SCA_elton",
    expectTool: "list_dir",
    validate(args) {
      const p = (args.path || "").toLowerCase();
      if (p.includes("sca_elton") || p.includes("sca")) return { score: 10, note: `path ok: ${args.path}` };
      if (p.length > 0) return { score: 5, note: `path parcial: ${args.path}` };
      return { score: 0, note: "sem path" };
    },
  },
  {
    id: "grep",
    label: "Buscar arquivo (grep)",
    prompt: "Na pasta D:\\SCA_elton tem um arquivo PDF grande. Procure todos os arquivos .pdf nessa pasta usando a ferramenta de busca.",
    expectTool: "grep",
    validate(args) {
      const pattern = (args.pattern || "").toLowerCase();
      const path = (args.path || "").toLowerCase();
      const include = (args.include || "").toLowerCase();
      let score = 0;
      const notes = [];
      if (pattern.includes("pdf") || include.includes("pdf")) { score += 5; notes.push("busca pdf"); }
      if (path.includes("sca") || path.includes("elton")) { score += 5; notes.push("pasta certa"); }
      if (score === 0 && (pattern.length > 0 || path.length > 0)) { score = 3; notes.push("tentou"); }
      return { score, note: notes.join(", ") || "vazio" };
    },
  },
  {
    id: "read_file",
    label: "Ler arquivo grande",
    prompt: 'Leia o arquivo "D:\\SCA_elton\\Versão Final SCA 0707.pdf". Como ele é grande, leia apenas as primeiras 100 linhas.',
    expectTool: "read_file",
    validate(args) {
      const p = (args.path || "").toLowerCase();
      let score = 0;
      const notes = [];
      if (p.includes("sca") || p.includes("pdf")) { score += 4; notes.push("path ok"); }
      if (args.limit !== undefined && args.limit <= 200) { score += 3; notes.push(`limit=${args.limit}`); }
      if (args.offset !== undefined) { score += 3; notes.push(`offset=${args.offset}`); }
      if (score === 0 && p.length > 0) { score = 3; notes.push("path parcial"); }
      return { score, note: notes.join(", ") || "vazio" };
    },
  },
  {
    id: "edit_file",
    label: "Editar arquivo",
    prompt: 'No arquivo "src/config.ts", troque a linha "const PORT = 3000" por "const PORT = 8080".',
    expectTool: "edit_file",
    validate(args) {
      let score = 0;
      const notes = [];
      const path = (args.path || "").toLowerCase();
      const old = (args.old_string || "");
      const nw = (args.new_string || "");
      if (path.includes("config")) { score += 3; notes.push("path ok"); }
      if (old.includes("3000")) { score += 3; notes.push("old ok"); }
      if (nw.includes("8080")) { score += 4; notes.push("new ok"); }
      if (score === 0 && (path.length > 0 || old.length > 0)) { score = 2; notes.push("tentou"); }
      return { score, note: notes.join(", ") || "vazio" };
    },
  },
  {
    id: "web_search",
    label: "Pesquisar na internet",
    prompt: "Pesquise na internet qual a cotação do dólar hoje.",
    expectTool: "web_search",
    validate(args) {
      const q = (args.query || "").toLowerCase();
      if (q.includes("dólar") || q.includes("dolar") || q.includes("cotação") || q.includes("cotacao")) {
        return { score: 10, note: `query ok: "${args.query}"` };
      }
      if (q.length > 0) return { score: 5, note: `query genérica: "${args.query}"` };
      return { score: 0, note: "sem query" };
    },
  },
  {
    id: "run_command",
    label: "Rodar comando",
    prompt: "Rode o comando 'dir D:\\SCA_elton' para listar os arquivos da pasta.",
    expectTool: "run_command",
    validate(args) {
      const cmd = (args.command || "").toLowerCase();
      if (cmd.includes("dir") && (cmd.includes("sca") || cmd.includes("elton"))) {
        return { score: 10, note: `comando ok: "${args.command}"` };
      }
      if (cmd.includes("dir") || cmd.includes("ls")) return { score: 6, note: `comando parcial: "${args.command}"` };
      if (cmd.length > 0) return { score: 3, note: `comando: "${args.command}"` };
      return { score: 0, note: "sem comando" };
    },
  },
];

async function testModel(model, test) {
  const start = Date.now();
  try {
    const resp = await fetch(OLLAMA, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model,
        messages: [
          {
            role: "system",
            content: "Voce e um assistente de programacao. Use as ferramentas disponiveis para executar as tarefas. NAO explique, apenas chame a ferramenta correta. Responda em portugues.",
          },
          { role: "user", content: test.prompt },
        ],
        tools: TOOLS,
        stream: false,
        max_tokens: 1024,
      }),
    });
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);
    if (!resp.ok) {
      return { model, test: test.id, elapsed, error: `HTTP ${resp.status}`, toolCall: null, score: 0, note: "" };
    }
    const data = await resp.json();
    const msg = data.choices?.[0]?.message;
    const toolCalls = msg?.tool_calls;

    if (!toolCalls || toolCalls.length === 0) {
      // Modelo respondeu com texto em vez de tool call
      const text = (msg?.content || "").slice(0, 100);
      return { model, test: test.id, elapsed, toolCall: null, score: 0, note: `Texto em vez de tool call: "${text}"` };
    }

    const tc = toolCalls[0];
    const fnName = tc.function?.name || "";
    let args = {};
    try { args = JSON.parse(tc.function?.arguments || "{}"); } catch {}

    // Avalia
    if (fnName !== test.expectTool) {
      return { model, test: test.id, elapsed, toolCall: fnName, score: 2, note: `Tool errada: ${fnName} (esperava ${test.expectTool})`, args };
    }

    const ev = test.validate(args);
    return { model, test: test.id, elapsed, toolCall: fnName, score: ev.score, note: ev.note, args };
  } catch (e) {
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);
    return { model, test: test.id, elapsed, error: String(e), toolCall: null, score: 0, note: "" };
  }
}

async function main() {
  console.log("=== BENCHMARK TOOL USE — CERNE CODE ===\n");
  console.log(`Modelos: ${MODELS.length} | Testes: ${TESTS.length} | Máx: ${TESTS.length * 10} pts\n`);

  const results = [];

  for (const model of MODELS) {
    console.log(`\n━━━ ${model} ━━━`);
    for (const test of TESTS) {
      process.stdout.write(`  ${test.label.padEnd(22)} `);
      const r = await testModel(model, test);
      results.push(r);
      const icon = r.score >= 8 ? "✅" : r.score >= 5 ? "⚠️" : "❌";
      console.log(`${icon} ${r.score}/10 | ${r.elapsed}s | ${r.note}`);
      if (r.error) console.log(`     ERRO: ${r.error}`);
    }
  }

  // Ranking
  console.log("\n\n━━━ RANKING TOOL USE ━━━\n");
  const byModel = {};
  for (const r of results) {
    if (!byModel[r.model]) byModel[r.model] = { total: 0, time: 0, count: 0, details: [] };
    byModel[r.model].total += r.score;
    byModel[r.model].time += parseFloat(r.elapsed);
    byModel[r.model].count++;
    byModel[r.model].details.push(`${r.test}:${r.score}`);
  }

  const maxScore = TESTS.length * 10;
  const ranking = Object.entries(byModel)
    .map(([model, d]) => ({
      model,
      score: d.total,
      avgTime: (d.time / d.count).toFixed(1),
      details: d.details.join(" | "),
    }))
    .sort((a, b) => b.score - a.score || parseFloat(a.avgTime) - parseFloat(b.avgTime));

  console.log(`Pos | Modelo                     | Nota     | Tempo méd | Detalhe`);
  console.log(`----|----------------------------|----------|-----------|--------`);
  ranking.forEach((r, i) => {
    const medal = i === 0 ? "🥇" : i === 1 ? "🥈" : i === 2 ? "🥉" : `${i + 1}`.padStart(2);
    console.log(`${medal} | ${r.model.padEnd(26)} | ${String(r.score).padStart(2)}/${maxScore}  | ${r.avgTime.padStart(7)}s | ${r.details}`);
  });

  // Relatório por teste
  console.log("\n\n━━━ POR TESTE ━━━\n");
  for (const test of TESTS) {
    console.log(`\n${test.label} (${test.expectTool}):`);
    const testResults = results.filter((r) => r.test === test.id).sort((a, b) => b.score - a.score);
    for (const r of testResults) {
      const icon = r.score >= 8 ? "✅" : r.score >= 5 ? "⚠️" : "❌";
      console.log(`  ${icon} ${r.model.padEnd(26)} ${r.score}/10 — ${r.note}`);
    }
  }

  const fs = await import("fs");
  fs.writeFileSync("scripts/benchmark_tools_result.json", JSON.stringify({ date: new Date().toISOString(), results, ranking }, null, 2));
  console.log("\n\nResultado salvo em scripts/benchmark_tools_result.json");
}

main().catch(console.error);
