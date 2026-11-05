// Converte um arquivo HTML em PDF usando o Edge ou o Chrome ja instalados
// na maquina (canal do Playwright), sem baixar navegador.
//
// Uso: node html_to_pdf.js <entrada.html> <saida.pdf> [A4|Letter|...] [landscape: true|false]
//
// Rede bloqueada: so file:, data: e about: carregam. Imagens e fontes externas
// somem do PDF em vez de vazar requisicoes.

const fs = require("fs");
const { chromium } = require("playwright-core");

const ALLOWED_SCHEMES = ["file:", "data:", "about:", "blob:"];

async function launchBrowser() {
  const errors = [];
  for (const channel of ["msedge", "chrome"]) {
    try {
      return await chromium.launch({ channel, headless: true });
    } catch (error) {
      errors.push(`${channel}: ${error.message.split("\n")[0]}`);
    }
  }
  throw new Error(
    "nao encontrei Edge nem Chrome instalados para gerar o PDF. " + errors.join(" | ")
  );
}

async function main() {
  const [, , inputPath, outputPath, format = "A4", landscape = "false"] = process.argv;
  if (!inputPath || !outputPath) {
    throw new Error("uso: html_to_pdf.js <entrada.html> <saida.pdf> [formato] [landscape]");
  }
  const html = fs.readFileSync(inputPath, "utf8");

  const browser = await launchBrowser();
  try {
    const page = await browser.newPage();
    await page.route("**/*", (route) => {
      const url = route.request().url();
      const allowed = ALLOWED_SCHEMES.some((scheme) => url.startsWith(scheme));
      return allowed ? route.continue() : route.abort();
    });
    await page.setContent(html, { waitUntil: "load" });
    await page.emulateMedia({ media: "print" });
    await page.pdf({
      path: outputPath,
      format,
      landscape: landscape === "true",
      printBackground: true,
    });
  } finally {
    await browser.close();
  }
  console.log(`PDF gerado: ${outputPath}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
