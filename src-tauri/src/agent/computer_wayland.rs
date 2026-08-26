//! Backend de input do computer_use pra sessoes Wayland (Tarefa 3.1b de
//! PLANOS/port_linux_macos.md). O enigo injeta input direto no display
//! server e o Wayland bloqueia isso por design (isolamento entre apps) -
//! a rota oficial e o portal xdg-desktop `RemoteDesktop`, que pede
//! consentimento explicito do usuario via dialogo nativo do compositor
//! (GNOME/KDE) na primeira chamada.
//!
//! `notify_pointer_motion_absolute` exige um `stream` (node id do PipeWire)
//! como referencia de coordenadas - por isso a sessao tambem negocia um
//! `Screencast` do monitor primario (mesmo escopo do
//! `computer_use_screenshot` via xcap), mesmo sem consumir o video em si.
//!
//! Escopo desta primeira versao: sessao + click/type/key/scroll ponta a
//! ponta. NAO implementado ainda (pendencias documentadas no plano):
//! - Calibracao de 5 pontos pra HiDPI/multi-monitor com escala fracionaria
//!   (a conversao pixel-do-screenshot -> coordenada do portal aqui e 1:1;
//!   sem calibracao, cliques podem desalinhar em telas com scaling).
//! - Cancelamento via GlobalShortcuts/hot-corner - o unico watchdog e o
//!   timeout de inatividade abaixo. Como o portal injeta input SINTETICO
//!   (nao captura o teclado/mouse fisico do usuario - ver nota do plano),
//!   o usuario sempre pode retomar o controle por vias normais do SO
//!   (Alt+Tab, fechar o app, etc.) mesmo sem essas camadas extras.

use anyhow::{anyhow, Result};
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::desktop::{PersistMode, Session};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OnceCell};

const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

// Codigos de botao evdev/BTN_* (linux/input-event-codes.h) - convencao
// exigida pelo protocolo do portal RemoteDesktop pra notify_pointer_button.
const BTN_LEFT: i32 = 0x110;
const BTN_RIGHT: i32 = 0x111;
const BTN_MIDDLE: i32 = 0x112;

struct WaylandSession {
    remote_desktop: RemoteDesktop,
    session: Session<RemoteDesktop>,
    stream_node_id: u32,
    last_used: Instant,
}

static SESSION_CELL: OnceCell<Mutex<Option<WaylandSession>>> = OnceCell::const_new();

async fn session_mutex() -> &'static Mutex<Option<WaylandSession>> {
    SESSION_CELL
        .get_or_init(|| async { Mutex::new(None) })
        .await
}

/// Detecta sessao Wayland. Mesma heuristica que o xcap usa internamente
/// (`WAYLAND_DISPLAY`/`XDG_SESSION_TYPE`), replicada aqui porque o xcap nao
/// expoe essa checagem na API publica.
pub fn is_wayland_session() -> bool {
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase();
    let wayland_display = std::env::var("WAYLAND_DISPLAY")
        .unwrap_or_default()
        .to_lowercase();
    xdg_session_type == "wayland" || wayland_display.contains("wayland")
}

/// Garante uma sessao RemoteDesktop+Screencast ativa, criando (e disparando
/// o dialogo de consentimento) se necessario. Sessoes ociosas por mais de
/// `IDLE_TIMEOUT` sao fechadas e recriadas na proxima chamada.
async fn ensure_session() -> Result<()> {
    let mutex = session_mutex().await;
    let mut guard = mutex.lock().await;

    if let Some(sess) = guard.as_mut() {
        if sess.last_used.elapsed() < IDLE_TIMEOUT {
            sess.last_used = Instant::now();
            return Ok(());
        }
        let _ = sess.session.close().await;
        *guard = None;
    }

    let remote_desktop = RemoteDesktop::new()
        .await
        .map_err(|e| anyhow!("falha ao conectar no portal RemoteDesktop: {e}"))?;
    let screencast = Screencast::new()
        .await
        .map_err(|e| anyhow!("falha ao conectar no portal Screencast: {e}"))?;
    let session = remote_desktop
        .create_session(Default::default())
        .await
        .map_err(|e| anyhow!("falha ao criar sessao no portal: {e}"))?;

    remote_desktop
        .select_devices(
            &session,
            SelectDevicesOptions::default()
                .set_devices(DeviceType::Keyboard | DeviceType::Pointer),
        )
        .await
        .map_err(|e| anyhow!("falha ao pedir teclado/mouse ao portal: {e}"))?
        .response()
        .map_err(|e| anyhow!("usuario recusou compartilhar teclado/mouse: {e}"))?;

    screencast
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Metadata)
                .set_sources(Some(SourceType::Monitor.into()))
                .set_multiple(false)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await
        .map_err(|e| anyhow!("falha ao pedir monitor ao portal: {e}"))?
        .response()
        .map_err(|e| anyhow!("usuario recusou compartilhar o monitor: {e}"))?;

    let response = remote_desktop
        .start(&session, None, Default::default())
        .await
        .map_err(|e| anyhow!("falha ao iniciar sessao do portal: {e}"))?
        .response()
        .map_err(|e| anyhow!("usuario recusou o dialogo de consentimento do portal: {e}"))?;

    let stream_node_id = response
        .streams()
        .first()
        .ok_or_else(|| anyhow!("portal nao retornou nenhum stream de monitor"))?
        .pipe_wire_node_id();

    *guard = Some(WaylandSession {
        remote_desktop,
        session,
        stream_node_id,
        last_used: Instant::now(),
    });
    Ok(())
}

pub async fn click(x: i32, y: i32, button: &str) -> Result<()> {
    ensure_session().await?;
    let mutex = session_mutex().await;
    let guard = mutex.lock().await;
    let sess = guard
        .as_ref()
        .ok_or_else(|| anyhow!("sessao Wayland nao inicializada"))?;

    let btn = match button {
        "right" => BTN_RIGHT,
        "middle" => BTN_MIDDLE,
        _ => BTN_LEFT,
    };

    sess.remote_desktop
        .notify_pointer_motion_absolute(
            &sess.session,
            sess.stream_node_id,
            x as f64,
            y as f64,
            Default::default(),
        )
        .await
        .map_err(|e| anyhow!("falha ao mover cursor via portal: {e}"))?;
    sess.remote_desktop
        .notify_pointer_button(&sess.session, btn, KeyState::Pressed, Default::default())
        .await
        .map_err(|e| anyhow!("falha ao pressionar botao via portal: {e}"))?;
    sess.remote_desktop
        .notify_pointer_button(&sess.session, btn, KeyState::Released, Default::default())
        .await
        .map_err(|e| anyhow!("falha ao soltar botao via portal: {e}"))?;
    Ok(())
}

/// Keysym X11 pro caracter. ASCII imprimivel (0x20-0x7E) mapeia direto pro
/// codepoint - convencao historica do X11 (keysyms Latin1 == Unicode nessa
/// faixa). Fora do ASCII, usa a convencao XKB de keysym Unicode
/// (0x01000000 + codepoint), suportada pelo protocolo desde XKB 2.0 e pelos
/// compositores Wayland principais (mutter/kwin).
fn char_to_keysym(ch: char) -> i32 {
    let cp = ch as u32;
    if (0x20..=0x7e).contains(&cp) {
        cp as i32
    } else {
        (0x0100_0000 + cp) as i32
    }
}

pub async fn type_text(text: &str) -> Result<()> {
    ensure_session().await?;
    let mutex = session_mutex().await;
    let guard = mutex.lock().await;
    let sess = guard
        .as_ref()
        .ok_or_else(|| anyhow!("sessao Wayland nao inicializada"))?;

    for ch in text.chars() {
        let keysym = char_to_keysym(ch);
        sess.remote_desktop
            .notify_keyboard_keysym(&sess.session, keysym, KeyState::Pressed, Default::default())
            .await
            .map_err(|e| anyhow!("falha ao digitar (tecla pressionada) via portal: {e}"))?;
        sess.remote_desktop
            .notify_keyboard_keysym(&sess.session, keysym, KeyState::Released, Default::default())
            .await
            .map_err(|e| anyhow!("falha ao digitar (tecla solta) via portal: {e}"))?;
    }
    Ok(())
}

/// Vocabulario de teclas nomeadas identico ao aceito pela tool
/// `computer_use_press_key` no Windows/X11 - valores de keysymdef.h,
/// estaveis desde os anos 1990.
fn named_key_to_keysym(key: &str) -> Option<i32> {
    let lower = key.to_lowercase();
    let named = match lower.as_str() {
        "return" | "enter" => 0xff0d,
        "tab" => 0xff09,
        "escape" | "esc" => 0xff1b,
        "backspace" => 0xff08,
        "delete" | "del" => 0xffff,
        "home" => 0xff50,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "pageup" => 0xff55,
        "pagedown" => 0xff56,
        "end" => 0xff57,
        "space" => 0x0020,
        _ => {
            if let Some(n) = lower.strip_prefix('f') {
                if let Ok(num) = n.parse::<u32>() {
                    if (1..=12).contains(&num) {
                        return Some((0xffbe + (num - 1)) as i32);
                    }
                }
            }
            let mut chars = lower.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) => Some(char_to_keysym(c)),
                _ => None,
            };
        }
    };
    Some(named)
}

fn modifier_keysym(modifier: &str) -> Option<i32> {
    Some(match modifier.to_lowercase().as_str() {
        "ctrl" | "control" => 0xffe3,
        "shift" => 0xffe1,
        "alt" => 0xffe9,
        "win" | "super" | "meta" => 0xffeb,
        _ => return None,
    })
}

pub async fn press_key(key: &str, modifiers: &[String]) -> Result<()> {
    ensure_session().await?;
    let mutex = session_mutex().await;
    let guard = mutex.lock().await;
    let sess = guard
        .as_ref()
        .ok_or_else(|| anyhow!("sessao Wayland nao inicializada"))?;

    let keysym =
        named_key_to_keysym(key).ok_or_else(|| anyhow!("tecla '{key}' nao reconhecida"))?;
    let mod_keysyms: Vec<i32> = modifiers
        .iter()
        .filter_map(|m| modifier_keysym(m))
        .collect();

    for m in &mod_keysyms {
        sess.remote_desktop
            .notify_keyboard_keysym(&sess.session, *m, KeyState::Pressed, Default::default())
            .await
            .map_err(|e| anyhow!("falha ao pressionar modificador via portal: {e}"))?;
    }
    sess.remote_desktop
        .notify_keyboard_keysym(&sess.session, keysym, KeyState::Pressed, Default::default())
        .await
        .map_err(|e| anyhow!("falha ao pressionar tecla via portal: {e}"))?;
    sess.remote_desktop
        .notify_keyboard_keysym(&sess.session, keysym, KeyState::Released, Default::default())
        .await
        .map_err(|e| anyhow!("falha ao soltar tecla via portal: {e}"))?;
    for m in mod_keysyms.iter().rev() {
        sess.remote_desktop
            .notify_keyboard_keysym(&sess.session, *m, KeyState::Released, Default::default())
            .await
            .map_err(|e| anyhow!("falha ao soltar modificador via portal: {e}"))?;
    }
    Ok(())
}

pub async fn scroll(direction: &str, amount: i32) -> Result<()> {
    ensure_session().await?;
    let mutex = session_mutex().await;
    let guard = mutex.lock().await;
    let sess = guard
        .as_ref()
        .ok_or_else(|| anyhow!("sessao Wayland nao inicializada"))?;

    let step = 15.0; // "linha" aproximada, mesma ordem de grandeza do enigo::Axis
    let (dx, dy) = match direction {
        "up" => (0.0, -step * amount as f64),
        "down" => (0.0, step * amount as f64),
        "left" => (-step * amount as f64, 0.0),
        "right" => (step * amount as f64, 0.0),
        other => return Err(anyhow!("direcao de scroll invalida: {other}")),
    };

    sess.remote_desktop
        .notify_pointer_axis(&sess.session, dx, dy, Default::default())
        .await
        .map_err(|e| anyhow!("falha ao rolar via portal: {e}"))?;
    Ok(())
}
