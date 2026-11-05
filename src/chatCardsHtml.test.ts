// Testes dos cartoes do chat. Rodar com: node --test src/chatCardsHtml.test.ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { formatFileSize, widgetSrcdoc, WIDGET_CSP } from "./chatCardsHtml.ts";

test("fragmento vira documento completo com CSP e script de altura", () => {
  const doc = widgetSrcdoc("<p>oi</p>");
  assert.ok(doc.startsWith("<!doctype html>"));
  assert.ok(doc.includes(`content="${WIDGET_CSP}"`));
  assert.ok(doc.includes("cerneWidgetHeight"));
  assert.ok(doc.includes("<p>oi</p>"));
});

test("documento completo mantem o doctype na frente e recebe o CSP no head", () => {
  const doc = widgetSrcdoc("<!doctype html><html><head><title>x</title></head><body>a</body></html>");
  assert.ok(doc.startsWith("<!doctype html>"));
  const cspAt = doc.indexOf("Content-Security-Policy");
  assert.ok(cspAt > doc.indexOf("<head"), "CSP deve ficar dentro do head");
  assert.ok(doc.indexOf("cerneWidgetHeight") < doc.indexOf("</body>"));
});

test("svg sem html ganha o envoltorio e continua com CSP", () => {
  const doc = widgetSrcdoc('<svg width="10" height="10"></svg>');
  assert.ok(doc.includes("<body><svg"));
  assert.ok(doc.includes("Content-Security-Policy"));
});

test("CSP bloqueia rede: nada de default-src aberto", () => {
  assert.ok(WIDGET_CSP.startsWith("default-src 'none'"));
  assert.ok(!WIDGET_CSP.includes("connect-src"));
});

test("formatFileSize escolhe a unidade", () => {
  assert.equal(formatFileSize(512), "512 B");
  assert.equal(formatFileSize(2048), "2.0 KB");
  assert.equal(formatFileSize(5 * 1024 * 1024), "5.0 MB");
});
