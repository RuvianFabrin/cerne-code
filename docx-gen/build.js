const fs = require("fs");
const {
  Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell,
  WidthType, ShadingType, AlignmentType, BorderStyle, Footer,
  PageNumber, VerticalAlign,
} = require("docx");

const FONT = "Arial";
const W = 9638; // largura útil em A4 com margem de 2 cm
const BORDER = { style: BorderStyle.SINGLE, size: 4, color: "BFBFBF" };
const BORDERS = { top: BORDER, bottom: BORDER, left: BORDER, right: BORDER };
const HEAD_FILL = "1F4E79";
const ALT_FILL = "F2F6FA";
const BOX = "\u2610";

const cell = (text, width, o = {}) =>
  new TableCell({
    borders: BORDERS,
    width: { size: width, type: WidthType.DXA },
    verticalAlign: VerticalAlign.CENTER,
    shading: o.fill ? { fill: o.fill, type: ShadingType.CLEAR, color: "auto" } : undefined,
    margins: { top: 60, bottom: 60, left: 100, right: 100 },
    children: [
      new Paragraph({
        alignment: o.align || AlignmentType.LEFT,
        spacing: { before: 0, after: 0, line: 276 },
        children: [
          new TextRun({
            text,
            font: FONT,
            size: o.size || 20,
            bold: !!o.bold,
            color: o.color || "000000",
          }),
        ],
      }),
    ],
  });

const headerRow = (labels, widths) =>
  new TableRow({
    tableHeader: true,
    children: labels.map((l, i) =>
      cell(l, widths[i], {
        fill: HEAD_FILL,
        bold: true,
        color: "FFFFFF",
        align: i <= 1 ? AlignmentType.LEFT : AlignmentType.CENTER,
      })
    ),
  });

const WIDTHS = [560, 5078, 800, 800, 800, 800, 800];
const LIKERT_HEAD = ["#", "Afirmação", "1", "2", "3", "4", "5"];

const questions = [
  "Sei diferenciar quando usar o Copilot como assistente e quando usar o modo agente, escolhendo o nível de autonomia conforme o risco e o raio de explosão da mudança.",
  "Antes de acionar o Copilot em modo agente, eu defino escopo, restrições, critérios de aceite e Definition of Done da tarefa.",
  "Eu utilizo skills/instruções compartilhadas e versionadas (ex.: copilot-instructions.md, prompt files, padrões de arquitetura, nomenclatura, segurança e testes) para que o Copilot siga as convenções do projeto.",
  "Eu crio e utilizo arquivos de agente customizados para tarefas diferentes — por exemplo, um agente/prompt dedicado a geração de testes, outro para refatoração, outro para revisão, documentação ou migração —, em vez de usar um único prompt genérico para tudo.",
  "Eu utilizo o MDS (Manual de Boas Práticas de Programação do TJPR) como referência para orientar o Copilot a gerar código alinhado aos padrões do TJPR (nomenclatura, arquitetura, segurança, testes etc.).",
  "Antes de aceitar código gerado pelo Copilot, aplico verificação e refatoração (validação semântica, alinhamento ao contexto do projeto, defesa de fronteira) e garanto que ele passa pelos mesmos gates de qualidade de código escrito manualmente (lint, análise estática, testes, cobertura, segurança) antes do commit/PR.",
  "Eu utilizo um checklist específico de revisão para código gerado por IA, além do checklist comum de revisão de código humano.",
  "Quando reviso código gerado pelo Copilot — meu ou de colegas —, faço revisão humana crítica e assumo accountability explícita, mesmo quando há sugestões automáticas.",
  "Na revisão, verifico se o código atende à intenção do requisito, e não apenas se compila ou se os testes passam.",
  "Eu sinalizo nos PRs/commits quando trechos relevantes foram gerados ou co-gerados por IA, permitindo rastreabilidade e auditoria.",
  "Eu gerencio a janela de contexto no Copilot: forneço contexto enxuto e relevante, evito misturar tarefas não relacionadas e reinicio ou compacto a sessão quando a precisão cai.",
  "Eu uso técnicas estruturadas de contexto (arquivos de referência, sumários, seleção de trechos, especificações, exemplos) em vez de despejar o repositório inteiro ou arquivos irrelevantes no prompt.",
  "Sei claramente o que não posso enviar ao Copilot (dados de jurisdicionados, segredos, chaves, tokens, código sensível) e aplico anonimização ou generalização quando necessário.",
  "Eu trato o código gerado pelo Copilot como qualquer outro entregável do Sprint: ele só é considerado pronto quando atende ao Definition of Done do time, incluindo testes, revisão e aderência ao MDS — sem atalhos por ter sido gerado por IA.",
  "Eu acompanho métricas multidimensionais (lead time, change failure rate, retrabalho, qualidade percebida, satisfação) para avaliar o impacto do meu uso do Copilot e ajustar minha forma de trabalhar.",
];

const sections = [
  ["Seção 1 — Agentes e Skills", [1, 2, 3, 4]],
  ["Seção 2 — Padrões de Projeto e Qualidade do Código Gerado", [5, 6, 7]],
  ["Seção 3 — Revisão de Código e Human-in-the-Loop", [8, 9, 10]],
  ["Seção 4 — Otimização de Contexto", [11, 12]],
  ["Seção 5 — Segurança e Governança", [13]],
  ["Seção 6 — Integração com Scrum e Métricas", [14, 15]],
];

const p = (text, opts = {}) =>
  new Paragraph({
    spacing: { after: 120, line: 300 },
    children: [new TextRun({ text, font: FONT, size: 22 })],
    ...opts,
  });

const sectionTitle = (text, before) =>
  new Paragraph({
    keepNext: true,
    spacing: { before, after: 140 },
    border: { bottom: { style: BorderStyle.SINGLE, size: 8, color: HEAD_FILL, space: 4 } },
    children: [new TextRun({ text, font: FONT, size: 26, bold: true, color: HEAD_FILL })],
  });

const body = [];

body.push(
  new Paragraph({
    alignment: AlignmentType.CENTER,
    spacing: { after: 80 },
    children: [new TextRun({ text: "Autoavaliação de Maturidade no Uso do GitHub Copilot", font: FONT, size: 36, bold: true, color: HEAD_FILL })],
  }),
  new Paragraph({
    alignment: AlignmentType.CENTER,
    spacing: { after: 280 },
    children: [new TextRun({ text: "Questionário individual · 15 afirmações", font: FONT, size: 22, color: "595959" })],
  }),
  sectionTitle("Instrução", 120),
  p("Responda considerando sua prática real no dia a dia. Marque um único número por afirmação."),
  new Paragraph({
    keepNext: true,
    spacing: { before: 160, after: 120 },
    children: [new TextRun({ text: "Escala", font: FONT, size: 24, bold: true })],
  })
);

const scale = [
  ["1", "Não faço / não conheço"],
  ["2", "Faço de forma inicial e informal"],
  ["3", "Faço parcialmente, sem consistência"],
  ["4", "Faço de forma estruturada e recorrente"],
  ["5", "Faço de forma estruturada, avalio resultados e melhoro continuamente"],
];
body.push(
  new Table({
    width: { size: W, type: WidthType.DXA },
    columnWidths: [1200, W - 1200],
    rows: scale.map(
      (r, idx) =>
        new TableRow({
          cantSplit: true,
          children: [
            cell(r[0], 1200, { bold: true, align: AlignmentType.CENTER, fill: idx % 2 ? ALT_FILL : undefined }),
            cell(r[1], W - 1200, { fill: idx % 2 ? ALT_FILL : undefined }),
          ],
        })
    ),
  })
);

for (const [title, nums] of sections) {
  body.push(sectionTitle(title, 360));
  const rows = nums.map((n) => [String(n), questions[n - 1], BOX, BOX, BOX, BOX, BOX]);
  body.push(
    new Table({
      width: { size: W, type: WidthType.DXA },
      columnWidths: WIDTHS,
      rows: [
        headerRow(LIKERT_HEAD, WIDTHS),
        ...rows.map(
          (r, idx) =>
            new TableRow({
              cantSplit: true,
              children: r.map((t, i) =>
                cell(t, WIDTHS[i], {
                  fill: idx % 2 === 1 ? ALT_FILL : undefined,
                  align: i === 1 ? AlignmentType.LEFT : AlignmentType.CENTER,
                  bold: i === 0,
                  size: i >= 2 ? 24 : 20,
                })
              ),
            })
        ),
      ],
    })
  );
}

body.push(
  sectionTitle("Leitura do resultado individual", 360),
  p("Some os pontos das 15 questões (mínimo 15, máximo 75) e compare com a faixa abaixo.")
);

const BW = [1500, 2200, W - 3700];
const bands = [
  ["15–30", "Iniciante", "Uso básico, sem processos estruturados. Priorizar fundamentos de revisão, contexto, segurança e alinhamento ao MDS."],
  ["31–45", "Em desenvolvimento", "Há prática, mas inconsistente. Priorizar checklists, skills/agentes customizados, gates de qualidade e uso sistemático do MDS."],
  ["46–60", "Intermediário", "Uso estruturado em várias frentes. Priorizar integração com Scrum, métricas e julgamento de autonomia de agentes."],
  ["61–75", "Avançado", "Prática consolidada e orientada a melhoria contínua. Pode mentorar colegas e contribuir com padrões do TJPR."],
];
body.push(
  new Table({
    width: { size: W, type: WidthType.DXA },
    columnWidths: BW,
    rows: [
      headerRow(["Faixa", "Nível", "Orientação"], BW),
      ...bands.map(
        (b, idx) =>
          new TableRow({
            cantSplit: true,
            children: [
              cell(b[0], BW[0], { bold: true, align: AlignmentType.CENTER, fill: idx % 2 ? ALT_FILL : undefined }),
              cell(b[1], BW[1], { bold: true, fill: idx % 2 ? ALT_FILL : undefined }),
              cell(b[2], BW[2], { fill: idx % 2 ? ALT_FILL : undefined }),
            ],
          })
      ),
    ],
  }),
  new Paragraph({
    spacing: { before: 360, after: 120 },
    children: [new TextRun({ text: "Pontuação total: ______ / 75", font: FONT, size: 24, bold: true })],
  })
);

const doc = new Document({
  creator: "Autoavaliação",
  title: "Autoavaliação de Maturidade no Uso do GitHub Copilot",
  styles: { default: { document: { run: { font: FONT, size: 22 } } } },
  sections: [
    {
      properties: {
        page: {
          size: { width: 11906, height: 16838 },
          margin: { top: 1134, bottom: 1134, left: 1134, right: 1134 },
        },
      },
      footers: {
        default: new Footer({
          children: [
            new Paragraph({
              alignment: AlignmentType.CENTER,
              children: [
                new TextRun({ children: ["Página ", PageNumber.CURRENT, " de ", PageNumber.TOTAL_PAGES], font: FONT, size: 18, color: "7F7F7F" }),
              ],
            }),
          ],
        }),
      },
      children: body,
    },
  ],
});

Packer.toBuffer(doc).then((buf) => {
  const out = "F:/pdfs e word/Autoavaliacao_Maturidade_Copilot.docx";
  fs.writeFileSync(out, buf);
  console.log("wrote", out);
});
