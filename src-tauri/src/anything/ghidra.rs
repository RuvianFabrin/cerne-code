//! Modo Anything, fase 02: Ghidra e o Java dele. O próprio `craft-anything-cli` faz o download (com
//! SHA-256 e o Java 21 junto); o Cerne só chama `engines-install`, espera com um limite de tempo e lê
//! `engines-list` para saber o que ficou instalado. Nada aqui baixa por conta própria.

use super::cli;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Tempo máximo da instalação (download de ~740 MB mais preparação). Passou disso, o processo é morto.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// O que a interface mostra sobre Ghidra e Java.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GhidraState {
    pub ghidra_found: bool,
    pub ghidra_path: Option<String>,
    pub java_found: bool,
    pub java_path: Option<String>,
}

/// Procura `"id": "<id>"` em `engines[]` e devolve o campo `found` (caminho ou `null`).
fn found_of(list: &Value, id: &str) -> Option<String> {
    list.get("result")?
        .get("engines")?
        .as_array()?
        .iter()
        .find(|e| e.get("id").and_then(Value::as_str) == Some(id))?
        .get("found")?
        .as_str()
        .map(str::to_string)
}

/// Lê `engines-list` do motor instalado. Não instala nada.
pub fn read_state_from(list: &Value) -> GhidraState {
    let ghidra_path = found_of(list, "ghidra");
    let java_path = found_of(list, "java");
    GhidraState {
        ghidra_found: ghidra_path.is_some(),
        ghidra_path,
        java_found: java_path.is_some(),
        java_path,
    }
}

/// Roda `craft-anything-cli engines-list` e devolve o JSON. Falha se o motor não existe ou não responde.
pub async fn engines_list(exe: &Path) -> Result<Value> {
    let out = run_cli(exe, &["engines-list"], Duration::from_secs(60)).await?;
    serde_json::from_slice(&out.stdout).context("o motor respondeu algo que não é JSON em engines-list")
}

/// Estado atual, sem instalar. `None` = o motor em si não está instalado (instale a fase 01 antes).
pub async fn state(store: &Path) -> Result<Option<GhidraState>> {
    let Some((exe, _)) = cli::find_cli(store) else {
        return Ok(None);
    };
    let list = engines_list(&exe).await?;
    Ok(Some(read_state_from(&list)))
}

/// Instala o Ghidra (com o Java 21 dele) pelo motor. Só roda quando o usuário confirma na interface.
/// Emite `anything:progress` com etapa `Instalando Ghidra`; o motor não expõe bytes, então não há
/// barra de bytes nesta etapa.
pub async fn install(app: &AppHandle, store: &Path) -> Result<GhidraState> {
    let (exe, _) = cli::find_cli(store).ok_or_else(|| anyhow!("o motor ainda não está instalado; faça a instalação do motor primeiro"))?;
    let _ = app.emit(cli::PROGRESS_EVENT, serde_json::json!({ "stage": "Instalando Ghidra e Java (pode demorar)", "received": 0, "total": null }));
    let out = run_cli(&exe, &["engines-install", "--engine", "ghidra"], INSTALL_TIMEOUT).await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let tail = |s: &str| s.chars().rev().take(800).collect::<String>().chars().rev().collect::<String>();
        bail!("o motor não conseguiu instalar o Ghidra: {} {}", tail(stderr.trim()), tail(stdout.trim()));
    }
    let state = state(store).await?.ok_or_else(|| anyhow!("o motor sumiu depois da instalação"))?;
    if !state.ghidra_found {
        bail!("a instalação terminou, mas o Ghidra não aparece em engines-list");
    }
    let _ = app.emit(cli::PROGRESS_EVENT, serde_json::json!({ "stage": "Ghidra pronto", "received": 0, "total": null }));
    Ok(state)
}

/// Roda o motor sem janela, sem entrada (`stdin` nulo: se pedir resposta, falha em vez de travar),
/// com limite de tempo. O processo morre se o limite passar ou se o future for cancelado.
pub(super) async fn run_cli(exe: &Path, args: &[&str], limit: Duration) -> Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().with_context(|| format!("não consegui iniciar {}", exe.display()))?;
    match tokio::time::timeout(limit, child.wait_with_output()).await {
        Ok(out) => Ok(out?),
        Err(_) => bail!("o motor passou de {} minutos sem terminar", limit.as_secs() / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_ghidra_and_java_from_engines_list() {
        let list = json!({"result": {"engines": [
            {"id": "ghidra", "found": null},
            {"id": "java", "found": "C:\\jdk\\bin\\java.exe"},
        ]}});
        let s = read_state_from(&list);
        assert!(!s.ghidra_found && s.ghidra_path.is_none());
        assert!(s.java_found);
        assert_eq!(s.java_path.as_deref(), Some("C:\\jdk\\bin\\java.exe"));
    }

    #[test]
    fn a_list_without_the_engines_says_nothing_is_found() {
        let s = read_state_from(&json!({"result": {}}));
        assert_eq!(s, GhidraState { ghidra_found: false, ghidra_path: None, java_found: false, java_path: None });
    }
}
