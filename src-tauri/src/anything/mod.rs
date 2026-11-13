//! Modo Anything (ver `F:\modoAnythingCerneCode`). Este módulo é o ponto de entrada: os comandos
//! Tauri ficam aqui e a lógica fica nos submódulos.

pub mod cli;
pub mod doctor;
pub mod lote;
pub mod ghidra;
pub mod modulos;
pub mod plan;
pub mod prompts;
pub mod session;
pub mod varredor;
pub mod workspace;

/// Status de um item de função depois que a ficha passou na conferência do código.
pub const STATUS_PRONTO: &str = "pronto";
/// Status de um item que não passou na conferência depois das correções. A fila segue.
pub const STATUS_BLOQUEADA: &str = "bloqueada";

use crate::models::{AnythingState, Session};
use crate::{sessions, AppState};
use tauri::State;

/// Liga ou desliga o modo Anything numa sessão. Ligar exige pasta de projeto e um diagnóstico
/// obrigatório `ok`; a mensagem de erro diz o que falta. Desligar encerra o servidor da sessão.
#[tauri::command]
pub async fn set_session_anything(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
    target: Option<String>,
    goal: Option<String>,
) -> Result<Session, String> {
    let app_data = state.app_data_dir.clone();
    let session = sessions::get_session(&app_data, &id).map_err(|e| e.to_string())?;
    if enabled {
        let root = session::raiz_de_trabalho(&session).ok_or(session::SEM_PASTA)?;
        // O motor vem dentro do Cerne: se ainda não foi instalado na pasta gerenciada, instala agora, sem rede.
        let store = cli::store_dir(&app_data);
        if cli::find_cli(&store).is_none() && cli::bundled_zip(&app).is_some() {
            cli::install(&app, &store).await.map_err(|e| e.to_string())?;
        }
        let report = doctor::run(&store).await;
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

/// Troca o prompt da entrega final só desta sessão. Texto vazio (ou `None`) volta a seguir as Configurações.
#[tauri::command]
pub fn set_session_anything_final_prompt(state: State<AppState>, id: String, prompt: Option<String>) -> Result<Session, String> {
    let session = sessions::get_session(&state.app_data_dir, &id).map_err(|e| e.to_string())?;
    let prompt = prompt.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    let estado = AnythingState { final_prompt: prompt, ..session.anything.clone() };
    sessions::update_anything(&state.app_data_dir, &id, estado).map_err(|e| e.to_string())
}

/// Texto padrão da entrega final, para o botão "Redefinir" e para mostrar quando nada foi escrito.
#[tauri::command]
pub fn anything_final_prompt_default() -> String {
    prompts::ENTREGA.trim().to_string()
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

/// Triagem, seleção, ordem e gravação do `tarefas.json` da sessão. `modo` é `completo` ou `objetivo`.
/// Usa o alvo e o objetivo da sessão e as opções efetivas (Deep ou clean room).
#[tauri::command]
pub async fn anything_plan(state: State<'_, AppState>, id: String, modo: String, teto: usize) -> Result<plan::Resumo, String> {
    let app_data = state.app_data_dir.clone();
    let session = sessions::get_session(&app_data, &id).map_err(|e| e.to_string())?;
    if !session.anything.enabled {
        return Err("ligue o modo Anything nesta sessão antes de planejar".into());
    }
    let alvo = session.anything.target.clone().ok_or("escolha o alvo (arquivo) antes de planejar")?;
    let objetivo = session.anything.goal.clone().unwrap_or_default();
    let raiz = std::path::PathBuf::from(session::raiz_de_trabalho(&session).ok_or(session::SEM_PASTA)?);
    if !std::path::Path::new(&alvo).is_file() {
        return Err(format!("o alvo '{alvo}' não é um arquivo; por enquanto só arquivos"));
    }
    let (exe, _) = cli::find_cli(&cli::store_dir(&app_data)).ok_or("o motor não está instalado")?;
    let global = state.config.lock().unwrap().anything.clone();
    let opts = session.anything.effective(&global);
    let flags = vec![if opts.deep { "--deep".to_string() } else { "--clean-room".to_string() }];
    let ws = workspace::Workspace::new(&raiz);
    let ctx = plan::Contexto { exe: &exe, projeto: session::project_dir(&raiz), alvo: &alvo, flags };
    plan::planejar(&ctx, &ws, &modo, teto, &objetivo).await.map_err(|e| e.to_string())
}

/// Varre a pasta do projeto da sessão, acha os executáveis e grava `.anything/alvos.json`.
#[tauri::command]
pub fn anything_varrer(state: State<AppState>, id: String) -> Result<varredor::Varredura, String> {
    let sessao = sessions::get_session(&state.app_data_dir, &id).map_err(|e| e.to_string())?;
    let raiz = std::path::PathBuf::from(session::raiz_de_trabalho(&sessao).ok_or(session::SEM_PASTA)?);
    varredor::varrer_fontes(&raiz).map_err(|e| e.to_string())
}

fn raiz_da_sessao(state: &State<AppState>, id: &str) -> Result<std::path::PathBuf, String> {
    let sessao = sessions::get_session(&state.app_data_dir, id).map_err(|e| e.to_string())?;
    Ok(std::path::PathBuf::from(session::raiz_de_trabalho(&sessao).ok_or(session::SEM_PASTA)?))
}

/// Pastas e arquivos que o usuário adicionou à varredura (`.anything/fontes.json`).
#[tauri::command]
pub fn anything_fontes(state: State<AppState>, id: String) -> Result<Vec<varredor::Fonte>, String> {
    varredor::ler_fontes(&raiz_da_sessao(&state, &id)?).map_err(|e| e.to_string())
}

/// Adiciona uma pasta ou um arquivo à lista de fontes. Não varre: a varredura é um passo à parte.
#[tauri::command]
pub fn anything_fonte_adicionar(state: State<AppState>, id: String, caminho: String) -> Result<Vec<varredor::Fonte>, String> {
    varredor::adicionar_fonte(&raiz_da_sessao(&state, &id)?, &caminho).map_err(|e| e.to_string())
}

/// Tira uma fonte da lista, junto com os alvos que só ela cobria. O estudo já feito fica no disco.
#[tauri::command]
pub fn anything_fonte_remover(state: State<AppState>, id: String, caminho: String) -> Result<Vec<varredor::Fonte>, String> {
    varredor::remover_fonte(&raiz_da_sessao(&state, &id)?, &caminho).map_err(|e| e.to_string())
}

/// Estudo em lote: planeja cada alvo nativo de `alvos.json` que está `fazer`, cada um na própria pasta
/// em `.anything\alvos\`. Um erro num alvo não para os outros. `modo` é `completo` ou `objetivo`.
#[tauri::command]
pub async fn anything_planejar_lote(state: State<'_, AppState>, id: String, modo: String, teto: usize) -> Result<Vec<lote::Resultado>, String> {
    let app_data = state.app_data_dir.clone();
    let sessao = sessions::get_session(&app_data, &id).map_err(|e| e.to_string())?;
    if !sessao.anything.enabled {
        return Err("ligue o modo Anything nesta sessão antes de planejar".into());
    }
    let raiz = std::path::PathBuf::from(session::raiz_de_trabalho(&sessao).ok_or(session::SEM_PASTA)?);
    let objetivo = sessao.anything.goal.clone().unwrap_or_default();
    let (exe, _) = cli::find_cli(&cli::store_dir(&app_data)).ok_or("o motor não está instalado")?;
    let global = state.config.lock().unwrap().anything.clone();
    let opts = sessao.anything.effective(&global);
    let flags = vec![if opts.deep { "--deep".to_string() } else { "--clean-room".to_string() }];
    lote::planejar_lote(&exe, &raiz, &objetivo, &modo, teto, flags).await.map_err(|e| e.to_string())
}

/// Lista de alvos já salva em `.anything/alvos.json` da sessão (vazia se ainda não varreu).
#[tauri::command]
pub fn anything_alvos(state: State<AppState>, id: String) -> Result<Vec<varredor::Alvo>, String> {
    let sessao = sessions::get_session(&state.app_data_dir, &id).map_err(|e| e.to_string())?;
    let raiz = std::path::PathBuf::from(session::raiz_de_trabalho(&sessao).ok_or(session::SEM_PASTA)?);
    varredor::ler(&raiz).map_err(|e| e.to_string())
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
