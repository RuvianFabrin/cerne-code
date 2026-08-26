//! AX-tree sem visao no Linux (Tarefa 3.3 de PLANOS/port_linux_macos.md),
//! via AT-SPI2 - o equivalente de acessibilidade do UI Automation do
//! Windows pra apps GTK/Qt (Electron/Chrome tem suporte parcial, igual
//! documentado no plano). Complementa `computer_wayland` (que cobre
//! click/type/scroll "as cegas" via portal): a AX-tree deixa o agente
//! localizar e clicar elementos por nome/papel, sem precisar de screenshot.
//!
//! Fluxo: conecta no barramento de acessibilidade -> pega os filhos do
//! registry (cada um e uma aplicacao rodando) -> resolve o PID de cada
//! aplicacao via `org.freedesktop.DBus.GetConnectionUnixProcessID` no nome
//! unico do barramento dela (mesma tecnica usada por leitores de tela como
//! o Orca) -> acha a aplicacao com o PID pedido -> percorre a arvore de
//! filhos dela coletando elementos com papel "acionavel". `click_element`
//! reusa a mesma travessia e invoca a acao padrao (indice 0, tipicamente
//! "click"/"press"/"activate") via a interface Action - mais confiavel que
//! coordenada de pixel porque nao depende do elemento estar visivel na
//! tela nem de calculo de clickable point.

use anyhow::{anyhow, Result};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::action::ActionProxy;
use atspi::AccessibilityConnection;
use atspi::Role;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

const MAX_ELEMENTS: usize = 200;
const MAX_DEPTH: u32 = 12;

fn is_actionable_role(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Entry
            | Role::CheckBox
            | Role::ComboBox
            | Role::Link
            | Role::ListItem
            | Role::MenuItem
            | Role::RadioButton
            | Role::PageTab
    )
}

struct FoundElement {
    name: String,
    role: Role,
    bus_name: String,
    path: ObjectPath<'static>,
}

async fn accessible_at(
    conn: &Connection,
    bus_name: &str,
    path: ObjectPath<'static>,
) -> Result<AccessibleProxy<'static>> {
    AccessibleProxy::builder(conn)
        .destination(bus_name.to_string())
        .map_err(|e| anyhow!("destino AT-SPI invalido ({bus_name}): {e}"))?
        .path(path)
        .map_err(|e| anyhow!("path AT-SPI invalido: {e}"))?
        .build()
        .await
        .map_err(|e| anyhow!("falha ao construir proxy AT-SPI: {e}"))
}

/// Acha a aplicacao (filha direta do registry AT-SPI) cujo processo tem o
/// PID pedido, resolvendo o PID de cada uma via D-Bus
/// (`GetConnectionUnixProcessID` no nome unico do barramento da app - AT-SPI
/// nao expoe PID como propriedade direta do objeto acessivel).
async fn find_application(conn: &Connection, pid: u32) -> Result<(String, ObjectPath<'static>)> {
    let dbus = zbus::fdo::DBusProxy::new(conn)
        .await
        .map_err(|e| anyhow!("falha ao conectar no D-Bus de sessao: {e}"))?;

    let a11y = AccessibilityConnection::new()
        .await
        .map_err(|e| anyhow!("falha ao conectar no barramento de acessibilidade (AT-SPI) - verifique se um servico de acessibilidade (orca/at-spi2-registryd) esta rodando: {e}"))?;

    let registry_root = a11y
        .root_accessible_on_registry()
        .await
        .map_err(|e| anyhow!("falha ao obter o registry AT-SPI: {e}"))?;

    let apps = registry_root
        .get_children()
        .await
        .map_err(|e| anyhow!("falha ao listar aplicacoes registradas no AT-SPI: {e}"))?;

    for app_ref in apps {
        let Some(bus_name) = app_ref.name_as_str().map(|s| s.to_string()) else {
            continue;
        };
        let Ok(unique_name) = zbus::names::UniqueName::try_from(bus_name.as_str()) else {
            continue;
        };
        let Ok(found_pid) = dbus
            .get_connection_unix_process_id(unique_name.into())
            .await
        else {
            continue;
        };
        if found_pid == pid {
            return Ok((bus_name, app_ref.path().to_owned()));
        }
    }

    Err(anyhow!(
        "nenhuma aplicacao com PID {pid} encontrada via AT-SPI - o processo pode nao \
         suportar acessibilidade, ou o AT-SPI pode estar desabilitado no sistema \
         (GTK/Qt tem suporte nativo; Electron/Chrome as vezes precisa de flag)"
    ))
}

/// Percorre a arvore de acessibilidade a partir de `(bus_name, path)`
/// (busca em profundidade iterativa - sem recursao async pra nao precisar
/// de dependencia extra so pra boxing de futures), coletando elementos com
/// papel acionavel ate `MAX_ELEMENTS` ou `MAX_DEPTH`.
async fn walk_actionable(
    conn: &Connection,
    root_bus: &str,
    root_path: ObjectPath<'static>,
) -> Vec<FoundElement> {
    let mut out = Vec::new();
    let mut stack: Vec<(String, ObjectPath<'static>, u32)> =
        vec![(root_bus.to_string(), root_path, 0)];

    while let Some((bus_name, path, depth)) = stack.pop() {
        if out.len() >= MAX_ELEMENTS || depth > MAX_DEPTH {
            continue;
        }
        let Ok(proxy) = accessible_at(conn, &bus_name, path.clone()).await else {
            continue;
        };
        let Ok(role) = proxy.get_role().await else {
            continue;
        };
        let name = proxy.name().await.unwrap_or_default();

        if is_actionable_role(role) {
            out.push(FoundElement {
                name,
                role,
                bus_name: bus_name.clone(),
                path: path.clone(),
            });
        }

        if out.len() >= MAX_ELEMENTS {
            continue;
        }

        if let Ok(children) = proxy.get_children().await {
            for child in children {
                if let Some(child_bus) = child.name_as_str().map(|s| s.to_string()) {
                    stack.push((child_bus, child.path().to_owned(), depth + 1));
                }
            }
        }
    }

    out
}

pub async fn get_window_state(pid: u32) -> Result<String> {
    let conn = Connection::session()
        .await
        .map_err(|e| anyhow!("falha ao conectar no D-Bus de sessao: {e}"))?;
    let (bus_name, path) = find_application(&conn, pid).await?;
    let elements = walk_actionable(&conn, &bus_name, path).await;

    if elements.is_empty() {
        return Ok(format!(
            "Nenhum elemento acionavel encontrado para PID {pid} via AT-SPI."
        ));
    }

    let lines: Vec<String> = elements
        .iter()
        .enumerate()
        .map(|(idx, el)| {
            if el.name.is_empty() {
                format!("[{idx}] {:?}", el.role)
            } else {
                format!("[{idx}] {:?} \"{}\"", el.role, el.name)
            }
        })
        .collect();

    Ok(format!(
        "AX tree (AT-SPI) para PID {pid} ({} elementos):\n{}",
        lines.len(),
        lines.join("\n")
    ))
}

pub async fn click_element(pid: u32, element_index: usize) -> Result<String> {
    let conn = Connection::session()
        .await
        .map_err(|e| anyhow!("falha ao conectar no D-Bus de sessao: {e}"))?;
    let (bus_name, path) = find_application(&conn, pid).await?;
    let elements = walk_actionable(&conn, &bus_name, path).await;

    let el = elements.get(element_index).ok_or_else(|| {
        anyhow!(
            "element_index {element_index} nao encontrado (total de elementos acionaveis: {})",
            elements.len()
        )
    })?;

    let action_proxy = ActionProxy::builder(&conn)
        .destination(el.bus_name.clone())
        .map_err(|e| anyhow!("destino AT-SPI invalido: {e}"))?
        .path(el.path.clone())
        .map_err(|e| anyhow!("path AT-SPI invalido: {e}"))?
        .build()
        .await
        .map_err(|e| anyhow!("elemento [{element_index}] nao suporta a interface Action: {e}"))?;

    let n_actions = action_proxy
        .n_actions()
        .await
        .map_err(|e| anyhow!("falha ao consultar acoes do elemento [{element_index}]: {e}"))?;
    if n_actions <= 0 {
        return Err(anyhow!(
            "elemento [{element_index}] nao tem nenhuma acao invocavel"
        ));
    }

    let ok = action_proxy
        .do_action(0)
        .await
        .map_err(|e| anyhow!("falha ao invocar acao no elemento [{element_index}]: {e}"))?;
    if !ok {
        return Err(anyhow!(
            "acao no elemento [{element_index}] retornou falha (o proprio AT-SPI recusou)"
        ));
    }

    Ok(format!(
        "Elemento [{element_index}] ({:?} \"{}\") acionado via AT-SPI (acao 0).",
        el.role, el.name
    ))
}
