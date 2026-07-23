// Benchmark de modelos locais — testa raciocínio e código, mede velocidade.
// Uso: node scripts/benchmark.mjs

const OLLAMA = "http://127.0.0.1:11434/v1/chat/completions";

const MODELS = [
  "qwen3.5:9b",
  "qwen3.5:4b",
  "qwen3-coder:30b",
  "gemma4:12b-it-qat",
  "ministral-3:8b",
  "devstral-small-2:24b",
  "gpt-oss:20b",
  "ornith:9b",
];

const TESTS = [
  {
    id: "raciocinio",
    label: "Raciocínio",
    prompt:
      "Se João tem 3 maçãs e Maria tem o dobro de João, e Pedro tem a metade do total de João e Maria juntos, quantas maçãs Pedro tem? Responda apenas com o número e uma explicação de 1 linha.",
  },
  {
    id: "codigo",
    label: "Código",
    prompt:
      'Escreva uma função Python chamada "inverte_palavras" que recebe uma string e retorna cada palavra invertida (ex: "ola mundo" → "alo odnum"). Responda APENAS com o código, sem explicação.',
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
        messages: [{ role: "user", content: test.prompt }],
        stream: false,
        max_tokens: 512,
      }),
    });
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);
    if (!resp.ok) {
      return { model, test: test.id, elapsed, error: `HTTP ${resp.status}`, content: "", tokens: 0 };
    }
    const data = await resp.json();
    const content = data.choices?.[0]?.message?.content ?? "";
    const tokens = data.usage?.completion_tokens ?? 0;
    const tps = tokens > 0 ? (tokens / parseFloat(elapsed)).toFixed(1) : "0";
    return { model, test: test.id, elapsed, content: content.trim(), tokens, tps };
  } catch (e) {
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);
    return { model, test: test.id, elapsed, error: String(e), content: "", tokens: 0 };
  }
}

// Avaliação simples de qualidade
function evaluate(result, testId) {
  if (result.error) return { score: 0, note: `ERRO: ${result.error}` };
  const c = result.content.toLowerCase();

  if (testId === "raciocinio") {
    // Resposta correta: Pedro tem 4.5 → mas como é maçã, 4 ou 5.
    // João=3, Maria=6, total=9, Pedro=9/2=4.5
    if (c.includes("4.5") || c.includes("4,5")) return { score: 10, note: "Correto (4.5)" };
    if (c.includes("4") || c.includes("5")) return { score: 6, note: "Parcial (arredondou)" };
    return { score: 2, note: "Incorreto" };
  }

  if (testId === "codigo") {
    let score = 0;
    const notes = [];
    if (c.includes("def inverte_palavras")) { score += 3; notes.push("função ok"); }
    if (c.includes("split")) { score += 2; notes.push("usa split"); }
    if (c.includes("[::-1]") || c.includes("reverse") || c.includes("reversed")) { score += 3; notes.push("inversão ok"); }
    if (c.includes("join")) { score += 2; notes.push("usa join"); }
    if (score === 0 && c.length > 20) { score = 3; notes.push("tentou mas formato errado"); }
    return { score, note: notes.join(", ") || "vazio" };
  }

  return { score: 0, note: "?" };
}

async function main() {
  console.log("=== BENCHMARK DE MODELOS LOCAIS ===\n");
  const results = [];

  for (const model of MODELS) {
    console.log(`\n--- ${model} ---`);
    for (const test of TESTS) {
      process.stdout.write(`  ${test.label}... `);
      const r = await testModel(model, test);
      const ev = evaluate(r, test.id);
      r.evalScore = ev.score;
      r.evalNote = ev.note;
      results.push(r);
      console.log(`${r.elapsed}s | ${r.tokens} tok | ${r.tps} tok/s | nota ${ev.score}/10 (${ev.note})`);
      if (r.error) console.log(`    ERRO: ${r.error}`);
      else console.log(`    → ${r.content.slice(0, 120).replace(/\n/g, " ")}${r.content.length > 120 ? "..." : ""}`);
    }
  }

  // Ranking
  console.log("\n\n=== RANKING FINAL ===\n");
  const byModel = {};
  for (const r of results) {
    if (!byModel[r.model]) byModel[r.model] = { total: 0, time: 0, tps: [], count: 0 };
    byModel[r.model].total += r.evalScore;
    byModel[r.model].time += parseFloat(r.elapsed);
    if (parseFloat(r.tps) > 0) byModel[r.model].tps.push(parseFloat(r.tps));
    byModel[r.model].count++;
  }

  const ranking = Object.entries(byModel)
    .map(([model, d]) => ({
      model,
      score: d.total,
      maxScore: d.count * 10,
      avgTime: (d.time / d.count).toFixed(1),
      avgTps: d.tps.length > 0 ? (d.tps.reduce((a, b) => a + b, 0) / d.tps.length).toFixed(1) : "0",
    }))
    .sort((a, b) => b.score - a.score || parseFloat(a.avgTime) - parseFloat(b.avgTime));

  console.log("Pos | Modelo                     | Nota  | Tempo méd | Tok/s");
  console.log("----|----------------------------|-------|-----------|------");
  ranking.forEach((r, i) => {
    console.log(
      `${String(i + 1).padStart(3)} | ${r.model.padEnd(26)} | ${String(r.score).padStart(2)}/${r.maxScore} | ${r.avgTime.padStart(7)}s | ${r.avgTps}`
    );
  });

  // Salva JSON
  const fs = await import("fs");
  fs.writeFileSync("scripts/benchmark_result.json", JSON.stringify({ date: new Date().toISOString(), results, ranking }, null, 2));
  console.log("\nResultado salvo em scripts/benchmark_result.json");
}

main().catch(console.error);
