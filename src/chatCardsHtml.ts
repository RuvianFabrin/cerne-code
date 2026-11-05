// Funcoes puras dos cartoes do chat (widget e arquivo). Sem Vue, pra testar direto.

export const WIDGET_CSP =
  "default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; font-src data:";

// Widget roda num iframe sem same-origin e sem rede: o CSP bloqueia qualquer
// recurso externo. O script so informa a altura pro pai, que ajusta o iframe.
export const WIDGET_HEIGHT_SCRIPT =
  '<script>(function(){function h(){parent.postMessage({cerneWidgetHeight:document.documentElement.scrollHeight},"*")}window.addEventListener("load",h);if(window.ResizeObserver){new ResizeObserver(h).observe(document.documentElement)}})();<\/script>';

export function widgetSrcdoc(code: string): string {
  const trimmed = code.trim();
  let doc = /^<(!doctype|html)/i.test(trimmed)
    ? trimmed
    : `<!doctype html><html><head><meta charset="utf-8"><style>body{margin:0;font-family:system-ui,sans-serif;color:#222;background:transparent}</style></head><body>${trimmed}</body></html>`;
  // O CSP precisa vir no <head> (ou logo apos o doctype), senao o modo quirks entra.
  const csp = `<meta http-equiv="Content-Security-Policy" content="${WIDGET_CSP}">`;
  doc = /<head[^>]*>/i.test(doc)
    ? doc.replace(/<head[^>]*>/i, (m) => m + csp)
    : doc.replace(/^(<!doctype[^>]*>)?/i, (m) => m + csp);
  return /<\/body>/i.test(doc)
    ? doc.replace(/<\/body>/i, WIDGET_HEIGHT_SCRIPT + "</body>")
    : doc + WIDGET_HEIGHT_SCRIPT;
}


export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
