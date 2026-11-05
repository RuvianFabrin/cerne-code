//! Overlay do Computer Use: uma janela transparente, sempre no topo, sem
//! barra de titulo e com cliques atravessando, que mostra a borda azul, o
//! aviso "Computer Use" e um anel no ponto da ultima acao.
//!
//! O overlay e so desenho. O mouse de verdade continua sendo o do usuario
//! (o computer use move o cursor real), entao o anel mostra onde a acao foi
//! feita, nao um cursor separado.
//!
//! No Windows a janela e marcada como excluida de captura de tela
//! (`WDA_EXCLUDEFROMCAPTURE`), pra que a borda nao entre nas screenshots que o
//! modelo recebe. Em outros sistemas a marcacao nao existe, e a borda pode
//! aparecer nas capturas.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

pub const OVERLAY_LABEL: &str = "computer-use-overlay";

/// Segundos sem acao de computer use antes de o overlay sumir.
const IDLE_HIDE_SECS: u64 = 6;

/// Cada acao gera um numero novo; so o timer da ultima acao esconde a janela.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Garante que a janela existe. Cria escondida e no tamanho do monitor primario.
fn ensure_window(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        return Ok(window);
    }
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .ok_or_else(|| anyhow!("nenhum monitor primario encontrado"))?;
    let pos = monitor.position();
    let size = monitor.size();
    let window = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("index.html?overlay=1".into()))
        .title("Computer Use")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .resizable(false)
        .visible(false)
        .position(pos.x as f64, pos.y as f64)
        .inner_size(size.width as f64, size.height as f64)
        .build()
        .map_err(|e| anyhow!("falha ao criar overlay: {e}"))?;
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_position(PhysicalPosition::new(pos.x, pos.y));
    let _ = window.set_size(PhysicalSize::new(size.width, size.height));
    #[cfg(windows)]
    exclude_from_capture(&window);
    Ok(window)
}

#[cfg(windows)]
fn exclude_from_capture(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
    if let Ok(hwnd) = window.hwnd() {
        // O HWND do Tauri e do crate `windows` de outra versao; o ponteiro cru e o mesmo.
        let raw = HWND(hwnd.0 as isize as *mut std::ffi::c_void);
        unsafe {
            let _ = SetWindowDisplayAffinity(raw, WDA_EXCLUDEFROMCAPTURE);
        }
    }
}

/// Mostra o overlay pra uma acao de computer use. `x`/`y` (coordenadas de
/// tela do monitor primario) posicionam o anel; sem eles, so a borda aparece.
pub fn show_action(app: &AppHandle, x: Option<i32>, y: Option<i32>) {
    let Ok(window) = ensure_window(app) else { return };
    let _ = window.show();
    let monitor_origin = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| (m.position().x, m.position().y))
        .unwrap_or((0, 0));
    // Coordenadas do monitor primario viram relativas a janela (que cobre o monitor).
    let payload = json!({
        "x": x.map(|v| v - monitor_origin.0),
        "y": y.map(|v| v - monitor_origin.1),
        "stamp": chrono::Utc::now().timestamp_millis(),
    });
    let _ = window.emit("overlay:action", payload);

    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(IDLE_HIDE_SECS)).await;
        if GENERATION.load(Ordering::SeqCst) == generation {
            if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
                let _ = window.hide();
            }
        }
    });
}
