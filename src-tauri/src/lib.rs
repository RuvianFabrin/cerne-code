mod agent;
mod attachments;
mod config;
mod context;
mod encoding;
mod mcp;
mod models;
mod providers;
mod sandbox;
mod search;
mod sessions;
mod skills;

use models::{
    AppConfig, ChatMessage, ExecutionMode, ModelInfo, PendingEdit, ProviderKind, ReasoningEffort,
    Session, TaskItem,
};
use providers::llama_cpp::LlamaForkConfig;
use skills::SkillMeta;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use tokio::process::Child;

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub config: Mutex<AppConfig>,
    pub pending_edits: Mutex<HashMap<String, PendingEdit>>,
    pub llama_children: Mutex<HashMap<String, Child>>,
    pub background_jobs: agent::background::BackgroundJobs,
    pub mcp_clients: mcp::McpClients,
    /// Perguntas (`ask`) que pausaram um turno esperando resposta do usuario —
    /// a task async do `run_turn` fica literalmente parada num `.await` no
    /// lado receptor do canal ate `answer_ask` mandar a resposta, sem precisar
    /// serializar/retomar estado (o loop do agente continua vivo, so
    /// suspenso).
    pub pending_questions: Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>,
    /// Pedidos de permissao (modo "Manual" de execucao) esperando o usuario
    /// aprovar/recusar uma tool call especifica — mesmo padrao de canal
    /// oneshot que `pending_questions` usa pro `ask`.
    pub pending_permissions: Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    /// Handle da task async de cada turno em andamento, por sessao — permite
    /// `cancel_turn` abortar um turno inteiro no modo "Auto" (o usuario nao
    /// precisa esperar o proximo checkpoint cooperativo, o abort da tokio
    /// task derruba a chamada HTTP em andamento imediatamente).
    pub running_turns: Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn set_config(state: State<AppState>, new_config: AppConfig) -> Result<(), String> {
    config::save_config(&state.app_data_dir, &new_config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = new_config;
    Ok(())
}

#[tauri::command]
fn set_openrouter_key(key: String) -> Result<(), String> {
    config::set_openrouter_key(&key).map_err(|e| e.to_string())
}

#[tauri::command]
fn has_openrouter_key() -> bool {
    config::has_openrouter_key()
}

/// Monta a config de conexão pra `kind` — pra `Custom`, busca o provider
/// configurado pelo usuário (id/label/base_url em `custom_providers.json`,
/// chave no keyring do SO) em vez de ler campos fixos do `AppConfig`, já que
/// não há como saber de antemão quais providers customizados existem.
pub(crate) fn build_provider_config(
    kind: ProviderKind,
    cfg: &AppConfig,
    app_data_dir: &PathBuf,
    custom_provider_id: Option<&str>,
) -> Result<(models::ProviderConfig, Option<String>), String> {
    if kind == ProviderKind::Custom {
        let id = custom_provider_id
            .ok_or_else(|| "custom_provider_id obrigatorio pro provider customizado".to_string())?;
        let providers =
            providers::custom::load_providers(app_data_dir).map_err(|e| e.to_string())?;
        let provider = providers
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("provider customizado desconhecido: {id}"))?;
        let api_key = providers::custom::get_key(id);
        return Ok((
            models::ProviderConfig {
                kind,
                base_url: provider.base_url,
                has_api_key: api_key.is_some(),
                llama_fork: None,
                supports_vision_override: provider.supports_vision,
                context_length_override: provider.context_length,
            },
            api_key,
        ));
    }

    let base_url = match kind {
        ProviderKind::Openrouter => cfg.openrouter_base_url.clone(),
        ProviderKind::LlamaCpp => cfg.llama_cpp_base_url.clone(),
        ProviderKind::Ollama => cfg.ollama_base_url.clone(),
        ProviderKind::LmStudio => cfg.lmstudio_base_url.clone(),
        ProviderKind::Custom => unreachable!(),
    };
    let api_key = if matches!(kind, ProviderKind::Openrouter) {
        config::get_openrouter_key()
    } else {
        None
    };
    Ok((
        models::ProviderConfig {
            kind,
            base_url,
            has_api_key: api_key.is_some(),
            llama_fork: Some(cfg.active_llama_fork.clone()),
            supports_vision_override: false,
            context_length_override: None,
        },
        api_key,
    ))
}

#[tauri::command]
async fn list_provider_models(
    state: State<'_, AppState>,
    kind: ProviderKind,
    custom_provider_id: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let cfg = state.config.lock().unwrap().clone();
    let (provider_cfg, api_key) = build_provider_config(
        kind,
        &cfg,
        &state.app_data_dir,
        custom_provider_id.as_deref(),
    )?;
    providers::list_models(&provider_cfg, api_key, &state.app_data_dir)
        .await
        .map_err(|e| e.to_string())
}

/// Best-effort context-window lookup for a given provider+model, used when
/// creating a session so the context-usage indicator has a real number.
#[tauri::command]
async fn resolve_context_length(
    state: State<'_, AppState>,
    kind: ProviderKind,
    model: String,
    fork_id: Option<String>,
    custom_provider_id: Option<String>,
) -> Result<Option<u32>, String> {
    let cfg = state.config.lock().unwrap().clone();
    if kind == ProviderKind::LlamaCpp {
        let fork_id = fork_id.unwrap_or(cfg.active_llama_fork);
        let forks =
            providers::llama_cpp::load_forks(&state.app_data_dir).map_err(|e| e.to_string())?;
        let fork = forks
            .into_iter()
            .find(|f| f.id == fork_id)
            .ok_or_else(|| format!("fork desconhecido: {fork_id}"))?;
        return Ok(providers::llama_cpp::preset_context_length(
            &fork.models_ini,
            &model,
        ));
    }
    let (provider_cfg, api_key) = build_provider_config(
        kind,
        &cfg,
        &state.app_data_dir,
        custom_provider_id.as_deref(),
    )?;
    Ok(providers::get_context_length(&provider_cfg, api_key, &model, &state.app_data_dir).await)
}

#[tauri::command]
fn list_llama_forks(state: State<AppState>) -> Result<Vec<LlamaForkConfig>, String> {
    providers::llama_cpp::load_forks(&state.app_data_dir).map_err(|e| e.to_string())
}

/// Adiciona (ou atualiza, se o `id` já existir) um fork llama.cpp configurado
/// pelo usuário — nunca assume um layout de pastas específico de máquina,
/// já que o Cerne é distribuído open source.
#[tauri::command]
fn add_llama_fork(
    state: State<AppState>,
    fork: LlamaForkConfig,
) -> Result<Vec<LlamaForkConfig>, String> {
    providers::llama_cpp::add_fork(&state.app_data_dir, fork).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_llama_fork(state: State<AppState>, id: String) -> Result<Vec<LlamaForkConfig>, String> {
    providers::llama_cpp::remove_fork(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

/// Provider "customizado": qualquer endpoint compatível com a API de chat
/// completions da OpenAI que o usuário configura (Claude via seu shim,
/// Grok/xAI, ChatGPT/OpenAI, Qwen/DashScope, Kimi/Moonshot, ou qualquer
/// outro) — ver `providers::custom`.
#[tauri::command]
fn list_custom_providers(
    state: State<AppState>,
) -> Result<Vec<providers::custom::CustomProviderConfig>, String> {
    providers::custom::load_providers(&state.app_data_dir).map_err(|e| e.to_string())
}

/// Testa um endpoint customizado ANTES de salvar (`base_url`/`api_key`
/// direto dos campos do formulário, sem tocar em `custom_providers.json` nem
/// no keyring) — chama `/models` de verdade e devolve os ids encontrados.
/// Mesma ideia do `test_mcp_server`: confirmar que a conexão funciona antes
/// de persistir qualquer coisa.
#[tauri::command]
async fn test_custom_provider(
    state: State<'_, AppState>,
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let cfg = models::ProviderConfig {
        kind: ProviderKind::Custom,
        base_url,
        has_api_key: api_key.is_some(),
        llama_fork: None,
        supports_vision_override: false,
        context_length_override: None,
    };
    providers::list_models(&cfg, api_key, &state.app_data_dir)
        .await
        .map_err(|e| e.to_string())
}

/// Adiciona (ou atualiza, se o `id` já existir) um provider customizado.
/// `api_key`, se enviado, substitui a chave salva no keyring do SO — enviar
/// `None` mantém a chave atual (permite editar label/URL sem reenviar a
/// chave toda vez, já que a UI não guarda o valor bruto depois de salvo).
#[tauri::command]
fn add_custom_provider(
    state: State<AppState>,
    provider: providers::custom::CustomProviderConfig,
    api_key: Option<String>,
) -> Result<Vec<providers::custom::CustomProviderConfig>, String> {
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        providers::custom::set_key(&provider.id, &key).map_err(|e| e.to_string())?;
    }
    providers::custom::add_provider(&state.app_data_dir, provider).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_custom_provider(
    state: State<AppState>,
    id: String,
) -> Result<Vec<providers::custom::CustomProviderConfig>, String> {
    providers::custom::remove_provider(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn has_custom_provider_key(id: String) -> bool {
    providers::custom::has_key(&id)
}

#[tauri::command]
fn list_llama_presets(state: State<AppState>, fork_id: String) -> Result<Vec<ModelInfo>, String> {
    let forks = providers::llama_cpp::load_forks(&state.app_data_dir).map_err(|e| e.to_string())?;
    let fork = forks
        .into_iter()
        .find(|f| f.id == fork_id)
        .ok_or_else(|| format!("fork desconhecido: {fork_id}"))?;
    providers::llama_cpp::list_presets(&fork.models_ini).map_err(|e| e.to_string())
}

/// Backs the status dot in the UI. Two forks can share a port (they're
/// alternatives for the same GPU slot, never meant to run together), so a
/// bare port health-check can't tell them apart — a probe of PrismML's port
/// would come back healthy while TurboQuant is what's actually serving
/// there. AppState's tracking is what's authoritative for "which fork did
/// *this app* start"; the HTTP probe on top only confirms that tracked
/// process hasn't quietly died or hung.
#[tauri::command]
async fn llama_server_health(state: State<'_, AppState>, fork_id: String) -> Result<bool, String> {
    let tracked_and_alive = {
        let mut children = state.llama_children.lock().unwrap();
        match children.get_mut(&fork_id) {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    };
    if !tracked_and_alive {
        return Ok(false);
    }

    let forks = providers::llama_cpp::load_forks(&state.app_data_dir).map_err(|e| e.to_string())?;
    let Some(fork) = forks.into_iter().find(|f| f.id == fork_id) else {
        return Ok(false);
    };
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/health", fork.port);
    let healthy = client
        .get(&url)
        .timeout(std::time::Duration::from_millis(1200))
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false);
    Ok(healthy)
}

#[tauri::command]
async fn start_llama_server(state: State<'_, AppState>, fork_id: String) -> Result<(), String> {
    ensure_llama_ready(&state, &fork_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_llama_server(state: State<AppState>, fork_id: String) -> Result<(), String> {
    if let Some(mut child) = state.llama_children.lock().unwrap().remove(&fork_id) {
        child.start_kill().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Only one local llama.cpp fork can realistically run at a time (they all
/// bind the same port) — this is the single place that enforces that and
/// spares the caller from remembering to stop the old one. Idempotent: a
/// second call for the fork that's already up and healthy is a fast no-op.
/// Used both when the user explicitly starts a fork and when a session
/// auto-starts one on send.
pub(crate) async fn ensure_llama_ready(state: &AppState, fork_id: &str) -> anyhow::Result<()> {
    let already_running = {
        let mut children = state.llama_children.lock().unwrap();
        let others: Vec<String> = children
            .keys()
            .filter(|k| k.as_str() != fork_id)
            .cloned()
            .collect();
        for other in others {
            if let Some(mut child) = children.remove(&other) {
                let _ = child.start_kill();
            }
        }
        match children.get_mut(fork_id) {
            Some(child) => match child.try_wait() {
                Ok(None) => true, // still alive
                _ => {
                    children.remove(fork_id);
                    false
                }
            },
            None => false,
        }
    };

    if already_running {
        return Ok(());
    }

    let forks = providers::llama_cpp::load_forks(&state.app_data_dir)?;
    let fork = forks
        .into_iter()
        .find(|f| f.id == fork_id)
        .ok_or_else(|| anyhow::anyhow!("fork desconhecido: {fork_id}"))?;
    let child = providers::llama_cpp::start_server(&fork).await?;
    state
        .llama_children
        .lock()
        .unwrap()
        .insert(fork_id.to_string(), child);
    Ok(())
}

/// Stops whatever local llama.cpp fork is currently tracked, freeing the
/// GPU — called when a session switches to a provider that isn't
/// llama.cpp, since a stopped-but-forgotten server just sits on VRAM.
fn stop_all_llama_servers(state: &AppState) {
    let mut children = state.llama_children.lock().unwrap();
    for (_, mut child) in children.drain() {
        let _ = child.start_kill();
    }
}

#[tauri::command]
fn list_sessions(state: State<AppState>) -> Result<Vec<Session>, String> {
    sessions::list_sessions(&state.app_data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_session(
    state: State<'_, AppState>,
    title: String,
    provider: ProviderKind,
    model: String,
    project_root: Option<String>,
    fork_id: Option<String>,
    custom_provider_id: Option<String>,
) -> Result<Session, String> {
    let context_length = resolve_context_length(
        state.clone(),
        provider,
        model.clone(),
        fork_id.clone(),
        custom_provider_id.clone(),
    )
    .await
    .unwrap_or(None);
    let session = sessions::create_session(
        &state.app_data_dir,
        title,
        provider,
        model,
        project_root,
        context_length,
        fork_id.clone(),
        custom_provider_id,
    )
    .map_err(|e| e.to_string())?;
    apply_llama_lifecycle(&state, provider, fork_id.as_deref()).await;
    Ok(session)
}

#[tauri::command]
async fn update_session_provider_model(
    state: State<'_, AppState>,
    id: String,
    provider: ProviderKind,
    model: String,
    fork_id: Option<String>,
    custom_provider_id: Option<String>,
) -> Result<Session, String> {
    let context_length = resolve_context_length(
        state.clone(),
        provider,
        model.clone(),
        fork_id.clone(),
        custom_provider_id.clone(),
    )
    .await
    .unwrap_or(None);
    let session = sessions::update_provider_model(
        &state.app_data_dir,
        &id,
        provider,
        model,
        context_length,
        fork_id.clone(),
        custom_provider_id,
    )
    .map_err(|e| e.to_string())?;
    apply_llama_lifecycle(&state, provider, fork_id.as_deref()).await;
    Ok(session)
}

#[tauri::command]
fn update_session_title(
    state: State<AppState>,
    id: String,
    title: String,
) -> Result<Session, String> {
    sessions::update_title(&state.app_data_dir, &id, title).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_session_read_paths(
    state: State<AppState>,
    id: String,
    extra_read_paths: Vec<String>,
) -> Result<Session, String> {
    sessions::update_extra_read_paths(&state.app_data_dir, &id, extra_read_paths)
        .map_err(|e| e.to_string())
}

/// Extrai o texto de um arquivo anexado no composer (pdf/docx/xlsx/md/código/
/// txt - ver `attachments::extract_text`). Roda em `spawn_blocking` porque
/// parsing de pdf/docx/xlsx é trabalho de CPU síncrono, não deveria travar o
/// runtime async enquanto processa um anexo grande.
#[tauri::command]
async fn extract_attachment_text(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        attachments::extract_text(std::path::Path::new(&path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Verifica se o provider/modelo/fork configurados NESTA sessão realmente
/// suportam vision — nunca assume isso a partir do nome do modelo (ver README
/// "Pesquisa: suporte real a imagem/áudio/vídeo por provider"). É essa
/// checagem que decide se o composer deixa anexar imagem ou avisa que não dá.
#[tauri::command]
async fn check_vision_support(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    let session =
        sessions::get_session(&state.app_data_dir, &session_id).map_err(|e| e.to_string())?;
    if session.provider == ProviderKind::LlamaCpp {
        let fork_id = session
            .llama_fork
            .clone()
            .unwrap_or_else(|| state.config.lock().unwrap().active_llama_fork.clone());
        let forks =
            providers::llama_cpp::load_forks(&state.app_data_dir).map_err(|e| e.to_string())?;
        let Some(fork) = forks.into_iter().find(|f| f.id == fork_id) else {
            return Ok(false);
        };
        return Ok(providers::llama_cpp::preset_supports_vision(
            &fork.models_ini,
            &session.model,
        ));
    }
    let (cfg, api_key) = agent::provider_config_for(
        &session.provider,
        &state,
        session.custom_provider_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    Ok(providers::supports_vision(&cfg, api_key, &session.model, &state.app_data_dir).await)
}

/// Envia uma imagem 1x1 pixel pro modelo e verifica se ele responde sem erro.
/// Retorna Ok(true) se o modelo aceitou a imagem, Ok(false) se rejeitou,
/// Err se houve erro de conexão/outro.
#[tauri::command]
async fn test_vision(
    state: State<'_, AppState>,
    kind: ProviderKind,
    custom_provider_id: Option<String>,
    model: String,
) -> Result<bool, String> {
    let cfg = state.config.lock().unwrap().clone();
    let (provider_cfg, api_key) = build_provider_config(
        kind,
        &cfg,
        &state.app_data_dir,
        custom_provider_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // 10x10 pixel PNG vermelho em base64 (imagem pequena mas válida)
    let tiny_png = "iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAYAAACNMs+9AAAAFklEQVQYV2P8z8BQz0AEYBxVOHIxAgALXQMB/1bCiAAAAABJRU5ErkJggg==";
    let data_url = format!("data:image/png;base64,{tiny_png}");

    let client = reqwest::Client::new();
    let url = format!(
        "{}/chat/completions",
        provider_cfg.base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Reply YES"},
                {"type": "image_url", "image_url": {"url": data_url}}
            ]
        }],
        "max_tokens": 5
    });

    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            if status.is_success() {
                let parsed: serde_json::Value = serde_json::from_str(&body_text).unwrap_or_default();
                if parsed["choices"][0]["message"]["content"].as_str().is_some()
                    || parsed["choices"][0]["message"]["reasoning_content"].as_str().is_some()
                {
                    return Ok(true);
                }
                if parsed["error"].is_object() || parsed["error"].is_string() {
                    let err_msg = parsed["error"]["message"].as_str()
                        .or_else(|| parsed["error"].as_str())
                        .unwrap_or("");
                    let lower = err_msg.to_lowercase();
                    if lower.contains("image") || lower.contains("vision") || lower.contains("multimodal")
                        || lower.contains("not support") || lower.contains("unsupported")
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            } else {
                let lower = body_text.to_lowercase();
                if lower.contains("image") || lower.contains("vision") || lower.contains("multimodal")
                    || lower.contains("not support") || lower.contains("unsupported")
                    || lower.contains("content_type") || lower.contains("invalid")
                {
                    Ok(false)
                } else if status.as_u16() == 400 || status.as_u16() == 422 {
                    Ok(false)
                } else {
                    Err(format!("HTTP {status}: {}", &body_text[..body_text.len().min(200)]))
                }
            }
        }
        Err(e) => Err(format!("erro de conexao: {e}")),
    }
}

/// Lê um arquivo de imagem do disco e devolve como data URI base64, pronto
/// pra entrar no array `images` da mensagem — roda em `spawn_blocking` pelo
/// mesmo motivo do `extract_attachment_text` (I/O + encode síncronos).
#[tauri::command]
async fn read_image_as_data_url(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use base64::Engine;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let mime = match std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => "image/jpeg",
        };
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:{mime};base64,{encoded}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Switching a session to/away from llama.cpp should manage the local
/// server without the user having to visit Settings: stop it when nothing
/// needs it anymore, start (or switch fork) when something does. Best-effort
/// on purpose — a failure here doesn't block saving the session choice; the
/// same start attempt happens again (and surfaces a real error) the next
/// time a message is actually sent, see `agent::run_turn`.
async fn apply_llama_lifecycle(state: &AppState, provider: ProviderKind, fork_id: Option<&str>) {
    match (provider, fork_id) {
        (ProviderKind::LlamaCpp, Some(fork)) => {
            let _ = ensure_llama_ready(state, fork).await;
        }
        (ProviderKind::LlamaCpp, None) => {}
        _ => stop_all_llama_servers(state),
    }
}

#[tauri::command]
fn get_session_context_usage(
    state: State<AppState>,
    id: String,
) -> Result<models::ContextUsage, String> {
    let session = sessions::get_session(&state.app_data_dir, &id).map_err(|e| e.to_string())?;
    let messages = sessions::load_messages(&state.app_data_dir, &id).map_err(|e| e.to_string())?;
    let (context_length, is_estimated) = match session.context_length {
        Some(len) => (len, false),
        None => (models::DEFAULT_CONTEXT_LENGTH, true),
    };
    Ok(context::usage_for(
        &id,
        &messages,
        context_length,
        is_estimated,
        session.total_prompt_tokens,
        session.total_completion_tokens,
        session.total_requests,
    ))
}

#[tauri::command]
fn get_session(state: State<AppState>, id: String) -> Result<Session, String> {
    sessions::get_session(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_session_messages(state: State<AppState>, id: String) -> Result<Vec<ChatMessage>, String> {
    sessions::load_messages(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_session_tasks(state: State<AppState>, id: String) -> Result<Vec<TaskItem>, String> {
    sessions::load_tasks(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_session(state: State<AppState>, id: String) -> Result<(), String> {
    sessions::delete_session(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_message(
    app: tauri::AppHandle,
    session_id: String,
    text: String,
    images: Vec<String>,
    display_text: Option<String>,
) -> Result<(), String> {
    // Runs in a detached task: the frontend gets progress via chat:token /
    // agent:tool_call / agent:pending_edit / agent:done events, not the
    // return value of this command. Handle is tracked in `running_turns` so
    // `cancel_turn` can abort it outright (modo "Auto" — o usuário não
    // precisa esperar nenhum checkpoint cooperativo).
    let handle = {
        let app = app.clone();
        let session_id = session_id.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            if let Err(e) = agent::run_turn(
                app.clone(),
                &state,
                session_id.clone(),
                text,
                images,
                display_text,
            )
            .await
            {
                let _ = tauri::Emitter::emit(
                    &app,
                    "agent:error",
                    serde_json::json!({ "session_id": session_id, "message": e.to_string() }),
                );
            }
            app.state::<AppState>()
                .running_turns
                .lock()
                .unwrap()
                .remove(&session_id);
        })
    };
    app.state::<AppState>()
        .running_turns
        .lock()
        .unwrap()
        .insert(session_id, handle);

    Ok(())
}

/// Aborta o turno em andamento de uma sessão — a task async inteira é
/// derrubada (`JoinHandle::abort`), incluindo qualquer chamada HTTP em
/// andamento pro provider, não espera nenhum checkpoint cooperativo. Emite
/// `agent:error` com uma mensagem clara pra UI voltar pro estado idle, já que
/// o próprio `run_turn` não roda mais nada depois do abort (nenhum código
/// dele executa pra emitir seu próprio evento).
///
/// Se a última mensagem salva for do usuário (o assistente não chegou a
/// salvar nada antes do abort), insere uma mensagem placeholder pra manter a
/// alternância user/assistant — sem isso, a próxima mensagem do usuário
/// aparece "grudada" na anterior (bug #03).
#[tauri::command]
fn cancel_turn(
    app: tauri::AppHandle,
    state: State<AppState>,
    session_id: String,
) -> Result<(), String> {
    let handle = state.running_turns.lock().unwrap().remove(&session_id);
    match handle {
        Some(handle) => {
            handle.abort();

            // Garante alternância user/assistant no chat_log: se o turno foi
            // cortado antes do assistente salvar qualquer coisa, a última
            // mensagem ainda é do usuário — insere um placeholder.
            if let Ok(mut messages) = sessions::load_messages(&state.app_data_dir, &session_id) {
                let last_is_user = messages.last().map(|m| m.role == "user").unwrap_or(false);
                if last_is_user {
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: "[Execução cancelada pelo usuário]".to_string(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                        images: Vec::new(),
                        display_content: None,
                    });
                    let _ = sessions::save_messages(&state.app_data_dir, &session_id, &messages);
                }
            }

            let _ = tauri::Emitter::emit(
                &app,
                "agent:error",
                serde_json::json!({ "session_id": session_id, "message": "Execução cancelada pelo usuário." }),
            );
            Ok(())
        }
        None => Err("nenhum turno em execução pra cancelar nesta sessão".to_string()),
    }
}

#[tauri::command]
fn list_pending_edits(state: State<AppState>, session_id: String) -> Vec<PendingEdit> {
    state
        .pending_edits
        .lock()
        .unwrap()
        .values()
        .filter(|e| e.session_id == session_id)
        .cloned()
        .collect()
}

#[tauri::command]
fn accept_edit(state: State<AppState>, edit_id: String) -> Result<(), String> {
    let edit = {
        let mut map = state.pending_edits.lock().unwrap();
        map.remove(&edit_id).ok_or("edicao nao encontrada")?
    };
    sandbox::accept_edit(
        std::path::Path::new(&edit.sandbox_path),
        std::path::Path::new(&edit.target_path),
    )
    .map_err(|e| e.to_string())?;
    agent::walk_cache::invalidate(std::path::Path::new(&edit.target_path));
    Ok(())
}

#[tauri::command]
fn reject_edit(state: State<AppState>, edit_id: String) -> Result<(), String> {
    let edit = {
        let mut map = state.pending_edits.lock().unwrap();
        map.remove(&edit_id).ok_or("edicao nao encontrada")?
    };
    sandbox::reject_edit(std::path::Path::new(&edit.sandbox_path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_attachment_md(
    state: State<AppState>,
    session_id: String,
    filename: String,
    text: String,
) -> Result<String, String> {
    let session_dir = state.app_data_dir.join("sessions").join(&session_id);
    let attach_dir = session_dir.join("attachments");
    std::fs::create_dir_all(&attach_dir).map_err(|e| format!("falha ao criar pasta de anexos: {e}"))?;
    let safe_name = filename
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .trim_end_matches('.')
        .to_string();
    let md_name = format!("{safe_name}.md");
    let md_path = attach_dir.join(&md_name);
    let char_count = text.chars().count();
    let header = format!(
        "---\nsource: \"{filename}\"\nextracted_chars: {char_count}\n---\n\n# Conteúdo extraído de {filename}\n\n"
    );
    std::fs::write(&md_path, format!("{header}{text}")).map_err(|e| format!("falha ao salvar anexo: {e}"))?;
    Ok(md_path.to_string_lossy().to_string())
}

#[tauri::command]
fn answer_ask(state: State<AppState>, id: String, answer: String) -> Result<(), String> {
    let sender = state
        .pending_questions
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("pergunta nao encontrada (id errado, ou ja respondida)")?;
    sender.send(answer).map_err(|_| {
        "nao foi possivel entregar a resposta (a tarefa que perguntou ja desistiu)".to_string()
    })
}

/// Aprova ou recusa uma tool call pendente no modo "Manual" de execução — ver
/// `agent::request_permission`.
#[tauri::command]
fn answer_permission(state: State<AppState>, id: String, approved: bool) -> Result<(), String> {
    let sender = state
        .pending_permissions
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("pedido de permissao nao encontrado (id errado, ou ja respondido)")?;
    sender.send(approved).map_err(|_| {
        "nao foi possivel entregar a resposta (a tarefa que pediu permissao ja desistiu)"
            .to_string()
    })
}

#[tauri::command]
fn update_session_execution_mode(
    state: State<AppState>,
    id: String,
    execution_mode: ExecutionMode,
) -> Result<Session, String> {
    sessions::update_execution_mode(&state.app_data_dir, &id, execution_mode)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_session_context_length(
    state: State<AppState>,
    id: String,
    context_length: Option<u32>,
) -> Result<Session, String> {
    sessions::update_context_length(&state.app_data_dir, &id, context_length)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_session_reasoning_effort(
    state: State<AppState>,
    id: String,
    effort: Option<ReasoningEffort>,
) -> Result<Session, String> {
    sessions::update_reasoning_effort(&state.app_data_dir, &id, effort)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_skills(
    state: State<AppState>,
    project_root: Option<String>,
) -> Result<Vec<SkillMeta>, String> {
    let project_path = project_root.as_deref().map(std::path::Path::new);
    skills::list_skills(&state.app_data_dir, project_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_skill(
    state: State<AppState>,
    name: String,
    description: String,
    language: skills::SkillLanguage,
) -> Result<String, String> {
    skills::create_skill(&state.app_data_dir, &name, &description, language)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn skill_template_body(language: skills::SkillLanguage) -> String {
    skills::template_body(language).to_string()
}

#[tauri::command]
fn read_skill(dir: String) -> Result<String, String> {
    skills::read_skill_file(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_skill(dir: String, content: String) -> Result<(), String> {
    skills::write_skill_file(&dir, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_skills_folder(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let dir = skills::ensure_global_skills_dir(&state.app_data_dir).map_err(|e| e.to_string())?;
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| e.to_string())
}

/// Abre um link (de uma resposta em markdown, por exemplo) no navegador
/// padrão do sistema em vez de navegar a janela do próprio app pra fora —
/// sem isso, clicar num link joga o WebView inteiro pra aquela URL e some
/// com a UI do Cerne Code.
#[tauri::command]
fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<String>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_search_config(state: State<AppState>) -> search::SearchConfigView {
    search::view(&state.app_data_dir)
}

#[tauri::command]
fn save_search_config(
    state: State<AppState>,
    provider: search::SearchProviderKind,
    searxng_url: String,
    api_key: Option<String>,
) -> Result<search::SearchConfigView, String> {
    search::save_config(
        &state.app_data_dir,
        &search::SearchConfig {
            provider,
            searxng_url,
        },
    )
    .map_err(|e| e.to_string())?;
    if let Some(key) = api_key {
        let key = key.trim();
        if !key.is_empty() {
            search::set_key(provider, key).map_err(|e| e.to_string())?;
        }
    }
    Ok(search::view(&state.app_data_dir))
}

#[tauri::command]
fn clear_search_api_key(
    state: State<AppState>,
    provider: search::SearchProviderKind,
) -> Result<search::SearchConfigView, String> {
    search::clear_key(provider).map_err(|e| e.to_string())?;
    Ok(search::view(&state.app_data_dir))
}

#[tauri::command]
async fn test_search_provider(
    provider: search::SearchProviderKind,
    api_key: Option<String>,
    searxng_url: Option<String>,
) -> Result<usize, String> {
    agent::websearch::test_provider(provider, api_key.as_deref(), searxng_url.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_mcp_servers(state: State<AppState>) -> Result<Vec<mcp::McpServerConfig>, String> {
    mcp::load_servers(&state.app_data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_mcp_server(state: State<AppState>, server: mcp::McpServerConfig) -> Result<(), String> {
    let mut servers = mcp::load_servers(&state.app_data_dir).map_err(|e| e.to_string())?;
    servers.retain(|s| s.name != server.name);
    servers.push(server);
    mcp::save_servers(&state.app_data_dir, &servers).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_mcp_server(state: State<AppState>, name: String) -> Result<(), String> {
    let mut servers = mcp::load_servers(&state.app_data_dir).map_err(|e| e.to_string())?;
    servers.retain(|s| s.name != name);
    mcp::save_servers(&state.app_data_dir, &servers).map_err(|e| e.to_string())
}

/// Testa a configuração de um servidor MCP ANTES de salvar (conexão
/// descartável, não entra no pool compartilhado) — devolve os nomes das
/// tools em caso de sucesso, ou uma mensagem de erro específica o bastante
/// pra apontar o que checar.
#[tauri::command]
async fn test_mcp_server(server: mcp::McpServerConfig) -> Result<Vec<String>, String> {
    mcp::test_connection(&server)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();
            let config = config::load_config(&app_data_dir);
            skills::ensure_global_skills_dir(&app_data_dir).ok();
            app.manage(AppState {
                app_data_dir,
                config: Mutex::new(config),
                pending_edits: Mutex::new(HashMap::new()),
                llama_children: Mutex::new(HashMap::new()),
                background_jobs: agent::background::BackgroundJobs::default(),
                mcp_clients: mcp::McpClients::default(),
                pending_questions: Mutex::new(HashMap::new()),
                pending_permissions: Mutex::new(HashMap::new()),
                running_turns: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            set_openrouter_key,
            has_openrouter_key,
            list_provider_models,
            resolve_context_length,
            list_llama_forks,
            add_llama_fork,
            remove_llama_fork,
            list_custom_providers,
            test_custom_provider,
            add_custom_provider,
            remove_custom_provider,
            has_custom_provider_key,
            list_llama_presets,
            llama_server_health,
            start_llama_server,
            stop_llama_server,
            list_sessions,
            create_session,
            update_session_provider_model,
            update_session_title,
            update_session_execution_mode,
            update_session_context_length,
            update_session_reasoning_effort,
            update_session_read_paths,
            extract_attachment_text,
            check_vision_support,
            test_vision,
            read_image_as_data_url,
            get_session,
            get_session_messages,
            get_session_tasks,
            get_session_context_usage,
            delete_session,
            send_message,
            cancel_turn,
            list_pending_edits,
            accept_edit,
            reject_edit,
            save_attachment_md,
            answer_ask,
            answer_permission,
            list_skills,
            create_skill,
            skill_template_body,
            read_skill,
            save_skill,
            open_skills_folder,
            open_external_url,
            list_mcp_servers,
            add_mcp_server,
            remove_mcp_server,
            test_mcp_server,
            get_search_config,
            save_search_config,
            clear_search_api_key,
            test_search_provider,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
