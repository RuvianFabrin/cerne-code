//! Ferramentas que mostram coisas no chat ou consultam o catalogo de skills.
//!
//! Ficam fora do `mod.rs` porque precisam de `AppHandle` (pra emitir eventos
//! pro chat) e o sub-agente (`subagent.rs`) executa suas ferramentas por outro
//! caminho (`tools::execute_tool`). Assim a mesma logica serve aos dois.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

use super::tools::ToolOutcome;

/// Nomes das ferramentas tratadas aqui.
pub const CHAT_TOOL_NAMES: &[&str] = &["report_findings", "show_widget", "send_user_file", "search_skills"];

/// Limite do HTML/SVG de um widget (200 KB).
const WIDGET_MAX_BYTES: usize = 200_000;
/// Quantidade maxima de achados por cartao.
const FINDINGS_MAX: usize = 32;

/// Executa uma das ferramentas de `CHAT_TOOL_NAMES`. `project_path` e a pasta
/// do projeto da sessao (pra resolver caminho relativo). Outro nome e erro.
pub fn run(
    name: &str,
    app: &AppHandle,
    session_id: &str,
    project_path: Option<&Path>,
    app_data_dir: &Path,
    args: &Value,
) -> Result<ToolOutcome> {
    match name {
        "report_findings" => report_findings(app, session_id, args),
        "show_widget" => show_widget(app, session_id, args),
        "send_user_file" => send_user_file(app, session_id, project_path, args),
        "search_skills" => search_skills(app_data_dir, project_path, args),
        other => Err(anyhow!("ferramenta de chat desconhecida: {other}")),
    }
}

fn outcome(observation: impl Into<String>) -> ToolOutcome {
    ToolOutcome {
        observation: observation.into(),
        pending_edit: None,
    }
}

/// Achados de revisao viram um cartao no chat (`agent:chat_card`, kind "findings").
pub fn report_findings(app: &AppHandle, session_id: &str, args: &Value) -> Result<ToolOutcome> {
    let items = args["findings"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        return Err(anyhow!("findings obrigatorio (array com pelo menos 1 achado)"));
    }
    let findings: Vec<Value> = items
        .iter()
        .take(FINDINGS_MAX)
        .map(|f| {
            json!({
                "file": f["file"].clone(),
                "line": f["line"].clone(),
                "severity": f["severity"].clone(),
                "category": f["category"].clone(),
                "summary": f["summary"].clone(),
                "failure_scenario": f["failure_scenario"].clone(),
            })
        })
        .collect();
    let n = findings.len();
    let _ = app.emit(
        "agent:chat_card",
        json!({ "session_id": session_id, "kind": "findings", "findings": findings }),
    );
    Ok(outcome(format!("{n} achado(s) exibido(s) no chat como cartoes.")))
}

/// Widget HTML/SVG inline (`agent:chat_card`, kind "widget"). O frontend o
/// coloca num iframe sem rede.
pub fn show_widget(app: &AppHandle, session_id: &str, args: &Value) -> Result<ToolOutcome> {
    let title = args["title"].as_str().unwrap_or("Widget").to_string();
    match args["widget_code"].as_str() {
        Some(code) if !code.trim().is_empty() && code.len() <= WIDGET_MAX_BYTES => {
            let _ = app.emit(
                "agent:chat_card",
                json!({ "session_id": session_id, "kind": "widget", "title": title, "code": code }),
            );
            Ok(outcome("Widget exibido no chat."))
        }
        Some(_) => Err(anyhow!("widget_code vazio ou maior que 200 KB")),
        None => Err(anyhow!("widget_code obrigatorio")),
    }
}

/// Anexa um arquivo existente ao chat (cartao com abrir/revelar). So vale pra
/// caminhos que essa funcao registrou (`SENT_USER_FILES`), no comando `open_user_file`.
pub fn send_user_file(
    app: &AppHandle,
    session_id: &str,
    project_path: Option<&Path>,
    args: &Value,
) -> Result<ToolOutcome> {
    let raw = args["path"]
        .as_str()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| anyhow!("path obrigatorio"))?;
    let candidate: PathBuf = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        project_path.map(|root| root.join(raw)).unwrap_or_else(|| PathBuf::from(raw))
    };
    let meta = std::fs::metadata(&candidate)
        .ok()
        .filter(|m| m.is_file())
        .ok_or_else(|| anyhow!("arquivo nao encontrado: {raw}"))?;
    let name = candidate
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let caption = args["caption"].as_str().unwrap_or("").to_string();
    let shown_path = candidate.display().to_string();
    if let Ok(mut sent) = super::SENT_USER_FILES.lock() {
        sent.push(shown_path.clone());
    }
    let _ = app.emit(
        "agent:user_file",
        json!({
            "session_id": session_id,
            "name": name,
            "path": shown_path,
            "size": meta.len(),
            "caption": caption,
        }),
    );
    Ok(outcome(format!(
        "Arquivo anexado ao chat: {name} ({} bytes). O usuario ve um cartao para abrir ou revelar o arquivo.",
        meta.len()
    )))
}

/// Busca skills do catalogo por palavras-chave.
pub fn search_skills(app_data_dir: &Path, project_path: Option<&Path>, args: &Value) -> Result<ToolOutcome> {
    let query = args["query"].as_str().ok_or_else(|| anyhow!("query obrigatorio"))?;
    let found = crate::skills::search_skills(app_data_dir, project_path, query)?;
    let observation = if found.is_empty() {
        "nenhuma skill encontrada para essa busca".to_string()
    } else {
        found
            .iter()
            .take(10)
            .map(|s| format!("- {} ({}): {}", s.name, s.scope, s.description))
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(outcome(observation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_tool_names_are_the_four_handled_here() {
        assert_eq!(CHAT_TOOL_NAMES.len(), 4);
        assert!(CHAT_TOOL_NAMES.contains(&"send_user_file"));
    }

    #[test]
    fn search_skills_requires_query() {
        let dir = std::env::temp_dir().join(format!("cerne-chat-tools-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(search_skills(&dir, None, &json!({})).is_err());
        let ok = search_skills(&dir, None, &json!({"query": "qualquer"})).unwrap();
        assert!(ok.observation.contains("nenhuma skill"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
