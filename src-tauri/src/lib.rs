mod agent;
mod attachments;
mod audio;
mod backup;
mod config;
mod context;
mod encoding;
mod folders;
mod git;
mod history;
mod mcp;
mod memory;
mod models;
mod osv;
mod personas;
mod providers;
mod python_tools;
mod sandbox;
mod search;
mod sessions;
mod skills;
mod voicebox;

use models::{
    AppConfig, ChatMessage, ExecutionMode, Folder, ModelInfo, PendingEdit, ProviderKind,
    ReasoningEffort, Session, TaskItem,
};
use personas::Persona;
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
    /// Planos de agentes/skills (modo "Manual", Fase A5) esperando aprovacao
    /// batelada do usuario antes de rodar `task`/`load_skill`/
    /// `verify_completion` — mesmo padrao de canal oneshot que
    /// `pending_permissions`, so que aprova/nega TODAS as chamadas listadas
    /// de uma vez em vez de uma por vez.
    pub pending_agent_plans: Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    /// Handle da task async de cada turno em andamento, por sessao — permite
    /// `cancel_turn` abortar um turno inteiro no modo "Auto" (o usuario nao
    /// precisa esperar o proximo checkpoint cooperativo, o abort da tokio
    /// task derruba a chamada HTTP em andamento imediatamente).
    pub running_turns: Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
    /// Registro em memória de execuções de agente/skill em andamento ou
    /// recém-terminadas (`task`/`verify_completion`) — Fase A1 do roteiro de
    /// Agentes/Skills. Não persistido: é só pra UI consultar em tempo real,
    /// não histórico de longo prazo.
    pub agent_executions: Mutex<HashMap<String, models::AgentExecution>>,
    /// Timing de sessões orquestradas (Fase G, `start_agent_session`) — só
    /// pra `check_agent_session` sugerir quanto esperar (G2). Não
    /// persistido, mesmo espírito de `agent_executions`.
    pub orchestrated_sessions: Mutex<HashMap<String, agent::OrchestratedSessionInfo>>,
    /// Guard anti-loop pro auto-continue: quantas vezes seguidas uma sessão
    /// já se retomou sozinha (comando em segundo plano ou sessão filha
    /// orquestrada terminando) sem uma mensagem de verdade do usuário no
    /// meio. Zerado em `send_message` (mensagem real do usuário). Ver
    /// `agent::spawn_auto_continue_turn`.
    pub auto_continue_counts: Mutex<HashMap<String, u32>>,
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

#[tauri::command]
fn openrouter_key_preview() -> Option<String> {
    config::openrouter_key_preview()
}

#[tauri::command]
fn clear_openrouter_key() -> Result<(), String> {
    config::clear_openrouter_key().map_err(|e| e.to_string())
}

/// Monta a config de conexão pra `kind` — pra `Custom`, busca o provider
/// configurado pelo usuário (id/label/base_url em `custom_providers.json`,
/// chave no keyring do SO) em vez de ler campos fixos do `AppConfig`, já que
/// não há como saber de antemão quais providers customizados existem.
///
/// `fork_id`: só relevante pra `LlamaCpp` — qual fork (com sua própria
/// porta) usar. `None` cai no fork ativo global (`cfg.active_llama_fork`).
///
/// T43 (2026-08-15): antes disso, `LlamaCpp` sempre montava a `base_url` a
/// partir de `cfg.llama_cpp_base_url` (fixo, default porta 8082),
/// **ignorando completamente** qual fork estava selecionado — resultado:
/// `ensure_llama_ready` subia o `llama-server` do fork certo (ex: Mainline,
/// porta 8083, já usava `fork.port` corretamente), mas o chat em si tentava
/// falar com a porta errada (8082) e falhava. Agora a porta vem sempre do
/// `LlamaForkConfig` de verdade (mesma fonte que inicia o servidor).
pub(crate) fn build_provider_config(
    kind: ProviderKind,
    cfg: &AppConfig,
    app_data_dir: &PathBuf,
    custom_provider_id: Option<&str>,
    fork_id: Option<&str>,
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

    if kind == ProviderKind::LlamaCpp {
        let fork_id = fork_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| cfg.active_llama_fork.clone());
        let forks =
            providers::llama_cpp::load_forks(app_data_dir).map_err(|e| e.to_string())?;
        let fork = forks
            .into_iter()
            .find(|f| f.id == fork_id)
            .ok_or_else(|| format!("fork llama.cpp desconhecido: {fork_id}"))?;
        let base_url = format!("http://127.0.0.1:{}/v1", fork.port);
        return Ok((
            models::ProviderConfig {
                kind,
                base_url,
                has_api_key: false,
                llama_fork: Some(fork_id),
                supports_vision_override: false,
                context_length_override: None,
            },
            None,
        ));
    }

    let base_url = match kind {
        ProviderKind::Openrouter => cfg.openrouter_base_url.clone(),
        ProviderKind::Ollama => cfg.ollama_base_url.clone(),
        ProviderKind::LmStudio => cfg.lmstudio_base_url.clone(),
        ProviderKind::LlamaCpp | ProviderKind::Custom => unreachable!(),
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
        None,
    )?;
    providers::list_models(&provider_cfg, api_key, &state.app_data_dir)
        .await
        .map_err(|e| e.to_string())
}

/// Favoritos do modal de navegação de modelos, por conexão (`provider_key` =
/// "openrouter"/"ollama"/"lm_studio"/"llama_cpp:{fork}"/"custom:{id}").
#[tauri::command]
fn get_model_favorites(state: State<AppState>, provider_key: String) -> Vec<String> {
    let mut map = config::load_model_favorites(&state.app_data_dir);
    map.remove(&provider_key).unwrap_or_default()
}

#[tauri::command]
fn set_model_favorites(
    state: State<AppState>,
    provider_key: String,
    model_ids: Vec<String>,
) -> Result<(), String> {
    let mut map = config::load_model_favorites(&state.app_data_dir);
    if model_ids.is_empty() {
        map.remove(&provider_key);
    } else {
        map.insert(provider_key, model_ids);
    }
    config::save_model_favorites(&state.app_data_dir, &map).map_err(|e| e.to_string())
}

/// Tamanho de contexto lembrado por modelo (`provider_key::model_id`, mesma
/// convenção de `provider_key` que os favoritos usam) — pedido do usuário
/// pra "colocar o contexto de um LLM por API" uma vez e ele valer em
/// qualquer sessão futura com o mesmo modelo, não só a atual.
#[tauri::command]
fn get_model_context_override(state: State<AppState>, key: String) -> Option<u32> {
    let map = config::load_model_context_overrides(&state.app_data_dir);
    map.get(&key).copied()
}

#[tauri::command]
fn set_model_context_override(
    state: State<AppState>,
    key: String,
    context_length: Option<u32>,
) -> Result<(), String> {
    let mut map = config::load_model_context_overrides(&state.app_data_dir);
    match context_length {
        Some(len) => map.insert(key, len),
        None => map.remove(&key),
    };
    config::save_model_context_overrides(&state.app_data_dir, &map).map_err(|e| e.to_string())
}

/// Mesma convenção de chave que `get_model_favorites`/`get_model_context_override`
/// usam (`provider_key::model_id`) — replicada aqui (em vez de reusar uma só
/// implementação) porque o lado Rust nunca tinha essa necessidade antes: até
/// agora só o frontend computava essa chave (`modelContextOverrideKey` em
/// `stores/provider.ts`), já que só ele chamava `get_model_context_override`.
fn model_context_override_key(
    kind: ProviderKind,
    model: &str,
    fork_id: Option<&str>,
    custom_provider_id: Option<&str>,
) -> String {
    let provider_key = match kind {
        ProviderKind::LlamaCpp => format!("llama_cpp:{}", fork_id.unwrap_or("")),
        ProviderKind::Custom => format!("custom:{}", custom_provider_id.unwrap_or("")),
        ProviderKind::Openrouter => "openrouter".to_string(),
        ProviderKind::Ollama => "ollama".to_string(),
        ProviderKind::LmStudio => "lm_studio".to_string(),
    };
    format!("{provider_key}::{model}")
}

#[cfg(test)]
mod model_context_override_key_tests {
    use super::*;

    #[test]
    fn matches_frontend_convention_for_openrouter() {
        assert_eq!(
            model_context_override_key(ProviderKind::Openrouter, "z-ai/glm-latest", None, None),
            "openrouter::z-ai/glm-latest"
        );
    }

    #[test]
    fn matches_frontend_convention_for_llama_cpp_fork() {
        assert_eq!(
            model_context_override_key(ProviderKind::LlamaCpp, "qwen3-8b", Some("turboquant"), None),
            "llama_cpp:turboquant::qwen3-8b"
        );
    }

    #[test]
    fn matches_frontend_convention_for_custom_provider() {
        assert_eq!(
            model_context_override_key(ProviderKind::Custom, "claude-sonnet", None, Some("claude-conn")),
            "custom:claude-conn::claude-sonnet"
        );
    }

    #[test]
    fn distinguishes_ollama_from_lm_studio() {
        assert_ne!(
            model_context_override_key(ProviderKind::Ollama, "llama3", None, None),
            model_context_override_key(ProviderKind::LmStudio, "llama3", None, None),
        );
    }
}

/// Tamanho de contexto pra uma sessão nova (ou trocando de modelo): o valor
/// LEMBRADO pelo usuário pro modelo (`model_context_overrides.json`) sempre
/// vence, se existir — só cai no lookup automático (`resolve_context_length`,
/// que pra OpenRouter quase sempre acha um valor real via `/models`) quando
/// não há nada lembrado ainda.
///
/// Bug real encontrado testando ao vivo, 2026-08-20: sem essa prioridade, o
/// valor lembrado NUNCA tinha chance de ser aplicado em conexões de API —
/// `resolve_context_length` já preenchia `context_length` na hora de criar a
/// sessão (OpenRouter documenta o contexto de quase todo modelo), e o
/// frontend (`applyRememberedContextLength`, `stores/session.ts`) só aplica
/// o valor lembrado quando a sessão AINDA não tem nenhum — nunca disparava
/// pra API. Em provider local (llama.cpp/Ollama/LM Studio), o lookup
/// automático falha com mais frequência (deixa `None`), por isso "local já
/// funcionava" e só a API estava presa no valor automático.
async fn resolve_session_context_length(
    state: State<'_, AppState>,
    kind: ProviderKind,
    model: &str,
    fork_id: Option<&str>,
    custom_provider_id: Option<&str>,
) -> Option<u32> {
    let key = model_context_override_key(kind, model, fork_id, custom_provider_id);
    if let Some(remembered) = config::load_model_context_overrides(&state.app_data_dir).get(&key).copied() {
        return Some(remembered);
    }
    resolve_context_length(
        state,
        kind,
        model.to_string(),
        fork_id.map(|s| s.to_string()),
        custom_provider_id.map(|s| s.to_string()),
    )
    .await
    .unwrap_or(None)
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
        None,
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

/// Mata de vez todo `llama-server` ainda tracked, via `taskkill /T /F` —
/// chamado no fechamento do app (ver `run`). `stop_all_llama_servers` acima
/// usa só `start_kill()`/`kill_on_drop`, que dependem do runtime async ter
/// chance de rodar o kill — no fechamento do app isso não é garantido (o
/// processo do Cerne pode sumir antes do kill assíncrono terminar), e o
/// llama-server fica órfão consumindo RAM/VRAM pra sempre. Síncrono de
/// propósito, mesmo motivo do `BackgroundJobs::kill_all_blocking`.
fn kill_all_llama_children_blocking(state: &AppState) {
    let pids: Vec<u32> = {
        let children = state.llama_children.lock().unwrap();
        children.values().filter_map(|c| c.id()).collect()
    };
    for pid in pids {
        agent::shell::kill_pid_tree_blocking(pid);
    }
    state.llama_children.lock().unwrap().clear();
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
    let context_length = resolve_session_context_length(
        state.clone(),
        provider,
        &model,
        fork_id.as_deref(),
        custom_provider_id.as_deref(),
    )
    .await;
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
    let context_length = resolve_session_context_length(
        state.clone(),
        provider,
        &model,
        fork_id.as_deref(),
        custom_provider_id.as_deref(),
    )
    .await;
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
    extra_read_paths: Vec<crate::models::FolderEntry>,
) -> Result<Session, String> {
    sessions::update_extra_read_paths(&state.app_data_dir, &id, extra_read_paths)
        .map_err(|e| e.to_string())
}

/// Fase D1: troca a pasta de trabalho (project_root) de uma sessão já
/// existente — ver `sessions::update_project_root`.
#[tauri::command]
fn update_session_project_root(
    state: State<AppState>,
    id: String,
    project_root: Option<String>,
) -> Result<Session, String> {
    sessions::update_project_root(&state.app_data_dir, &id, project_root).map_err(|e| e.to_string())
}

/// Verifica se um caminho é um diretório existente. Usado pelo composer para
/// detectar quando o usuário cola um caminho de pasta e oferecer adicioná-la.
#[tauri::command]
fn check_path_is_directory(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

#[derive(serde::Serialize)]
struct DirEntryInfo {
    name: String,
    path: String,
    is_dir: bool,
}

/// Fase D2 do roteiro de Agentes/Skills: lista o conteúdo de UMA pasta (não
/// recursivo) — pro `FileBrowser.vue` carregar sob demanda conforme o
/// usuário expande nós da árvore, em vez de ler tudo de uma vez (mais
/// simples e não trava em pastas gigantes tipo `node_modules`). Mesmo
/// espírito de leitura sem restrição extra que a tool `list_dir` do agente
/// já tem — quem está navegando é o próprio usuário, não o LLM.
#[tauri::command]
fn list_dir_entries(path: String) -> Result<Vec<DirEntryInfo>, String> {
    let dir = std::path::Path::new(&path);
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        // Esconde ocultos (.git, .cerne, .env, etc.) — ruído que o usuário
        // quase nunca quer navegar manualmente.
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(DirEntryInfo {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

/// Fase D1 revisitada (2026-08-17): diff de repositório de verdade, pego do
/// `git` — substitui a reconstrução a partir do histórico de tool calls que
/// `RepoDiffViewer.vue` usava antes (perdia edição manual do usuário, não
/// distinguia arquivo novo/deletado/renomeado direito). Roda em
/// `spawn_blocking` porque `std::process::Command` bloqueia a thread
/// enquanto o subprocesso `git` roda.
#[tauri::command]
async fn git_repo_status(project_root: String) -> Result<Vec<git::GitFileChange>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        git::status(std::path::Path::new(&project_root)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_file_diff(project_root: String, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        git::diff_file(std::path::Path::new(&project_root), &path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct ExportSessionsResult {
    exported: usize,
}

#[tauri::command]
async fn export_sessions_backup(
    state: tauri::State<'_, AppState>,
    session_ids: Vec<String>,
    dest_path: String,
) -> Result<ExportSessionsResult, String> {
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        backup::export_sessions_zip(&app_data_dir, &session_ids, std::path::Path::new(&dest_path))
            .map(|exported| ExportSessionsResult { exported })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn import_sessions_backup(
    state: tauri::State<'_, AppState>,
    source_path: String,
) -> Result<backup::ImportSummary, String> {
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        backup::import_sessions_zip(&app_data_dir, std::path::Path::new(&source_path))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct BackupGitStatus {
    is_repo: bool,
    remote: Option<String>,
    identity_name: Option<String>,
    identity_email: Option<String>,
    has_token: bool,
    token_preview: Option<String>,
}

fn backup_git_dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    app_data_dir.join("sessions")
}

#[tauri::command]
async fn backup_git_status(state: tauri::State<'_, AppState>) -> Result<BackupGitStatus, String> {
    let dir = backup_git_dir(&state.app_data_dir);
    tauri::async_runtime::spawn_blocking(move || {
        let (identity_name, identity_email) = git::get_local_identity(&dir);
        BackupGitStatus {
            is_repo: git::is_repo(&dir),
            remote: git::get_remote(&dir),
            identity_name,
            identity_email,
            has_token: config::has_backup_git_token(),
            token_preview: config::backup_git_token_preview(),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn backup_git_init(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let dir = backup_git_dir(&state.app_data_dir);
    tauri::async_runtime::spawn_blocking(move || git::init_repo(&dir).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn backup_git_set_remote(state: tauri::State<'_, AppState>, url: String) -> Result<(), String> {
    let dir = backup_git_dir(&state.app_data_dir);
    tauri::async_runtime::spawn_blocking(move || {
        git::set_remote(&dir, &url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn backup_git_set_identity(
    state: tauri::State<'_, AppState>,
    name: String,
    email: String,
) -> Result<(), String> {
    let dir = backup_git_dir(&state.app_data_dir);
    tauri::async_runtime::spawn_blocking(move || {
        git::set_local_identity(&dir, &name, &email).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn set_backup_git_token(token: String) -> Result<(), String> {
    config::set_backup_git_token(&token).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_backup_git_token() -> Result<(), String> {
    config::clear_backup_git_token().map_err(|e| e.to_string())
}

#[tauri::command]
async fn backup_git_sync(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let dir = backup_git_dir(&state.app_data_dir);
    tauri::async_runtime::spawn_blocking(move || {
        let token = config::get_backup_git_token();
        git::sync(&dir, token.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_memory(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || memory::load_memory(&app_data_dir).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_memory(state: tauri::State<'_, AppState>, content: String) -> Result<(), String> {
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        memory::save_memory(&app_data_dir, &content).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct TtsResult {
    audio_base64: String,
    /// MIME real do áudio devolvido — a OpenAI/OpenRouter mandam MP3, então
    /// o frontend usa isso pra montar a data URI certa em vez de assumir
    /// MP3 sempre. Vazio quando `play_locally` é falso (nada pra tocar).
    mime: String,
    /// Quando falso, o Cerne não deve criar/tocar nenhum `<audio>` — o
    /// Voicebox já toca a fala sozinho (opção "Autoplay on generate" dele,
    /// ligada por padrão). Pedido do usuário, 2026-08-20: com os dois
    /// tocando, soava como fala duplicada/eco; a correção é deixar só o
    /// Voicebox tocar e o Cerne só disparar e mostrar o estado.
    play_locally: bool,
}

#[tauri::command]
async fn tts_speak(state: State<'_, AppState>, text: String) -> Result<TtsResult, String> {
    let cfg = state.config.lock().unwrap().clone();

    if cfg.tts_backend == models::VoiceBackend::Voicebox {
        let language = if cfg.tts_auto_language {
            Some(audio::detect_language(&text))
        } else {
            None
        };
        voicebox::speak(&cfg.voicebox_base_url, &text, &cfg.voicebox_tts_profile, language)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(TtsResult { audio_base64: String::new(), mime: String::new(), play_locally: false });
    }

    let (provider_cfg, api_key) = build_provider_config(
        cfg.tts_provider,
        &cfg,
        &state.app_data_dir,
        cfg.tts_custom_provider_id.as_deref(),
        cfg.tts_llama_fork.as_deref(),
    )?;
    if cfg.tts_provider == ProviderKind::Openrouter && api_key.is_none() {
        return Err("chave da OpenRouter nao configurada".to_string());
    }
    let voice = audio::resolve_voice(&cfg.tts_model, &cfg.tts_voice, &text, cfg.tts_auto_language);
    let audio_base64 = audio::synthesize_speech(&provider_cfg.base_url, api_key.as_deref(), &text, &cfg.tts_model, &voice)
        .await
        .map_err(|e| e.to_string())?;
    Ok(TtsResult { audio_base64, mime: "audio/mpeg".to_string(), play_locally: true })
}

#[tauri::command]
async fn stt_transcribe(
    state: State<'_, AppState>,
    audio_base64: String,
    format: String,
) -> Result<String, String> {
    let cfg = state.config.lock().unwrap().clone();

    if cfg.stt_backend == models::VoiceBackend::Voicebox {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&audio_base64)
            .map_err(|e| format!("audio base64 invalido: {e}"))?;
        let language = if cfg.voicebox_stt_language.trim().is_empty() {
            None
        } else {
            Some(cfg.voicebox_stt_language.as_str())
        };
        return voicebox::transcribe_audio(&cfg.voicebox_base_url, bytes, &format, language)
            .await
            .map_err(|e| e.to_string());
    }

    let (provider_cfg, api_key) = build_provider_config(
        cfg.stt_provider,
        &cfg,
        &state.app_data_dir,
        cfg.stt_custom_provider_id.as_deref(),
        cfg.stt_llama_fork.as_deref(),
    )?;
    if cfg.stt_provider == ProviderKind::Openrouter && api_key.is_none() {
        return Err("chave da OpenRouter nao configurada".to_string());
    }
    audio::transcribe_audio(
        &provider_cfg.base_url,
        api_key.as_deref(),
        &audio_base64,
        &format,
        &cfg.stt_model,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Checagem genérica de dependência externa opcional (`git`, `uv`, `node`) —
/// 2026-08-17, pedido do usuário testando ao vivo: um usuário só com o
/// Cerne Code instalado (sem `uv`, por exemplo) batia num erro de shell
/// confuso na primeira vez que uma ferramenta Python era usada, sem
/// entender o motivo. UI consulta isso pra mostrar um aviso claro em vez de
/// deixar a ferramenta falhar silenciosamente depois.
#[tauri::command]
async fn check_command_available(name: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || agent::shell::command_exists(&name))
        .await
        .unwrap_or(false)
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
        None,
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
        None,
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

    // Antes isto caía direto em `DEFAULT_CONTEXT_LENGTH` (8192) quando a sessão
    // não tinha `context_length`, enquanto `run_turn` resolvia o valor de
    // verdade (tabela de modelos + cache) — ou seja, a mesma sessão podia
    // mostrar 8.192 no medidor e usar 1M no turno. Agora passa pelo mesmo
    // caminho do turno, então os dois concordam.
    let context_length = session.context_length.unwrap_or_else(|| {
        providers::resolve_context_length(&state.app_data_dir, &session.model, None)
    });
    let is_estimated_length = session.context_length.is_none();

    // As tool specs fazem parte do que vai no request, então precisam entrar na
    // estimativa (ver `context.rs`). Como este comando não tem o contexto do
    // turno, reconstrói o conjunto que a sessão realmente usaria: as sempre
    // presentes, as de projeto (quando há pasta) e as de orquestração (quando
    // não é sessão filha) — mesma regra do `run_turn`.
    let mut tool_specs = agent::tools::always_tool_specs();
    if session.project_root.is_some() || !session.extra_read_paths.is_empty() {
        tool_specs.extend(agent::tools::project_tool_specs());
    }
    if session.parent_session_id.is_none() {
        tool_specs.extend(agent::tools::orchestration_tool_specs());
    }

    // As tools de `computer_use` só entram quando o modelo tem visão (ver
    // `run_turn`), e são ~1,9 mil tokens — deixá-las de fora erra a estimativa
    // em 22% numa sessão de visão (medido contra o `prompt_tokens` real).
    //
    // O `run_turn` descobre isso chamando `supports_vision`, que faz request de
    // rede — não dá pra pagar esse custo toda vez que o usuário abre uma sessão
    // na sidebar. Então usa a evidência local disponível: **se o histórico
    // desta sessão tem alguma imagem, o modelo aceitou imagem** — quem decidiu
    // isso foi o provider, e o app só deixa anexar imagem depois de confirmar
    // visão.
    //
    // Limitação aceita: sessão nova (sem histórico) com modelo de visão fica
    // sem as tools na estimativa e subestima ~22%. É o caso menos provável
    // (sem imagem anexada, o computer_use dificilmente vai ser usado) e dura
    // só até a primeira resposta — depois o medidor usa o `prompt_tokens` real.
    let tem_evidencia_de_visao = messages.iter().any(|m| !m.images.is_empty());
    if tem_evidencia_de_visao {
        tool_specs.extend(agent::computer::tool_specs());
    }

    Ok(context::usage_for(context::UsageInputs {
        session_id: &id,
        messages: &messages,
        tool_specs: &tool_specs,
        model: &session.model,
        context_length,
        is_estimated_length,
        // O prompt_tokens real da última requisição é a fonte da verdade.
        real_used_tokens: session.last_prompt_tokens,
        total_prompt_tokens: session.total_prompt_tokens,
        total_completion_tokens: session.total_completion_tokens,
        total_requests: session.total_requests,
    }))
}

/// Lista as execuções de agente/skill (`task`/`verify_completion`) em
/// andamento ou recém-terminadas, de qualquer sessão — mesmo escopo global
/// ao app que `list_background` já usa pra jobs em segundo plano.
#[tauri::command]
fn list_agent_executions(state: State<AppState>) -> Vec<models::AgentExecution> {
    let mut executions: Vec<models::AgentExecution> =
        state.agent_executions.lock().unwrap().values().cloned().collect();
    // Mais recente primeiro — pedido do usuário testando ao vivo: o painel é
    // um histórico, e ele quer ver o último comando/skill rodado no topo, sem
    // ter que rolar passando pelos mais antigos.
    executions.sort_by_key(|e| std::cmp::Reverse(e.started_at_ms));
    executions
}

/// Fase C1: versão pro painel da UI de `list_background` (que já existe
/// como tool do LLM) — antes o frontend não tinha NENHUM comando Tauri pra
/// listar jobs em segundo plano, só via evento `agent:background_output`
/// (push, mas sem descoberta inicial dos jobs já rodando).
#[tauri::command]
fn list_background_jobs(state: State<AppState>) -> Vec<agent::background::BackgroundJobInfo> {
    state.background_jobs.list_structured()
}

/// Fase C1: encerra um job em segundo plano a pedido do usuário (botão
/// "cancelar" no painel) — mesma lógica de `stop_background` (tool do LLM),
/// só exposta como comando Tauri direto.
#[tauri::command]
async fn stop_background_job(state: State<'_, AppState>, id: String) -> Result<String, String> {
    state.background_jobs.stop(&id).await.map_err(|e| e.to_string())
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
    // Mensagem de verdade do usuário — zera o guard anti-loop do
    // auto-continue (job em segundo plano / sessão filha reativando a
    // sessão sozinha, ver `agent::spawn_auto_continue_turn`). Sem isso, um
    // job em background terminando logo após você mandar uma mensagem
    // manual contaria contra o mesmo limite de tentativas.
    app.state::<AppState>()
        .auto_continue_counts
        .lock()
        .unwrap()
        .remove(&session_id);

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
                // O abort pode ter acontecido DEPOIS do assistente já ter
                // salvo um `tool_calls` em disco (run_turn salva o pedido
                // antes de executar a ferramenta), deixando o histórico com um
                // pedido sem resposta — o provider passa a responder 400 em
                // toda mensagem seguinte. `repair` fecha esse grupo com uma
                // resposta sintética avisando que a execução foi cancelada.
                history::repair(&mut messages);
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
                }
                let _ = sessions::save_messages(&state.app_data_dir, &session_id, &messages);
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

/// Aprova ou recusa um plano de agentes/skills pendente (Fase A5) — ver
/// `agent::request_agents_skills_plan`.
#[tauri::command]
fn answer_agents_skills_plan(state: State<AppState>, id: String, approved: bool) -> Result<(), String> {
    let sender = state
        .pending_agent_plans
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("plano de agentes/skills nao encontrado (id errado, ou ja respondido)")?;
    sender.send(approved).map_err(|_| {
        "nao foi possivel entregar a resposta (a tarefa que pediu o plano ja desistiu)".to_string()
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
fn update_session_fable_method(
    state: State<AppState>,
    id: String,
    enabled: bool,
) -> Result<Session, String> {
    sessions::update_fable_method(&state.app_data_dir, &id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_session_mcp_servers(
    state: State<AppState>,
    id: String,
    enabled_names: Option<Vec<String>>,
) -> Result<Session, String> {
    sessions::update_enabled_mcp_servers(&state.app_data_dir, &id, enabled_names)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_session_persona(
    state: State<AppState>,
    id: String,
    persona_id: Option<String>,
) -> Result<Session, String> {
    sessions::update_persona(&state.app_data_dir, &id, persona_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_personas(state: State<AppState>) -> Result<Vec<Persona>, String> {
    personas::list_personas(&state.app_data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_persona(
    state: State<AppState>,
    name: String,
    content: String,
    tools: Vec<String>,
    skills: Vec<String>,
    kind: personas::PersonaKind,
) -> Result<Persona, String> {
    personas::create_persona(&state.app_data_dir, &name, &content, tools, skills, kind)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_persona(
    state: State<AppState>,
    id: String,
    name: String,
    content: String,
    tools: Vec<String>,
    skills: Vec<String>,
    kind: personas::PersonaKind,
) -> Result<Persona, String> {
    personas::update_persona(&state.app_data_dir, &id, &name, &content, tools, skills, kind)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_persona(state: State<AppState>, id: String) -> Result<(), String> {
    personas::delete_persona(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

/// T29: pastas na lista de sessões da barra lateral, 2 níveis.
#[tauri::command]
fn list_folders(state: State<AppState>) -> Result<Vec<Folder>, String> {
    folders::list_folders(&state.app_data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_folder(
    state: State<AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<Folder, String> {
    folders::create_folder(&state.app_data_dir, &name, parent_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_folder(state: State<AppState>, id: String, name: String) -> Result<Folder, String> {
    folders::rename_folder(&state.app_data_dir, &id, &name).map_err(|e| e.to_string())
}

/// Devolve os ids das subpastas que ficaram órfãs (movidas pra raiz) — o
/// frontend usa isso pra atualizar a árvore local sem precisar recarregar
/// tudo de novo.
#[tauri::command]
fn delete_folder(state: State<AppState>, id: String) -> Result<Vec<String>, String> {
    folders::delete_folder(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_session_folder(
    state: State<AppState>,
    id: String,
    folder_id: Option<String>,
) -> Result<Session, String> {
    sessions::update_folder(&state.app_data_dir, &id, folder_id).map_err(|e| e.to_string())
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

/// Fase 6, Degrau 1 (skill store — importar de URL, sem registry/hospedagem
/// nova): busca o conteúdo pro preview no `SkillImportModal.vue`.
#[tauri::command]
async fn fetch_skill_from_url(url: String) -> Result<String, String> {
    skills::fetch_skill_from_url(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn import_skill(state: State<AppState>, content: String) -> Result<String, String> {
    skills::import_skill(&state.app_data_dir, &content)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// T17: ferramentas Python que o próprio LLM cria (`create_python_tool`/
/// `update_python_tool`, ver `agent/tools.rs`) — o usuário só vê/edita/apaga
/// pela UI, não cria uma do zero por aqui (ver `python_tools.rs`).
#[tauri::command]
fn list_python_tools(state: State<AppState>) -> Result<Vec<python_tools::PythonTool>, String> {
    python_tools::list_python_tools(&state.app_data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_python_tool(
    state: State<AppState>,
    name: String,
    description: String,
    script: String,
    dependencies: Vec<String>,
) -> Result<python_tools::PythonTool, String> {
    python_tools::update_python_tool(&state.app_data_dir, &name, &description, &script, dependencies)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_python_tool(state: State<AppState>, name: String) -> Result<(), String> {
    python_tools::delete_python_tool(&state.app_data_dir, &name).map_err(|e| e.to_string())
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
    google_cse_id: Option<String>,
    bing_endpoint: Option<String>,
) -> Result<search::SearchConfigView, String> {
    let previous = search::load_config(&state.app_data_dir);
    search::save_config(
        &state.app_data_dir,
        &search::SearchConfig {
            provider,
            searxng_url,
            google_cse_id: google_cse_id.unwrap_or(previous.google_cse_id),
            bing_endpoint: bing_endpoint
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(previous.bing_endpoint),
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
    google_cse_id: Option<String>,
    bing_endpoint: Option<String>,
) -> Result<usize, String> {
    agent::websearch::test_provider(
        provider,
        api_key.as_deref(),
        searxng_url.as_deref(),
        google_cse_id.as_deref(),
        bing_endpoint.as_deref(),
    )
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
async fn test_mcp_server(server: mcp::McpServerConfig) -> Result<Vec<mcp::McpToolInfo>, String> {
    mcp::test_connection(&server)
        .await
        .map_err(|e| e.to_string())
}

/// Modal "ver ferramentas" do composer (2026-08-17): se o servidor já
/// estiver conectado no pool compartilhado (habilitado nesta sessão de
/// app), reaproveita essa conexão em vez de abrir uma segunda em paralelo
/// pro mesmo comando — algumas implementações de servidor MCP não toleram
/// duas instâncias ao mesmo tempo e derrubavam o handshake (achado ao
/// vivo). Só cai pra uma conexão descartável (`test_mcp_server`) quando o
/// servidor ainda não está conectado (nunca habilitado, ou desconectado).
#[tauri::command]
async fn list_mcp_server_tools(
    state: State<'_, AppState>,
    server: mcp::McpServerConfig,
) -> Result<Vec<mcp::McpToolInfo>, String> {
    if let Some(tools) = state.mcp_clients.list_tools(&server.name).await {
        return Ok(tools);
    }
    mcp::test_connection(&server).await.map_err(|e| e.to_string())
}

/// Verifica se o usuário já aceitou o disclaimer de responsabilidade.
#[tauri::command]
fn get_disclaimer_accepted(state: State<AppState>) -> bool {
    let path = state.app_data_dir.join("disclaimer_accepted");
    path.exists()
}

/// Marca o disclaimer como aceito (cria arquivo marcador).
#[tauri::command]
fn set_disclaimer_accepted(state: State<AppState>, _accepted: bool) -> Result<(), String> {
    let path = state.app_data_dir.join("disclaimer_accepted");
    std::fs::write(&path, "accepted").map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    // TCC (Accessibility/Screen Recording/etc.) so existe no macOS - o
    // computer_use la depende de check()/request() guiado na UI antes de
    // screenshot/click funcionarem.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_macos_permissions::init());

    builder
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
                background_jobs: agent::background::BackgroundJobs::new(app.handle().clone()),
                mcp_clients: mcp::McpClients::default(),
                pending_questions: Mutex::new(HashMap::new()),
                pending_permissions: Mutex::new(HashMap::new()),
                pending_agent_plans: Mutex::new(HashMap::new()),
                running_turns: Mutex::new(HashMap::new()),
                agent_executions: Mutex::new(HashMap::new()),
                orchestrated_sessions: Mutex::new(HashMap::new()),
                auto_continue_counts: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            export_sessions_backup,
            import_sessions_backup,
            backup_git_status,
            backup_git_init,
            backup_git_set_remote,
            backup_git_set_identity,
            set_backup_git_token,
            clear_backup_git_token,
            backup_git_sync,
            tts_speak,
            stt_transcribe,
            get_memory,
            set_memory,
            set_openrouter_key,
            has_openrouter_key,
            openrouter_key_preview,
            clear_openrouter_key,
            get_disclaimer_accepted,
            set_disclaimer_accepted,
            list_provider_models,
            get_model_favorites,
            set_model_favorites,
            get_model_context_override,
            set_model_context_override,
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
            update_session_fable_method,
            update_session_mcp_servers,
            update_session_persona,
            list_personas,
            create_persona,
            update_persona,
            delete_persona,
            list_folders,
            create_folder,
            rename_folder,
            delete_folder,
            update_session_folder,
            update_session_read_paths,
            update_session_project_root,
            check_path_is_directory,
            list_dir_entries,
            git_repo_status,
            git_file_diff,
            check_command_available,
            extract_attachment_text,
            check_vision_support,
            test_vision,
            read_image_as_data_url,
            get_session,
            get_session_messages,
            get_session_tasks,
            get_session_context_usage,
            list_agent_executions,
            list_background_jobs,
            stop_background_job,
            delete_session,
            send_message,
            cancel_turn,
            list_pending_edits,
            accept_edit,
            reject_edit,
            save_attachment_md,
            answer_ask,
            answer_permission,
            answer_agents_skills_plan,
            list_skills,
            create_skill,
            skill_template_body,
            read_skill,
            save_skill,
            open_skills_folder,
            fetch_skill_from_url,
            import_skill,
            list_python_tools,
            update_python_tool,
            delete_python_tool,
            open_external_url,
            list_mcp_servers,
            add_mcp_server,
            remove_mcp_server,
            test_mcp_server,
            list_mcp_server_tools,
            get_search_config,
            save_search_config,
            clear_search_api_key,
            test_search_provider,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Fechar a ultima janela dispara isso antes do processo do app
            // realmente sumir - e a unica chance confiavel de matar
            // llama-server/dev-servers em background (ver funcoes acima:
            // sem isso eles ficam orfaos consumindo RAM/VRAM pra sempre).
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                kill_all_llama_children_blocking(&state);
                state.background_jobs.kill_all_blocking();
            }
        });
}
