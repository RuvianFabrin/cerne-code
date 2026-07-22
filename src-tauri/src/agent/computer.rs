use crate::models::{ToolFunctionSpec, ToolSpec};
use anyhow::{anyhow, Result};
use enigo::{Enigo, Key, Keyboard, Mouse, Settings};
use serde_json::{json, Value};
use std::time::Instant;

static LAST_ACTION: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
const MIN_ACTION_INTERVAL_MS: u128 = 800;

fn rate_limit() -> Result<()> {
    let mut last = LAST_ACTION.lock().unwrap();
    if let Some(t) = *last {
        let elapsed = t.elapsed().as_millis();
        if elapsed < MIN_ACTION_INTERVAL_MS {
            let wait = MIN_ACTION_INTERVAL_MS - elapsed;
            std::thread::sleep(std::time::Duration::from_millis(wait as u64));
        }
    }
    *last = Some(Instant::now());
    Ok(())
}

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "computer_use_screenshot",
            "Captura screenshot da tela inteira ou de uma janela especifica. Retorna a imagem para voce analisar visualmente. Use ANTES de qualquer click ou type para ver o estado atual da tela. REQUER modelo com suporte a visao.",
            json!({
                "type": "object",
                "properties": {
                    "window_title": { "type": "string", "description": "Titulo parcial da janela (opcional, vazio = tela inteira)" }
                },
                "required": []
            }),
        ),
        spec(
            "computer_use_click",
            "Clica em coordenadas de tela (pixels). SEMPRE chame computer_use_screenshot ANTES para ver onde vai clicar. O resultado inclui um screenshot pos-clique para confirmar o efeito. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "Coordenada X em pixels de tela" },
                    "y": { "type": "integer", "description": "Coordenada Y em pixels de tela" },
                    "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Botao do mouse (default: left)" }
                },
                "required": ["x", "y"]
            }),
        ),
        spec(
            "computer_use_type_text",
            "Digita texto via teclado no elemento focado. Use computer_use_click ANTES para focar o campo desejado. Max 500 chars por chamada. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Texto a digitar (max 500 chars)" }
                },
                "required": ["text"]
            }),
        ),
        spec(
            "computer_use_press_key",
            "Pressiona uma tecla ou combinacao (ex: ctrl+c, alt+tab, enter). Combinacoes destrutivas (alt+f4, ctrl+shift+esc) sao bloqueadas. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Tecla: return, tab, escape, up, down, left, right, space, delete, home, end, pageup, pagedown, f1-f12, ou letra/digito" },
                    "modifiers": { "type": "array", "items": { "type": "string" }, "description": "Modificadores: ctrl, shift, alt, win" }
                },
                "required": ["key"]
            }),
        ),
        spec(
            "computer_use_list_windows",
            "Lista janelas visiveis com PID, titulo e geometria (x,y,width,height). Use para descobrir o PID/titulo antes de screenshot ou click. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        spec(
            "computer_use_scroll",
            "Rola a tela ou janela focada. Use computer_use_click ANTES para garantir que a janela certa esta focada. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["up", "down", "left", "right"] },
                    "amount": { "type": "integer", "description": "Quantidade de linhas/scrolls (default: 3)" }
                },
                "required": ["direction"]
            }),
        ),
    ]
}

fn spec(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        kind: "function".to_string(),
        function: ToolFunctionSpec {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

pub struct ComputerOutcome {
    pub text: String,
    pub screenshot_base64: Option<String>,
}

pub fn execute(name: &str, args: &Value) -> Result<ComputerOutcome> {
    match name {
        "computer_use_screenshot" => exec_screenshot(args),
        "computer_use_click" => exec_click(args),
        "computer_use_type_text" => exec_type_text(args),
        "computer_use_press_key" => exec_press_key(args),
        "computer_use_list_windows" => exec_list_windows(),
        "computer_use_scroll" => exec_scroll(args),
        _ => Err(anyhow!("computer_use tool desconhecida: {name}")),
    }
}

fn rgba_to_base64(img: image::RgbaImage) -> Result<String> {
    use base64::Engine;
    let dyn_img = image::DynamicImage::ImageRgba8(img);
    let mut buf = std::io::Cursor::new(Vec::new());
    dyn_img
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| anyhow!("falha ao codificar PNG: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

fn capture_screen_base64(window_title: Option<&str>) -> Result<(String, u32, u32)> {
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| anyhow!("falha ao listar monitores: {e}"))?;
    let monitor = monitors
        .first()
        .ok_or_else(|| anyhow!("nenhum monitor encontrado"))?;

    let img = if let Some(title) = window_title {
        use xcap::Window;
        let windows = Window::all().map_err(|e| anyhow!("falha ao listar janelas: {e}"))?;
        let title_lower = title.to_lowercase();
        let win = windows
            .iter()
            .find(|w| w.title().unwrap_or_default().to_lowercase().contains(&title_lower))
            .ok_or_else(|| anyhow!("janela com titulo contendo '{title}' nao encontrada"))?;
        win.capture_image()
            .map_err(|e| anyhow!("falha ao capturar janela: {e}"))?
    } else {
        monitor
            .capture_image()
            .map_err(|e| anyhow!("falha ao capturar tela: {e}"))?
    };

    let (w, h) = (img.width(), img.height());
    let b64 = rgba_to_base64(img)?;
    Ok((b64, w, h))
}

fn exec_screenshot(args: &Value) -> Result<ComputerOutcome> {
    let window_title = args["window_title"].as_str();
    let (b64, w, h) = capture_screen_base64(window_title)?;
    Ok(ComputerOutcome {
        text: format!("Screenshot capturado ({w}x{h}px). Analise a imagem para decidir a proxima acao."),
        screenshot_base64: Some(b64),
    })
}

fn exec_click(args: &Value) -> Result<ComputerOutcome> {
    rate_limit()?;
    let x = args["x"]
        .as_i64()
        .ok_or_else(|| anyhow!("x obrigatorio (integer)"))? as i32;
    let y = args["y"]
        .as_i64()
        .ok_or_else(|| anyhow!("y obrigatorio (integer)"))? as i32;
    let button_str = args["button"].as_str().unwrap_or("left");

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("falha ao inicializar enigo: {e}"))?;

    enigo
        .move_mouse(x, y, enigo::Coordinate::Abs)
        .map_err(|e| anyhow!("falha ao mover mouse: {e}"))?;

    let button = match button_str {
        "right" => enigo::Button::Right,
        "middle" => enigo::Button::Middle,
        _ => enigo::Button::Left,
    };

    enigo
        .button(button, enigo::Direction::Click)
        .map_err(|e| anyhow!("falha ao clicar: {e}"))?;

    std::thread::sleep(std::time::Duration::from_millis(300));

    let (b64, w, h) = capture_screen_base64(None)?;
    Ok(ComputerOutcome {
        text: format!("Clique {button_str} em ({x},{y}) executado. Screenshot pos-clique ({w}x{h}px) anexado — verifique se o efeito foi o esperado."),
        screenshot_base64: Some(b64),
    })
}

fn exec_type_text(args: &Value) -> Result<ComputerOutcome> {
    rate_limit()?;
    let text = args["text"]
        .as_str()
        .ok_or_else(|| anyhow!("text obrigatorio"))?;
    if text.len() > 500 {
        return Err(anyhow!(
            "texto muito longo ({} chars, max 500). Divida em chamadas menores.",
            text.len()
        ));
    }

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("falha ao inicializar enigo: {e}"))?;
    enigo
        .text(text)
        .map_err(|e| anyhow!("falha ao digitar: {e}"))?;

    std::thread::sleep(std::time::Duration::from_millis(200));
    let (b64, w, h) = capture_screen_base64(None)?;
    Ok(ComputerOutcome {
        text: format!(
            "Texto digitado ({} chars). Screenshot pos-digitacao ({w}x{h}px) anexado.",
            text.len()
        ),
        screenshot_base64: Some(b64),
    })
}

const BLOCKED_COMBOS: &[(&str, &[&str])] = &[
    ("f4", &["alt"]),
    ("escape", &["ctrl", "shift"]),
    ("delete", &["ctrl", "shift"]),
];

fn is_blocked_combo(key: &str, modifiers: &[String]) -> bool {
    let key_lower = key.to_lowercase();
    let mods_lower: Vec<String> = modifiers.iter().map(|m| m.to_lowercase()).collect();
    for (blocked_key, blocked_mods) in BLOCKED_COMBOS {
        if key_lower == *blocked_key {
            let all_present = blocked_mods
                .iter()
                .all(|bm| mods_lower.iter().any(|m| m == bm));
            if all_present {
                return true;
            }
        }
    }
    false
}

fn parse_key(name: &str) -> Result<Key> {
    match name.to_lowercase().as_str() {
        "return" | "enter" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "escape" | "esc" => Ok(Key::Escape),
        "space" => Ok(Key::Space),
        "delete" | "del" => Ok(Key::Delete),
        "backspace" => Ok(Key::Backspace),
        "up" => Ok(Key::UpArrow),
        "down" => Ok(Key::DownArrow),
        "left" => Ok(Key::LeftArrow),
        "right" => Ok(Key::RightArrow),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        c if c.len() == 1 => {
            let ch = c.chars().next().unwrap();
            Ok(Key::Unicode(ch))
        }
        _ => Err(anyhow!("tecla desconhecida: {name}")),
    }
}

fn parse_modifier(name: &str) -> Result<Key> {
    match name.to_lowercase().as_str() {
        "ctrl" | "control" => Ok(Key::Control),
        "shift" => Ok(Key::Shift),
        "alt" => Ok(Key::Alt),
        "win" | "cmd" | "command" | "meta" => Ok(Key::Meta),
        _ => Err(anyhow!("modificador desconhecido: {name}")),
    }
}

fn exec_press_key(args: &Value) -> Result<ComputerOutcome> {
    rate_limit()?;
    let key_name = args["key"]
        .as_str()
        .ok_or_else(|| anyhow!("key obrigatorio"))?;
    let modifiers: Vec<String> = args["modifiers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if is_blocked_combo(key_name, &modifiers) {
        return Err(anyhow!(
            "combinacao bloqueada por seguranca: {}+{key_name}. Use o sistema manualmente para esta acao.",
            modifiers.join("+")
        ));
    }

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("falha ao inicializar enigo: {e}"))?;

    for m in &modifiers {
        let mk = parse_modifier(m)?;
        enigo
            .key(mk, enigo::Direction::Press)
            .map_err(|e| anyhow!("falha ao pressionar modificador {m}: {e}"))?;
    }

    let key = parse_key(key_name)?;
    enigo
        .key(key, enigo::Direction::Click)
        .map_err(|e| anyhow!("falha ao pressionar tecla: {e}"))?;

    for m in modifiers.iter().rev() {
        let mk = parse_modifier(m)?;
        enigo
            .key(mk, enigo::Direction::Release)
            .map_err(|e| anyhow!("falha ao soltar modificador {m}: {e}"))?;
    }

    std::thread::sleep(std::time::Duration::from_millis(200));
    let (b64, w, h) = capture_screen_base64(None)?;
    let mod_str = if modifiers.is_empty() {
        String::new()
    } else {
        format!("{}+", modifiers.join("+"))
    };
    Ok(ComputerOutcome {
        text: format!("Tecla {mod_str}{key_name} pressionada. Screenshot pos-acao ({w}x{h}px) anexado."),
        screenshot_base64: Some(b64),
    })
}

#[cfg(windows)]
fn exec_list_windows() -> Result<ComputerOutcome> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct WinInfo {
        pid: u32,
        title: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    }

    let mut results: Vec<WinInfo> = Vec::new();

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let results = &mut *(lparam.0 as *mut Vec<WinInfo>);
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                let mut buf = [0u16; 256];
                let len = GetWindowTextW(hwnd, &mut buf);
                if len > 0 {
                    let title = String::from_utf16_lossy(&buf[..len as usize]);
                    if title.trim().is_empty() {
                        return BOOL(1);
                    }
                    let mut pid: u32 = 0;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    let mut rect = RECT::default();
                    let _ = GetWindowRect(hwnd, &mut rect);
                    results.push(WinInfo {
                        pid,
                        title,
                        x: rect.left,
                        y: rect.top,
                        w: rect.right - rect.left,
                        h: rect.bottom - rect.top,
                    });
                }
            }
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(
            Some(callback),
            LPARAM(&mut results as *mut Vec<WinInfo> as isize),
        );
    }

    if results.is_empty() {
        return Ok(ComputerOutcome {
            text: "Nenhuma janela visivel encontrada.".to_string(),
            screenshot_base64: None,
        });
    }

    let lines: Vec<String> = results
        .iter()
        .map(|w| {
            format!(
                "pid={} title=\"{}\" rect=({},{},{}x{})",
                w.pid, w.title, w.x, w.y, w.w, w.h
            )
        })
        .collect();

    Ok(ComputerOutcome {
        text: format!(
            "{} janelas visiveis:\n{}",
            results.len(),
            lines.join("\n")
        ),
        screenshot_base64: None,
    })
}

#[cfg(not(windows))]
fn exec_list_windows() -> Result<ComputerOutcome> {
    Ok(ComputerOutcome {
        text: "list_windows ainda nao implementado nesta plataforma (so Windows por enquanto).".to_string(),
        screenshot_base64: None,
    })
}

fn exec_scroll(args: &Value) -> Result<ComputerOutcome> {
    rate_limit()?;
    let direction = args["direction"]
        .as_str()
        .ok_or_else(|| anyhow!("direction obrigatorio"))?;
    let amount = args["amount"].as_i64().unwrap_or(3) as i32;

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("falha ao inicializar enigo: {e}"))?;

    match direction {
        "up" => {
            enigo
                .scroll(amount, enigo::Axis::Vertical)
                .map_err(|e| anyhow!("falha ao rolar: {e}"))?;
        }
        "down" => {
            enigo
                .scroll(-amount, enigo::Axis::Vertical)
                .map_err(|e| anyhow!("falha ao rolar: {e}"))?;
        }
        "left" => {
            enigo
                .scroll(amount, enigo::Axis::Horizontal)
                .map_err(|e| anyhow!("falha ao rolar: {e}"))?;
        }
        "right" => {
            enigo
                .scroll(-amount, enigo::Axis::Horizontal)
                .map_err(|e| anyhow!("falha ao rolar: {e}"))?;
        }
        _ => return Err(anyhow!("direction invalida: {direction} (use up/down/left/right)")),
    }

    std::thread::sleep(std::time::Duration::from_millis(200));
    let (b64, w, h) = capture_screen_base64(None)?;
    Ok(ComputerOutcome {
        text: format!("Scroll {direction} ({amount}) executado. Screenshot pos-scroll ({w}x{h}px) anexado."),
        screenshot_base64: Some(b64),
    })
}
