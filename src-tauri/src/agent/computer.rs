use crate::models::{ToolFunctionSpec, ToolSpec};
use anyhow::{anyhow, Result};
use enigo::{Enigo, Key, Keyboard, Mouse, Settings};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

static LAST_ACTION: Mutex<Option<Instant>> = Mutex::new(None);
static AUTHORIZED_APPS: Mutex<Option<Vec<String>>> = Mutex::new(None);
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

fn auth_file_path(app_data_dir: &Path) -> std::path::PathBuf {
    app_data_dir.join("computer_permissions.json")
}

fn load_authorized(app_data_dir: &Path) -> Vec<String> {
    let mut cache = AUTHORIZED_APPS.lock().unwrap();
    if let Some(ref list) = *cache {
        return list.clone();
    }
    let path = auth_file_path(app_data_dir);
    let list: Vec<String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    *cache = Some(list.clone());
    list
}

fn save_authorized(app_data_dir: &Path, list: &[String]) -> Result<()> {
    let path = auth_file_path(app_data_dir);
    std::fs::write(&path, serde_json::to_string_pretty(list)?)?;
    let mut cache = AUTHORIZED_APPS.lock().unwrap();
    *cache = Some(list.to_vec());
    Ok(())
}

pub fn authorize_app(app_data_dir: &Path, exe_name: &str) -> Result<()> {
    let mut list = load_authorized(app_data_dir);
    let lower = exe_name.to_lowercase();
    if !list.iter().any(|a| a.to_lowercase() == lower) {
        list.push(lower);
        save_authorized(app_data_dir, &list)?;
    }
    Ok(())
}

pub fn is_authorized(app_data_dir: &Path, exe_name: &str) -> bool {
    let list = load_authorized(app_data_dir);
    let lower = exe_name.to_lowercase();
    list.iter().any(|a| a.to_lowercase() == lower)
}

#[cfg(windows)]
fn get_foreground_exe_name() -> Result<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_WIN32};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == HWND::default() {
            return Err(anyhow!("nao foi possivel obter janela em primeiro plano"));
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return Err(anyhow!("nao foi possivel obter PID da janela em primeiro plano"));
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|e| anyhow!("falha ao abrir processo {pid}: {e}"))?;
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let pwstr = windows::core::PWSTR(buf.as_mut_ptr());
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, pwstr, &mut size);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if ok.is_err() {
            return Err(anyhow!("falha ao obter caminho do executavel"));
        }
        let full_path = String::from_utf16_lossy(&buf[..size as usize]);
        let exe_name = full_path
            .split(['\\', '/'])
            .last()
            .unwrap_or(&full_path)
            .to_string();
        Ok(exe_name)
    }
}

#[cfg(not(windows))]
fn get_foreground_exe_name() -> Result<String> {
    Err(anyhow!("get_foreground_exe_name nao implementado nesta plataforma"))
}

/// Acha a primeira janela visivel cujo titulo contem `title_substr`
/// (case-insensitive) - mesmo criterio de match usado por
/// `capture_screen_base64` pra screenshot de janela especifica.
#[cfg(windows)]
fn find_window_by_title(title_substr: &str) -> Result<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible};

    struct FindCtx {
        needle: String,
        found: Option<HWND>,
    }
    let mut ctx = FindCtx {
        needle: title_substr.to_lowercase(),
        found: None,
    };

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lparam.0 as *mut FindCtx) };
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                let mut buf = [0u16; 256];
                let len = GetWindowTextW(hwnd, &mut buf);
                if len > 0 {
                    let title = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
                    if title.contains(&ctx.needle) {
                        ctx.found = Some(hwnd);
                        return BOOL(0); // achou - para a enumeracao
                    }
                }
            }
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(callback), LPARAM(&mut ctx as *mut FindCtx as isize));
    }
    ctx.found
        .ok_or_else(|| anyhow!("janela com titulo contendo '{title_substr}' nao encontrada"))
}

/// Restaura (se minimizada) e traz `hwnd` pro primeiro plano de verdade.
///
/// O Windows tem uma protecao ("foreground lock") que ignora
/// `SetForegroundWindow` vindo de um processo que nao foi o ultimo a
/// receber input do usuario - sem isso, computer_use so conseguia
/// clicar/digitar em cima da janela que ja estava em primeiro plano por
/// acaso. A tecnica padrao (mesma usada por AutoHotkey/pywinauto) e
/// anexar temporariamente a fila de mensagens da nossa thread a da
/// janela em primeiro plano atual e a da janela alvo via
/// `AttachThreadInput` - isso concede a permissao de trocar o foco,
/// que e revogada de novo assim que desanexamos.
#[cfg(windows)]
fn focus_window(hwnd: windows::Win32::Foundation::HWND) -> Result<()> {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
        SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let current_thread = GetCurrentThreadId();

        let attach_fg = foreground_thread != 0 && foreground_thread != current_thread;
        let attach_target =
            target_thread != 0 && target_thread != current_thread && target_thread != foreground_thread;

        if attach_fg {
            let _ = AttachThreadInput(current_thread, foreground_thread, true);
        }
        if attach_target {
            let _ = AttachThreadInput(current_thread, target_thread, true);
        }

        let _ = BringWindowToTop(hwnd);
        let result = SetForegroundWindow(hwnd);

        if attach_target {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }
        if attach_fg {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }

        if !result.as_bool() {
            return Err(anyhow!(
                "SetForegroundWindow falhou - o Windows pode ter recusado a troca de foco"
            ));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn exec_focus_window(_args: &Value) -> Result<ComputerOutcome> {
    Ok(ComputerOutcome {
        text: "focus_window so disponivel no Windows.".to_string(),
        screenshot_base64: None,
    })
}

/// Foca a janela indicada em `args["window_title"]` (se presente e não
/// vazio) ANTES de qualquer outra coisa - usado por click/type_text/
/// press_key/scroll pra deixar focar+agir numa aplicação em segundo plano
/// numa chamada só, sem depender do modelo lembrar de chamar
/// `computer_use_focus_window` separadamente antes. Precisa rodar ANTES de
/// `check_authorization`, senão a autorização seria checada contra a
/// janela antiga (normalmente o próprio Cerne Code, já que é nele que o
/// usuário estava digitando o pedido).
#[cfg(windows)]
pub fn maybe_focus_from_args(args: &Value) -> Result<()> {
    if let Some(title) = args["window_title"].as_str() {
        if !title.trim().is_empty() {
            let hwnd = find_window_by_title(title)?;
            focus_window(hwnd)?;
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn maybe_focus_from_args(_args: &Value) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn exec_focus_window(args: &Value) -> Result<ComputerOutcome> {
    rate_limit()?;
    let title = args["window_title"]
        .as_str()
        .ok_or_else(|| anyhow!("window_title obrigatorio"))?;
    let hwnd = find_window_by_title(title)?;
    focus_window(hwnd)?;
    std::thread::sleep(std::time::Duration::from_millis(150));
    Ok(ComputerOutcome {
        text: format!(
            "Janela contendo '{title}' trazida para primeiro plano. Use computer_use_screenshot pra ver o estado atual antes de clicar/digitar."
        ),
        screenshot_base64: None,
    })
}

fn check_authorization(app_data_dir: &Path) -> Result<()> {
    let exe = get_foreground_exe_name()?;
    if is_authorized(app_data_dir, &exe) {
        return Ok(());
    }
    Err(anyhow!(
        "APLICACAO NAO AUTORIZADA: o processo em primeiro plano e '{exe}'. \
         Use computer_use_authorize para autorizar esta aplicacao antes de interagir com ela. \
         Antes de autorizar, use ask para confirmar com o usuario: 'Posso interagir com {exe}?'"
    ))
}

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "computer_use_screenshot",
            "Captura screenshot do MONITOR PRIMARIO apenas. Retorna a imagem + metadata (resolucao, offset, total de monitores). SEMPRE use esta tool ANTES de qualquer click/type para ver o estado atual. Se a aplicacao alvo nao estiver visivel no monitor primario, peça ao usuario para move-la. REQUER modelo com visao.",
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
            "Clica em coordenadas do MONITOR PRIMARIO (pixels relativos ao screenshot). SEMPRE chame computer_use_screenshot ANTES. As coordenadas sao relativas ao canto superior esquerdo do monitor primario (0,0). O resultado inclui screenshot pos-clique. REQUER modelo com visao.",
            json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer", "description": "Coordenada X em pixels de tela" },
                    "y": { "type": "integer", "description": "Coordenada Y em pixels de tela" },
                    "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Botao do mouse (default: left)" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES do clique - use sempre que a aplicacao alvo nao for a que o usuario estava usando (ex: o Cerne Code) no momento do pedido, senao o clique cai na janela errada." }
                },
                "required": ["x", "y"]
            }),
        ),
        spec(
            "computer_use_type_text",
            "Digita texto via teclado no elemento focado. Foque o campo desejado ANTES, com computer_use_click (precisa de visao) ou computer_use_click_element (via AX-tree, nao precisa de visao). Max 500 chars por chamada. Nao requer visao (desde que o foco tenha sido feito por outra via).",
            json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Texto a digitar (max 500 chars)" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES de digitar - use sempre que a aplicacao alvo nao for a que o usuario estava usando (ex: o Cerne Code) no momento do pedido." }
                },
                "required": ["text"]
            }),
        ),
        spec(
            "computer_use_press_key",
            "Pressiona uma tecla ou combinacao (ex: ctrl+c, alt+tab, enter). Combinacoes destrutivas (alt+f4, ctrl+shift+esc) sao bloqueadas. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Tecla: return, tab, escape, up, down, left, right, space, delete, home, end, pageup, pagedown, f1-f12, ou letra/digito" },
                    "modifiers": { "type": "array", "items": { "type": "string" }, "description": "Modificadores: ctrl, shift, alt, win" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES de pressionar a tecla." }
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
            "computer_use_focus_window",
            "Traz uma janela (pelo titulo parcial, igual computer_use_list_windows mostra) para primeiro plano, restaurando-a se estiver minimizada. Use ISSO antes de click/type/scroll sempre que a aplicacao alvo nao for a que ja esta em primeiro plano - sem focar primeiro, o clique/digitacao vai pra janela errada (a que o usuario estiver usando no momento). Depois de focar, chame computer_use_screenshot pra confirmar o estado antes de interagir. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {
                    "window_title": { "type": "string", "description": "Titulo parcial da janela (case-insensitive), ex: 'Notepad' ou 'Cerne Code'" }
                },
                "required": ["window_title"]
            }),
        ),
        spec(
            "computer_use_scroll",
            "Rola a tela ou janela focada. Use computer_use_click ou computer_use_focus_window ANTES para garantir que a janela certa esta focada. Nao requer visao.",
            json!({
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["up", "down", "left", "right"] },
                    "amount": { "type": "integer", "description": "Quantidade de linhas/scrolls (default: 3)" },
                    "window_title": { "type": "string", "description": "Titulo parcial da janela alvo (opcional). Se informado, ela e trazida pro primeiro plano ANTES de rolar." }
                },
                "required": ["direction"]
            }),
        ),
        spec(
            "computer_use_authorize",
            "Autoriza o computer_use a interagir com uma aplicacao (pelo nome do executavel, ex: chrome.exe). Use ANTES de click/type/key/scroll. Sempre confirme com o usuario via ask antes de autorizar.",
            json!({
                "type": "object",
                "properties": {
                    "exe_name": { "type": "string", "description": "Nome do executavel (ex: chrome.exe, code.exe, notepad.exe)" }
                },
                "required": ["exe_name"]
            }),
        ),
        spec(
            "computer_use_browser_execute",
            "Interage com paginas web via CDP (Chrome DevTools Protocol). Funciona com Chrome/Edge/Brave que tenham --remote-debugging-port ativado. Acoes: execute_javascript, click_element (CSS), get_text, query_dom. Nao requer visao nem autorizacao de janela.",
            json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["execute_javascript", "click_element", "get_text", "query_dom"] },
                    "javascript": { "type": "string", "description": "JS a executar (para execute_javascript)" },
                    "css_selector": { "type": "string", "description": "Seletor CSS (para click_element / query_dom)" },
                    "port": { "type": "integer", "description": "Porta CDP (default: 9222)" }
                },
                "required": ["action"]
            }),
        ),
        spec(
            "computer_use_get_window_state",
            "Le a arvore de acessibilidade (UI Automation) de uma janela. Retorna elementos interativos com [element_index N] para usar em computer_use_click_element. Mais confiavel que coordenadas pixel. So Windows.",
            json!({
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "PID do processo" }
                },
                "required": ["pid"]
            }),
        ),
        spec(
            "computer_use_click_element",
            "Clica em um elemento da arvore de acessibilidade pelo element_index (obtido via computer_use_get_window_state). Mais confiavel que coordenadas pixel. So Windows.",
            json!({
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "PID do processo" },
                    "element_index": { "type": "integer", "description": "Indice do elemento (de get_window_state)" }
                },
                "required": ["pid", "element_index"]
            }),
        ),
    ]
}

/// Ferramentas que exigem TER VISTO a tela em pixel pra funcionar - screenshot
/// obviamente, e click porque as coordenadas x,y vem de olhar o screenshot.
/// Todo o resto continua disponivel sem visao:
/// list_windows/focus_window/authorize/browser_execute/AX-tree (get_window_state,
/// click_element) sao baseadas em texto/estrutura; type_text/press_key/scroll
/// so agem sobre o elemento que ja estiver focado (via computer_use_click_element,
/// por exemplo) e nao precisam saber onde nada esta na tela - um modelo sem
/// visao consegue automatizar 100% via arvore de acessibilidade: list_windows
/// -> focus_window -> get_window_state -> click_element -> type_text/press_key.
pub const VISION_REQUIRED_TOOLS: &[&str] = &["computer_use_screenshot", "computer_use_click"];

pub fn requires_vision(tool_name: &str) -> bool {
    VISION_REQUIRED_TOOLS.contains(&tool_name)
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

pub async fn execute(name: &str, args: &Value, app_data_dir: &Path) -> Result<ComputerOutcome> {
    match name {
        "computer_use_screenshot" => exec_screenshot(args),
        "computer_use_list_windows" => exec_list_windows(),
        "computer_use_focus_window" => exec_focus_window(args),
        "computer_use_authorize" => exec_authorize(args, app_data_dir),
        "computer_use_click" => {
            maybe_focus_from_args(args)?;
            check_authorization(app_data_dir)?;
            exec_click(args)
        }
        "computer_use_type_text" => {
            maybe_focus_from_args(args)?;
            check_authorization(app_data_dir)?;
            exec_type_text(args)
        }
        "computer_use_press_key" => {
            maybe_focus_from_args(args)?;
            check_authorization(app_data_dir)?;
            exec_press_key(args)
        }
        "computer_use_scroll" => {
            maybe_focus_from_args(args)?;
            check_authorization(app_data_dir)?;
            exec_scroll(args)
        }
        "computer_use_browser_execute" => exec_browser(args).await,
        "computer_use_get_window_state" => exec_ax_tree(args),
        "computer_use_click_element" => exec_click_element(args),
        _ => Err(anyhow!("computer_use tool desconhecida: {name}")),
    }
}

fn rgba_to_base64(img: image::RgbaImage) -> Result<String> {
    use base64::Engine;
    let mut dyn_img = image::DynamicImage::ImageRgba8(img);
    if dyn_img.width() > 1280 {
        let ratio = 1280.0 / dyn_img.width() as f32;
        let new_h = (dyn_img.height() as f32 * ratio) as u32;
        dyn_img = dyn_img.resize(1280, new_h, image::imageops::FilterType::Triangle);
    }
    let rgb = dyn_img.to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
    encoder
        .encode(&rgb, rgb.width(), rgb.height(), image::ColorType::Rgb8.into())
        .map_err(|e| anyhow!("falha ao codificar JPEG: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

fn capture_screen_base64(window_title: Option<&str>) -> Result<(String, u32, u32, String)> {
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| anyhow!("falha ao listar monitores: {e}"))?;
    let total = monitors.len();
    let monitor = monitors
        .first()
        .ok_or_else(|| anyhow!("nenhum monitor encontrado"))?;

    let mon_x = monitor.x().unwrap_or(0);
    let mon_y = monitor.y().unwrap_or(0);
    let mon_w = monitor.width().unwrap_or(0);
    let mon_h = monitor.height().unwrap_or(0);
    let mon_name = monitor.name().unwrap_or_default();

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
    let meta = format!(
        "monitor_primario=\"{mon_name}\" res={mon_w}x{mon_h} offset_virtual=({mon_x},{mon_y}) total_monitores={total} screenshot={w}x{h}px"
    );
    Ok((b64, w, h, meta))
}

fn exec_authorize(args: &Value, app_data_dir: &Path) -> Result<ComputerOutcome> {
    let exe_name = args["exe_name"]
        .as_str()
        .ok_or_else(|| anyhow!("exe_name obrigatorio"))?;
    authorize_app(app_data_dir, exe_name)?;
    Ok(ComputerOutcome {
        text: format!("Aplicacao '{exe_name}' autorizada para computer_use. Voce agora pode usar click, type, key e scroll quando esta aplicacao estiver em primeiro plano."),
        screenshot_base64: None,
    })
}

fn exec_screenshot(args: &Value) -> Result<ComputerOutcome> {
    let window_title = args["window_title"].as_str();
    let (b64, w, h, meta) = capture_screen_base64(window_title)?;
    Ok(ComputerOutcome {
        text: format!("Screenshot do MONITOR PRIMARIO capturado ({w}x{h}px). {meta}. IMPORTANTE: so o monitor primario e visivel. Se a aplicacao alvo nao estiver neste monitor, peça ao usuario para move-la. Analise a imagem para decidir a proxima acao."),
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

    let (b64, w, h, meta) = capture_screen_base64(None)?;
    Ok(ComputerOutcome {
        text: format!("Clique {button_str} em ({x},{y}) executado no monitor primario. {meta}. Screenshot pos-clique ({w}x{h}px) anexado — verifique se o efeito foi o esperado."),
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
    let (b64, w, h, _meta) = capture_screen_base64(None)?;
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
    let (b64, w, h, _meta) = capture_screen_base64(None)?;
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
    let (b64, w, h, _meta) = capture_screen_base64(None)?;
    Ok(ComputerOutcome {
        text: format!("Scroll {direction} ({amount}) executado. Screenshot pos-scroll ({w}x{h}px) anexado."),
        screenshot_base64: Some(b64),
    })
}

// ── CDP Browser ──────────────────────────────────────────────────────

async fn cdp_rpc(ws_url: &str, method: &str, params: Value) -> Result<Value> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;

    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| anyhow!("falha ao conectar CDP WebSocket: {e}"))?;
    let (mut write, mut read) = ws_stream.split();

    let id = rand_id();
    let msg = json!({ "id": id, "method": method, "params": params });
    write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            msg.to_string().into(),
        ))
        .await
        .map_err(|e| anyhow!("falha ao enviar CDP: {e}"))?;

    while let Some(Ok(frame)) = read.next().await {
        if let tokio_tungstenite::tungstenite::Message::Text(text) = frame {
            let resp: Value = serde_json::from_str(&text).unwrap_or_default();
            if resp["id"].as_u64() == Some(id) {
                if let Some(err) = resp.get("error") {
                    return Err(anyhow!("CDP error: {err}"));
                }
                return Ok(resp["result"].clone());
            }
        }
    }
    Err(anyhow!("CDP WebSocket fechado sem resposta"))
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
}

async fn exec_browser(args: &Value) -> Result<ComputerOutcome> {
    let action = args["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action obrigatorio"))?;
    let port = args["port"].as_u64().unwrap_or(9222);

    let targets_url = format!("http://127.0.0.1:{port}/json");
    let resp = reqwest::get(&targets_url)
        .await
        .map_err(|e| anyhow!("falha ao conectar CDP em porta {port}: {e}. O browser esta rodando com --remote-debugging-port={port}?"))?;
    let targets: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta CDP invalida: {e}"))?;
    let ws_url = targets
        .as_array()
        .and_then(|arr| arr.iter().find(|t| t["type"] == "page"))
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .ok_or_else(|| anyhow!("nenhum alvo CDP (page) encontrado"))?
        .to_string();

    let result = match action {
        "execute_javascript" => {
            let js = args["javascript"]
                .as_str()
                .ok_or_else(|| anyhow!("javascript obrigatorio para execute_javascript"))?;
            let r = cdp_rpc(
                &ws_url,
                "Runtime.evaluate",
                json!({ "expression": js, "returnByValue": true, "awaitPromise": true }),
            )
            .await?;
            if let Some(exc) = r.get("exceptionDetails") {
                format!("EXCEPTION: {exc}")
            } else {
                serde_json::to_string_pretty(&r["result"]["value"]).unwrap_or_default()
            }
        }
        "get_text" => {
            let r = cdp_rpc(
                &ws_url,
                "Runtime.evaluate",
                json!({ "expression": "document.body.innerText", "returnByValue": true }),
            )
            .await?;
            r["result"]["value"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(8000)
                .collect()
        }
        "query_dom" => {
            let sel = args["css_selector"]
                .as_str()
                .ok_or_else(|| anyhow!("css_selector obrigatorio para query_dom"))?;
            let js = format!(
                "JSON.stringify(Array.from(document.querySelectorAll('{sel}')).slice(0,50).map(e => ({{tag: e.tagName, id: e.id, class: e.className, text: (e.textContent||'').slice(0,100)}})))"
            );
            let r = cdp_rpc(
                &ws_url,
                "Runtime.evaluate",
                json!({ "expression": js, "returnByValue": true }),
            )
            .await?;
            r["result"]["value"]
                .as_str()
                .unwrap_or("[]")
                .to_string()
        }
        "click_element" => {
            let sel = args["css_selector"]
                .as_str()
                .ok_or_else(|| anyhow!("css_selector obrigatorio para click_element"))?;
            let js = format!("document.querySelector('{sel}')?.click(); 'clicked'");
            let r = cdp_rpc(
                &ws_url,
                "Runtime.evaluate",
                json!({ "expression": js, "returnByValue": true }),
            )
            .await?;
            format!(
                "click em '{sel}': {}",
                r["result"]["value"].as_str().unwrap_or("erro")
            )
        }
        _ => return Err(anyhow!("action desconhecida: {action}")),
    };

    Ok(ComputerOutcome {
        text: format!("CDP {action} (porta {port}): {result}"),
        screenshot_base64: None,
    })
}

// ── AX Tree (UI Automation) ─────────────────────────────────────────

#[cfg(windows)]
fn exec_ax_tree(args: &Value) -> Result<ComputerOutcome> {
    use uiautomation::UIAutomation;

    let pid = args["pid"]
        .as_u64()
        .ok_or_else(|| anyhow!("pid obrigatorio"))? as u32;

    let automation =
        UIAutomation::new().map_err(|e| anyhow!("falha ao inicializar UI Automation: {e}"))?;
    let root = automation
        .get_root_element()
        .map_err(|e| anyhow!("falha ao obter root element: {e}"))?;

    let condition = automation
        .create_property_condition(
            uiautomation::types::UIProperty::ProcessId,
            uiautomation::variants::Variant::from(pid as i32),
            None,
        )
        .map_err(|e| anyhow!("falha ao criar condicao: {e}"))?;

    let elements = root
        .find_all(uiautomation::types::TreeScope::Descendants, &condition)
        .unwrap_or_default();

    let mut lines: Vec<String> = Vec::new();
    let mut idx = 0;
    for el in elements.iter().take(200) {
        let name = el.get_name().unwrap_or_default();
        let ctrl = el
            .get_control_type()
            .map(|c| format!("{:?}", c))
            .unwrap_or_default();
        let is_actionable = matches!(
            el.get_control_type(),
            Ok(uiautomation::types::ControlType::Button)
                | Ok(uiautomation::types::ControlType::Edit)
                | Ok(uiautomation::types::ControlType::CheckBox)
                | Ok(uiautomation::types::ControlType::ComboBox)
                | Ok(uiautomation::types::ControlType::Hyperlink)
                | Ok(uiautomation::types::ControlType::ListItem)
                | Ok(uiautomation::types::ControlType::MenuItem)
                | Ok(uiautomation::types::ControlType::TabItem)
                | Ok(uiautomation::types::ControlType::TreeItem)
        );
        if is_actionable || !name.is_empty() {
            let label = if name.is_empty() {
                ctrl.clone()
            } else {
                format!("{ctrl} \"{name}\"")
            };
            if is_actionable {
                lines.push(format!("[{idx}] {label}"));
            } else {
                lines.push(format!("    {label}"));
            }
            idx += 1;
        }
    }

    if lines.is_empty() {
        return Ok(ComputerOutcome {
            text: format!("Nenhum elemento encontrado para PID {pid}."),
            screenshot_base64: None,
        });
    }

    Ok(ComputerOutcome {
        text: format!(
            "AX tree para PID {pid} ({} elementos):\n{}",
            lines.len(),
            lines.join("\n")
        ),
        screenshot_base64: None,
    })
}

#[cfg(not(windows))]
fn exec_ax_tree(_args: &Value) -> Result<ComputerOutcome> {
    Ok(ComputerOutcome {
        text: "AX tree so disponivel no Windows.".to_string(),
        screenshot_base64: None,
    })
}

#[cfg(windows)]
fn exec_click_element(args: &Value) -> Result<ComputerOutcome> {
    use uiautomation::UIAutomation;

    let pid = args["pid"]
        .as_u64()
        .ok_or_else(|| anyhow!("pid obrigatorio"))? as u32;
    let element_index = args["element_index"]
        .as_u64()
        .ok_or_else(|| anyhow!("element_index obrigatorio"))? as usize;

    let automation =
        UIAutomation::new().map_err(|e| anyhow!("falha ao inicializar UI Automation: {e}"))?;
    let root = automation
        .get_root_element()
        .map_err(|e| anyhow!("falha ao obter root element: {e}"))?;

    let condition = automation
        .create_property_condition(
            uiautomation::types::UIProperty::ProcessId,
            uiautomation::variants::Variant::from(pid as i32),
            None,
        )
        .map_err(|e| anyhow!("falha ao criar condicao: {e}"))?;

    let elements = root
        .find_all(uiautomation::types::TreeScope::Descendants, &condition)
        .unwrap_or_default();

    let mut actionable_idx = 0;
    for el in elements.iter() {
        let name = el.get_name().unwrap_or_default();
        let is_actionable = matches!(
            el.get_control_type(),
            Ok(uiautomation::types::ControlType::Button)
                | Ok(uiautomation::types::ControlType::Edit)
                | Ok(uiautomation::types::ControlType::CheckBox)
                | Ok(uiautomation::types::ControlType::ComboBox)
                | Ok(uiautomation::types::ControlType::Hyperlink)
                | Ok(uiautomation::types::ControlType::ListItem)
                | Ok(uiautomation::types::ControlType::MenuItem)
                | Ok(uiautomation::types::ControlType::TabItem)
                | Ok(uiautomation::types::ControlType::TreeItem)
        );
        if is_actionable || !name.is_empty() {
            if is_actionable && actionable_idx == element_index {
                if let Ok(Some(point)) = el.get_clickable_point() {
                    let cx = point.get_x();
                    let cy = point.get_y();
                    let mut enigo = Enigo::new(&Settings::default())
                        .map_err(|e| anyhow!("falha ao inicializar enigo: {e}"))?;
                    enigo
                        .move_mouse(cx, cy, enigo::Coordinate::Abs)
                        .map_err(|e| anyhow!("falha ao mover mouse: {e}"))?;
                    enigo
                        .button(enigo::Button::Left, enigo::Direction::Click)
                        .map_err(|e| anyhow!("falha ao clicar: {e}"))?;
                    return Ok(ComputerOutcome {
                        text: format!("Elemento [{element_index}] clicado em ({cx},{cy}) via AX clickable point."),
                        screenshot_base64: None,
                    });
                }
                return Err(anyhow!(
                    "elemento [{element_index}] nao tem clickable point"
                ));
            }
            if is_actionable {
                actionable_idx += 1;
            }
            actionable_idx += if !is_actionable { 0 } else { 0 };
        }
    }

    Err(anyhow!(
        "element_index {element_index} nao encontrado (total de elementos actionaveis: {actionable_idx})"
    ))
}

#[cfg(not(windows))]
fn exec_click_element(_args: &Value) -> Result<ComputerOutcome> {
    Ok(ComputerOutcome {
        text: "click_element so disponivel no Windows.".to_string(),
        screenshot_base64: None,
    })
}
