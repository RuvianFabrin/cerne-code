// Cliente CDP minimo (sem dependencias - usa o WebSocket nativo do Node) pra
// controlar a janela WebView2 de verdade do Cerne via
// --remote-debugging-port=9222. Uso: node scripts/cdp.mjs <comando> [args...]
//
// Por que isso existe: o request_access do computer-use nao reconhece o
// Cerne (nao tem atalho no Menu Iniciar), entao a unica forma de testar a
// janela nativa de verdade (com IPC real, nao uma copia sem Tauri) e anexar
// no WebView2 via debug remoto do Chromium por baixo dele.
//
// Como usar (PowerShell, precisa setar a env var ANTES do `tauri dev`):
//   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
//   npm run tauri dev
// Depois, noutro terminal: node scripts/cdp.mjs screenshot out.png
//
// Comandos:
//   list                          lista os alvos (pages) do CDP
//   screenshot <arquivo.png>      screenshot da pagina inteira
//   eval "<js>"                   roda JS na pagina, imprime o retorno (JSON)
//   click <x> <y>                 clique do mouse em coordenadas de tela (CSS px)
//   type "<texto>"                digita texto (dispara Input.insertText)
//   key <nome>                    tecla especial (Enter, Tab, Escape, etc)
//
// Limitacao conhecida: dialogos nativos do Windows (o file-picker de
// "Escolher pasta" do plugin-dialog, por exemplo) nao fazem parte do DOM da
// pagina, entao o CDP nao alcanca. Pra criar uma sessao com projeto sem
// passar pelo picker, chame o comando Tauri direto via `eval`:
//   eval "window.__TAURI_INTERNALS__.invoke('create_session', { title: 't', provider: 'llama_cpp', model: 'qwen3.5-9b-mtp', projectRoot: 'C:/caminho/com/barra/normal', forkId: 'turboquant' })"
// (use barra normal '/' no caminho, nao '\\' - escapar backslash pelo shell
// costuma corromper o valor antes de chegar no Node).

const PORT = process.env.CDP_PORT || 9222;

async function pickTarget() {
  const res = await fetch(`http://127.0.0.1:${PORT}/json`);
  const targets = await res.json();
  const page = targets.find(t => t.type === "page") || targets[0];
  if (!page) throw new Error("nenhum alvo CDP encontrado");
  return page;
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    ws.addEventListener("open", () => resolve(ws));
    ws.addEventListener("error", (e) => reject(e));
  });
}

function rpc(ws, method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = Math.floor(Math.random() * 1e9);
    const onMessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id === id) {
        ws.removeEventListener("message", onMessage);
        if (msg.error) reject(new Error(JSON.stringify(msg.error)));
        else resolve(msg.result);
      }
    };
    ws.addEventListener("message", onMessage);
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function main() {
  const [, , cmd, ...args] = process.argv;
  const target = await pickTarget();
  const ws = await connect(target.webSocketDebuggerUrl);
  await rpc(ws, "Page.enable");
  await rpc(ws, "Runtime.enable");
  await rpc(ws, "DOM.enable");

  try {
    switch (cmd) {
      case "list": {
        const res = await fetch(`http://127.0.0.1:${PORT}/json`);
        console.log(await res.text());
        break;
      }
      case "screenshot": {
        const outPath = args[0] || "screenshot.png";
        const { data } = await rpc(ws, "Page.captureScreenshot", { format: "png" });
        await import("node:fs").then((fs) => fs.writeFileSync(outPath, Buffer.from(data, "base64")));
        console.log(`salvo em ${outPath}`);
        break;
      }
      case "eval": {
        const expr = args.join(" ");
        const result = await rpc(ws, "Runtime.evaluate", {
          expression: expr,
          returnByValue: true,
          awaitPromise: true,
        });
        if (result.exceptionDetails) {
          console.error(JSON.stringify(result.exceptionDetails, null, 2));
          process.exitCode = 1;
        } else {
          console.log(JSON.stringify(result.result.value, null, 2));
        }
        break;
      }
      case "click": {
        const x = Number(args[0]);
        const y = Number(args[1]);
        await rpc(ws, "Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
        await rpc(ws, "Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
        console.log(`clique em ${x},${y}`);
        break;
      }
      case "type": {
        const text = args.join(" ");
        await rpc(ws, "Input.insertText", { text });
        console.log(`digitado: ${text}`);
        break;
      }
      case "key": {
        const keyMap = {
          Enter: { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 },
          Tab: { key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 },
          Escape: { key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 },
        };
        const k = keyMap[args[0]];
        if (!k) throw new Error(`tecla desconhecida: ${args[0]}`);
        await rpc(ws, "Input.dispatchKeyEvent", { type: "keyDown", ...k });
        await rpc(ws, "Input.dispatchKeyEvent", { type: "keyUp", ...k });
        console.log(`tecla: ${args[0]}`);
        break;
      }
      default:
        console.error("comando desconhecido:", cmd);
        process.exitCode = 1;
    }
  } finally {
    ws.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
