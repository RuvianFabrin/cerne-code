//! Modo Anything (ver `F:\modoAnythingCerneCode`). Este módulo é o ponto de entrada: os comandos
//! Tauri ficam aqui e a lógica fica nos submódulos.

pub mod cli;
pub mod doctor;
pub mod ghidra;
pub mod session;

use crate::models::{AnythingState, Session};
use crate::{sessions, AppState};
use tauri::State;

/// Liga ou desliga o modo Anything numa sessão. Ligar exige pasta de projeto e um diagnóstico
/// obrigatório `ok`; a mensagem de erro diz o que falta. Desligar encerra o servidor da sessão.
#[tauri::command]
pub async fn set_session_anything(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
    target: Option<String>,
    goal: Option<String>,
) -> Result<Session, String> {
    let app_data = state.app_data_dir.clone();
    let session = sessions::get_session(&app_data, &id).map_err(|e| e.to_string())?;
    if enabled {
        let root = session.project_root.clone().ok_or("escolha uma pasta de projeto para a sessão antes de ligar o modo Anything")?;
        let report = doctor::run(&cli::store_dir(&app_data)).await;
        if !report.pronto {
            let faltam: Vec<String> = report
                .itens
                .iter()
                .filter(|i| i.obrigatorio && i.status != doctor::Status::Ok)
                .map(|i| format!("{}: {}", i.id, i.detalhe))
                .collect();
            return Err(format!("faltam itens para o modo Anything: {}", faltam.join("; ")));
        }
        std::fs::create_dir_all(session::workspace_dir(std::path::Path::new(&root))).map_err(|e| e.to_string())?;
    } else {
        state.mcp_clients.disconnect(&session::server_name(&id)).await;
    }
    let clean = |s: Option<String>| s.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    // As opções de sessão (full_tools, deep, allow_run) continuam as mesmas: só liga/desliga e alvo mudam.
    let estado = AnythingState { enabled, target: clean(target), goal: clean(goal), ..session.anything.clone() };
    sessions::update_anything(&app_data, &id, estado).map_err(|e| e.to_string())
}

/// Troca as opções desta sessão. `None` devolve a opção ao padrão global. Com o modo ligado, o
/// servidor da sessão é encerrado para subir de novo com os novos flags.
#[tauri::command]
pub async fn set_session_anything_options(
    state: State<'_, AppState>,
    id: String,
    full_tools: Option<bool>,
    deep: Option<bool>,
    allow_run: Option<bool>,
) -> Result<Session, String> {
    let app_data = state.app_data_dir.clone();
    let session = sessions::get_session(&app_data, &id).map_err(|e| e.to_string())?;
    let estado = AnythingState { full_tools, deep, allow_run, ..session.anything.clone() };
    if session.anything.enabled {
        state.mcp_clients.disconnect(&session::server_name(&id)).await;
    }
    sessions::update_anything(&app_data, &id, estado).map_err(|e| e.to_string())
}

/// Ghidra e Java nesta máquina, pelo `engines-list` do motor. Não instala nada.
#[tauri::command]
pub async fn anything_ghidra_status(state: State<'_, AppState>) -> Result<Option<ghidra::GhidraState>, String> {
    ghidra::state(&cli::store_dir(&state.app_data_dir)).await.map_err(|e| e.to_string())
}

/// Diagnóstico do modo: motor, Ghidra, Java, disco, servidor MCP, Deep e o `doctor` do motor.
/// Não instala nada; limite total de dois minutos.
#[tauri::command]
pub async fn anything_doctor(state: State<'_, AppState>) -> Result<doctor::Report, String> {
    let store = cli::store_dir(&state.app_data_dir);
    tokio::time::timeout(std::time::Duration::from_secs(120), doctor::run(&store))
        .await
        .map_err(|_| "o diagnóstico passou de 2 minutos".to_string())
}

/// Instala Ghidra e Java pelo motor. Só roda quando o usuário confirma.
#[tauri::command]
pub async fn anything_ghidra_install(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<ghidra::GhidraState, String> {
    ghidra::install(&app, &cli::store_dir(&state.app_data_dir)).await.map_err(|e| e.to_string())
}

/// Estado do motor `craft-anything-cli` nesta máquina. Não baixa nada.
#[tauri::command]
pub fn anything_cli_status(state: State<AppState>) -> Result<cli::CliStatus, String> {
    Ok(cli::status(&cli::store_dir(&state.app_data_dir)))
}

/// Baixa e instala o motor na versão fixada. Só roda quando o usuário clica; emite
/// `anything:progress` durante o download.
#[tauri::command]
pub async fn anything_cli_install(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<cli::CliStatus, String> {
    let store = cli::store_dir(&state.app_data_dir);
    cli::install(&app, &store).await.map_err(|e| e.to_string())?;
    Ok(cli::status(&store))
}
