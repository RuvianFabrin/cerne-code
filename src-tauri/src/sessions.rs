use crate::models::{
    ChatMessage, ExecutionMode, FixedContextState, LongHorizonState, MessagesPage, ProviderKind, ReasoningEffort,
    Session, TaskItem,
};
use anyhow::Result;
use std::path::PathBuf;

fn sessions_dir(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("sessions")
}

fn session_dir(app_data_dir: &PathBuf, id: &str) -> PathBuf {
    sessions_dir(app_data_dir).join(id)
}

pub fn list_sessions(app_data_dir: &PathBuf) -> Result<Vec<Session>> {
    let dir = sessions_dir(app_data_dir);
    std::fs::create_dir_all(&dir)?;
    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let meta_path = entry.path().join("session.json");
        if let Ok(text) = std::fs::read_to_string(&meta_path) {
            if let Ok(session) = serde_json::from_str::<Session>(&text) {
                sessions.push(session);
            }
        }
    }
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(sessions)
}

#[allow(clippy::too_many_arguments)]
pub fn create_session(
    app_data_dir: &PathBuf,
    title: String,
    provider: ProviderKind,
    model: String,
    project_root: Option<String>,
    context_length: Option<u32>,
    llama_fork: Option<String>,
    custom_provider_id: Option<String>,
    external_cli_backend: Option<String>,
) -> Result<Session> {
    let id = uuid::Uuid::new_v4().to_string();
    let session = Session {
        id: id.clone(),
        title,
        created_at: chrono::Utc::now(),
        provider,
        model,
        project_root,
        context_length,
        llama_fork,
        custom_provider_id,
        external_cli_backend,
        external_cli_mode: Default::default(),
        external_cli_session_id: None,
        extra_read_paths: Vec::new(),
        execution_mode: ExecutionMode::default(),
        // Pensamento sempre desligado por padrão — o usuário liga manualmente
        // no composer quando precisar. Antes, providers cloud ficavam em Auto
        // e modelos locais em Off; agora é uniforme: Off pra todos.
        reasoning_effort: Some(ReasoningEffort::Off),
        // Fase E3 do roteiro de Agentes/Skills: sessão nova nasce com TODOS
        // os MCPs desabilitados (Some(vec![]), não None) — o usuário liga
        // explicitamente pelo modal "MCPs" do composer os que quiser usar
        // nessa sessão. `None` continua significando "todos habilitados"
        // pra sessões já existentes antes dessa mudança (retrocompatível —
        // esse default só afeta sessão criada a partir de agora).
        enabled_mcp_servers: Some(Vec::new()),
        browser_profile: None,
        anything: Default::default(),
        fable_method: false,
        persona_id: None,
        total_prompt_tokens: 0,
        total_completion_tokens: 0,
        total_requests: 0,
        // Sem requisição ainda — o medidor estima até a primeira resposta.
        last_prompt_tokens: None,
        folder_id: None,
        parent_session_id: None,
        long_horizon: LongHorizonState::default(),
        fixed_context: FixedContextState::default(),
        task_queue_enabled: false,
        plain_queue_enabled: false,
        plain_queue_path: None,
        context_economy: false,
    };
    let dir = session_dir(app_data_dir, &id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    std::fs::write(dir.join("chat_log.json"), "[]")?;
    std::fs::write(dir.join("tasks.json"), "[]")?;
    Ok(session)
}

pub fn get_session(app_data_dir: &PathBuf, id: &str) -> Result<Session> {
    let text = std::fs::read_to_string(session_dir(app_data_dir, id).join("session.json"))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn update_cli_mode(app_data_dir: &PathBuf, id: &str, mode: crate::agent::cli_inference::CliMode) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.external_cli_mode = mode;
    write_atomic(&session_dir(app_data_dir, id).join("session.json"), &serde_json::to_string_pretty(&session)?)?;
    Ok(session)
}

pub fn update_external_cli_session(app_data_dir: &PathBuf, id: &str, cli_session_id: Option<String>) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.external_cli_session_id = cli_session_id;
    write_atomic(&session_dir(app_data_dir, id).join("session.json"), &serde_json::to_string_pretty(&session)?)?;
    Ok(session)
}

pub fn update_context_economy(app_data_dir: &PathBuf, id: &str, enabled: bool) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.context_economy = enabled;
    session.last_prompt_tokens = None;
    std::fs::write(session_dir(app_data_dir, id).join("session.json"), serde_json::to_vec_pretty(&session)?)?;
    Ok(session)
}

/// Updates which provider/model a session talks to. Sessions pin their
/// provider+model at creation and never re-read the global config, so this
/// is the only way to actually change what an existing session sends —
/// editing the picker alone is not enough.
pub fn update_provider_model(
    app_data_dir: &PathBuf,
    id: &str,
    provider: ProviderKind,
    model: String,
    context_length: Option<u32>,
    llama_fork: Option<String>,
    custom_provider_id: Option<String>,
    external_cli_backend: Option<String>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.provider = provider;
    session.model = model;
    session.context_length = context_length;
    session.llama_fork = llama_fork;
    session.custom_provider_id = custom_provider_id;
    session.external_cli_backend = external_cli_backend;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn update_execution_mode(
    app_data_dir: &PathBuf,
    id: &str,
    execution_mode: ExecutionMode,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.execution_mode = execution_mode;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Fase G: marca `id` como criada por `parent_session_id` — chamado logo
/// depois de `create_session` (mesmo padrão do `folder_id`/T29, que também
/// seta um campo extra num segundo passo em vez de inflar ainda mais a
/// assinatura de `create_session`, já com 8 parâmetros).
pub fn update_parent_session_id(
    app_data_dir: &PathBuf,
    id: &str,
    parent_session_id: Option<String>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.parent_session_id = parent_session_id;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn update_title(app_data_dir: &PathBuf, id: &str, title: String) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.title = title;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Override manual do tamanho de contexto — resolvido automaticamente na
/// criação da sessão (ou troca de provider/modelo), mas providers
/// customizados genéricos quase nunca expõem isso via API, então a sessão
/// fica presa em `DEFAULT_CONTEXT_LENGTH` (8192) até o usuário corrigir aqui
/// (clicando no indicador de contexto na tela). `None` reseta pro
/// comportamento automático de novo.
pub fn update_context_length(
    app_data_dir: &PathBuf,
    id: &str,
    context_length: Option<u32>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.context_length = context_length;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Pastas extras (fora do project_root) que as ferramentas de leitura desta
/// sessao podem acessar via caminho absoluto. Ver campo `extra_read_paths`
/// em `models::Session`.
pub fn update_extra_read_paths(
    app_data_dir: &PathBuf,
    id: &str,
    extra_read_paths: Vec<crate::models::FolderEntry>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.extra_read_paths = extra_read_paths;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Fase D1 do roteiro de Agentes/Skills: troca a pasta de trabalho de uma
/// sessão já existente — antes só dava pra definir `project_root` na
/// criação da sessão (`create_session`), sem jeito de mudar depois.
pub fn update_project_root(
    app_data_dir: &PathBuf,
    id: &str,
    project_root: Option<String>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.project_root = project_root;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn update_reasoning_effort(
    app_data_dir: &PathBuf,
    id: &str,
    effort: Option<ReasoningEffort>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.reasoning_effort = effort;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn update_fable_method(app_data_dir: &PathBuf, id: &str, enabled: bool) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.fable_method = enabled;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Liga/desliga o "Ctx fixo" da sessão e grava o orçamento em tokens
/// (limitado a [1000, 1_000_000] pra não virar zero nem absurdo).
/// Modos da sessão que não valem juntos. A interface já bloqueia o botão; esta é a guarda
/// do backend, para o estado em disco nunca ficar com uma combinação inválida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusiveMode {
    LongHorizon,
    FixedContext,
    TaskQueue,
    PlainQueue,
}

impl ExclusiveMode {
    fn label(self) -> &'static str {
        match self {
            Self::LongHorizon => "Long Horizon",
            Self::FixedContext => "Ctx fixo",
            Self::TaskQueue => "Fila de tarefas",
            Self::PlainQueue => "Fila de tarefas (sem Long Horizon)",
        }
    }

    fn is_on(self, session: &Session) -> bool {
        match self {
            Self::LongHorizon => session.long_horizon.enabled,
            Self::FixedContext => session.fixed_context.enabled,
            Self::TaskQueue => session.task_queue_enabled,
            Self::PlainQueue => session.plain_queue_enabled,
        }
    }

    /// Com quem este modo não pode estar ligado ao mesmo tempo:
    /// - a fila sem Long Horizon não vale com a fila do Long Horizon nem com o próprio Long
    ///   Horizon (as duas juntas mandavam ao modelo as ferramentas e os caminhos do Long
    ///   Horizon numa fila que devia rodar sem ele; visto em 2026-11-11);
    /// - o Ctx fixo é ignorado quando o Long Horizon está ligado (ver `run_turn`).
    fn conflicts(self) -> &'static [ExclusiveMode] {
        match self {
            Self::LongHorizon => &[Self::PlainQueue, Self::FixedContext],
            Self::FixedContext => &[Self::LongHorizon],
            Self::TaskQueue => &[Self::PlainQueue],
            Self::PlainQueue => &[Self::TaskQueue, Self::LongHorizon],
        }
    }
}

/// Erro se `mode` não pode ser LIGADO agora porque um modo incompatível está ligado.
/// Desligar é sempre permitido.
pub fn ensure_can_enable(session: &Session, mode: ExclusiveMode) -> Result<()> {
    if let Some(other) = mode.conflicts().iter().find(|other| other.is_on(session)) {
        anyhow::bail!(
            "\"{}\" não pode ser ligado enquanto \"{}\" estiver ligado. Desligue \"{}\" primeiro.",
            mode.label(),
            other.label(),
            other.label()
        );
    }
    Ok(())
}

#[cfg(test)]
mod exclusive_mode_tests {
    use super::*;
    use crate::models::ProviderKind;

    fn nova_sessao() -> (PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("cerne-exclusivo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let session = create_session(
            &dir, "teste".to_string(), ProviderKind::Ollama, "m".to_string(), None, None, None, None, None,
        )
        .unwrap();
        (dir, session.id)
    }

    #[test]
    fn as_duas_filas_nao_ligam_juntas_em_nenhuma_ordem() {
        let (dir, id) = nova_sessao();
        update_plain_queue_enabled(&dir, &id, true).unwrap();
        assert!(update_task_queue_enabled(&dir, &id, true).is_err());
        assert!(update_long_horizon_enabled(&dir, &id, true).is_err());
        assert!(!get_session(&dir, &id).unwrap().task_queue_enabled);
        // Desligar libera o outro lado.
        update_plain_queue_enabled(&dir, &id, false).unwrap();
        update_long_horizon_enabled(&dir, &id, true).unwrap();
        update_task_queue_enabled(&dir, &id, true).unwrap();
        assert!(update_plain_queue_enabled(&dir, &id, true).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn fila_de_tarefas_depende_do_long_horizon() {
        let (dir, id) = nova_sessao();
        // Sem o Long Horizon a fila não liga.
        assert!(update_task_queue_enabled(&dir, &id, true).is_err());
        update_long_horizon_enabled(&dir, &id, true).unwrap();
        assert!(update_task_queue_enabled(&dir, &id, true).unwrap().task_queue_enabled);
        // Desligar o Long Horizon desliga a fila junto, e isso fica gravado.
        let depois = update_long_horizon_enabled(&dir, &id, false).unwrap();
        assert!(!depois.long_horizon.enabled && !depois.task_queue_enabled);
        assert!(!get_session(&dir, &id).unwrap().task_queue_enabled);
        // Com os dois desligados, a fila sem Long Horizon fica liberada.
        update_plain_queue_enabled(&dir, &id, true).unwrap();
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn ctx_fixo_e_long_horizon_nao_ligam_juntos_mas_sessao_antiga_ainda_ajusta_e_desliga() {
        let (dir, id) = nova_sessao();
        update_fixed_context(&dir, &id, true, 40_000, false, None).unwrap();
        assert!(update_long_horizon_enabled(&dir, &id, true).is_err());
        // O Ctx fixo vale com a fila sem Long Horizon.
        update_plain_queue_enabled(&dir, &id, true).unwrap();
        update_plain_queue_enabled(&dir, &id, false).unwrap();

        // Sessão antiga, gravada com os dois ligados: mudar tokens e desligar continuam possíveis.
        let mut antiga = get_session(&dir, &id).unwrap();
        antiga.long_horizon.enabled = true;
        std::fs::write(session_dir(&dir, &id).join("session.json"), serde_json::to_string_pretty(&antiga).unwrap()).unwrap();
        assert_eq!(update_fixed_context(&dir, &id, true, 20_000, false, None).unwrap().fixed_context.tokens, 20_000);
        update_fixed_context(&dir, &id, false, 20_000, false, None).unwrap();
        assert!(update_fixed_context(&dir, &id, true, 20_000, false, None).is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}

pub fn update_fixed_context(
    app_data_dir: &PathBuf,
    id: &str,
    enabled: bool,
    tokens: u32,
    rolling: bool,
    blocks: Option<bool>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    // Só barra a passagem de desligado para ligado: sessão antiga que já tinha os dois
    // ligados ainda consegue mudar os tokens ou desligar.
    if enabled && !session.fixed_context.enabled {
        ensure_can_enable(&session, ExclusiveMode::FixedContext)?;
    }
    // `blocks` ausente mantém o valor salvo; o modo em blocos e o rolante não valem juntos.
    let blocks = blocks.unwrap_or(session.fixed_context.blocks) && !rolling;
    session.fixed_context = FixedContextState {
        enabled,
        tokens: tokens.clamp(1_000, 1_000_000),
        rolling,
        blocks,
    };
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Resumo rolante do "Ctx fixo": texto e quantas mensagens (a partir do índice
/// 1, depois do sistema) ele já cobre. Fica em `fixed_context_summary.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RollingSummary {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub covered: usize,
}

fn rolling_summary_path(app_data_dir: &PathBuf, id: &str) -> PathBuf {
    session_dir(app_data_dir, id).join("fixed_context_summary.json")
}

pub fn read_rolling_summary(app_data_dir: &PathBuf, id: &str) -> RollingSummary {
    std::fs::read_to_string(rolling_summary_path(app_data_dir, id))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn write_rolling_summary(app_data_dir: &PathBuf, id: &str, summary: &RollingSummary) -> Result<()> {
    write_atomic(&rolling_summary_path(app_data_dir, id), &serde_json::to_string_pretty(summary)?)
}

/// "Janela do modelo" da sessão: o que o Ctx fixo (qualquer modo) já cortou do que é
/// mandado ao modelo. `start` é o índice, em `chat_log.json`, da primeira mensagem que o
/// modelo ainda vê (o prompt de sistema, índice 0, sempre entra); `log_len` é o tamanho do
/// histórico quando foi gravada. Só o que o MODELO recebe muda: `chat_log.json` (o chat que
/// o usuário vê) continua inteiro. Fica em `model_window.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelWindow {
    pub start: usize,
    pub log_len: usize,
}

fn model_window_path(app_data_dir: &PathBuf, id: &str) -> PathBuf {
    session_dir(app_data_dir, id).join("model_window.json")
}

/// `None` quando a sessão nunca usou o Ctx fixo (ou o arquivo está ilegível).
pub fn read_model_window(app_data_dir: &PathBuf, id: &str) -> Option<ModelWindow> {
    std::fs::read_to_string(model_window_path(app_data_dir, id))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

pub fn write_model_window(app_data_dir: &PathBuf, id: &str, window: &ModelWindow) -> Result<()> {
    let path = model_window_path(app_data_dir, id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_atomic(&path, &serde_json::to_string(window)?)
}

pub fn update_long_horizon_enabled(
    app_data_dir: &PathBuf,
    id: &str,
    enabled: bool,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    if enabled && !session.long_horizon.enabled {
        ensure_can_enable(&session, ExclusiveMode::LongHorizon)?;
    }
    session.long_horizon.enabled = enabled;
    if !enabled {
        // A "Fila de tarefas" depende do Long Horizon: sem ele, ela desliga junto. Quem quer
        // fila sem Long Horizon usa a "Fila de tarefas (sem Long Horizon)".
        session.task_queue_enabled = false;
    }
    let dir = session_dir(app_data_dir, id);
    if enabled {
        // Primeira vez que o modo liga nesta sessão: cria a pasta de estado
        // (tarefa 1.3) — memória e projeto começam vazios, o próprio agente
        // preenche ao longo das iterações. Idempotente: se a pasta já existe
        // (modo religado depois de desligado), não sobrescreve o conteúdo.
        let lh_dir = dir.join("long_horizon");
        std::fs::create_dir_all(&lh_dir)?;
        let memoria = lh_dir.join("memoria.md");
        if !memoria.exists() {
            std::fs::write(&memoria, "")?;
        }
        let projeto = lh_dir.join("projeto.md");
        if !projeto.exists() {
            std::fs::write(&projeto, "")?;
        }
    }
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Lê `<sessão>/long_horizon/memoria.md`. String vazia se a sessão nunca
/// ligou o modo (pasta ainda não existe) — não é erro, é "ainda não tem nada".
pub fn read_long_horizon_memoria(app_data_dir: &PathBuf, id: &str) -> Result<String> {
    let path = session_dir(app_data_dir, id).join("long_horizon").join("memoria.md");
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(anyhow::anyhow!("Não foi possível ler {}: {error}", path.display())),
    }
}

pub fn write_long_horizon_memoria(app_data_dir: &PathBuf, id: &str, conteudo: &str) -> Result<()> {
    let dir = session_dir(app_data_dir, id).join("long_horizon");
    std::fs::create_dir_all(&dir)?;
    write_compact_state(&dir.join("memoria.md"), conteudo)
}

pub fn read_long_horizon_projeto(app_data_dir: &PathBuf, id: &str) -> Result<String> {
    let path = session_dir(app_data_dir, id).join("long_horizon").join("projeto.md");
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(anyhow::anyhow!("Não foi possível ler {}: {error}", path.display())),
    }
}

pub fn write_long_horizon_projeto(app_data_dir: &PathBuf, id: &str, conteudo: &str) -> Result<()> {
    let dir = session_dir(app_data_dir, id).join("long_horizon");
    std::fs::create_dir_all(&dir)?;
    write_compact_state(&dir.join("projeto.md"), conteudo)
}

fn write_compact_state(path: &std::path::Path, contents: &str) -> Result<()> {
    anyhow::ensure!(contents.chars().count() <= crate::agent::long_horizon::MAX_STATE_CHARS,
        "Estado grande demais: limite de 12000 caracteres; compacte antes de salvar");
    let old = std::fs::read_to_string(path).unwrap_or_default();
    anyhow::ensure!(!contents.trim().is_empty() || old.trim().is_empty(),
        "Um estado vazio nao pode apagar um registro existente");
    if old.trim() == contents.trim() { return Ok(()); }
    if !old.is_empty() { write_atomic(&path.with_extension("previous.md"), &old)?; }
    write_atomic(path, contents)
}

#[cfg(test)]
mod compact_state_tests {
    use super::*;
    #[test]
    fn duplicate_write_is_noop_and_failed_checkpoint_preserves_state() {
        let dir = std::env::temp_dir().join(format!("cerne-state-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memoria.md");
        write_compact_state(&path, "Java Angular").unwrap();
        write_compact_state(&path, "Java Angular").unwrap();
        assert!(!path.with_extension("previous.md").exists());
        assert!(write_compact_state(&path, "").is_err());
        assert!(write_compact_state(&path, &"x".repeat(12001)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Java Angular");
        write_compact_state(&path, "Java Angular; testes passando").unwrap();
        assert_eq!(std::fs::read_to_string(path.with_extension("previous.md")).unwrap(), "Java Angular");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Java Angular; testes passando");
        std::fs::remove_dir_all(dir).ok();
    }
}

/// Grava via tmp + rename — um crash/kill NO MEIO da escrita deixa o `.tmp`
/// órfão (inofensivo, ignorado na próxima leitura) em vez de truncar o
/// `tarefas.json` de verdade. A fila de tarefas reescreve este arquivo a
/// cada item processado (bem mais vezes que memoria.md/projeto.md), então
/// vale a pena aqui mesmo sem mexer nos outros `write_*` já existentes.
pub(crate) fn write_atomic(path: &std::path::Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn update_task_queue_enabled(app_data_dir: &PathBuf, id: &str, enabled: bool) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    if enabled && !session.task_queue_enabled {
        ensure_can_enable(&session, ExclusiveMode::TaskQueue)?;
        anyhow::ensure!(
            session.long_horizon.enabled,
            "\"Fila de tarefas\" depende do Long Horizon: ligue o Long Horizon primeiro, ou use a \"Fila de tarefas (sem Long Horizon)\"."
        );
    }
    session.task_queue_enabled = enabled;
    let dir = session_dir(app_data_dir, id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn read_task_queue(app_data_dir: &PathBuf, id: &str) -> Result<String> {
    let path = session_dir(app_data_dir, id).join("task_queue").join("tarefas.json");
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(anyhow::anyhow!("Não foi possível ler {}: {error}", path.display())),
    }
}

/// Valida (via `task_queue::parse_tasks`) ANTES de gravar — nunca deixa um
/// JSON quebrado substituir um válido no disco; o chamador (`lib.rs`) devolve
/// o erro de parse pro frontend mostrar direto onde colou.
pub fn write_task_queue(app_data_dir: &PathBuf, id: &str, json_text: &str) -> Result<()> {
    crate::agent::task_queue::parse_tasks(json_text)?;
    let dir = session_dir(app_data_dir, id).join("task_queue");
    std::fs::create_dir_all(&dir)?;
    write_atomic(&dir.join("tarefas.json"), json_text)
}

/// Sobrescreve só o array de tarefas (já validado/mutado em memória pelo
/// loop de execução) — sempre no formato canônico `{"tarefas": [...]}`
/// (ver `task_queue::serialize_tasks`), nunca precisa passar por
/// `write_task_queue`/`parse_tasks` de novo (já veio de um parse válido).
pub fn write_task_queue_tasks(app_data_dir: &PathBuf, id: &str, tasks: &[serde_json::Value]) -> Result<()> {
    let dir = session_dir(app_data_dir, id).join("task_queue");
    std::fs::create_dir_all(&dir)?;
    write_atomic(&dir.join("tarefas.json"), &crate::agent::task_queue::serialize_tasks(tasks))
}

/// Fila sem Long Horizon (`agent::plain_queue`) — mesmo formato de
/// `tarefas.json`, mas numa pasta própria pra não misturar com a fila do LH.
pub fn update_plain_queue_enabled(app_data_dir: &PathBuf, id: &str, enabled: bool) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    if enabled && !session.plain_queue_enabled {
        ensure_can_enable(&session, ExclusiveMode::PlainQueue)?;
    }
    session.plain_queue_enabled = enabled;
    let dir = session_dir(app_data_dir, id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("session.json"), serde_json::to_string_pretty(&session)?)?;
    Ok(session)
}

/// Grava (ou limpa, com `None`/vazio) o caminho do JSON da fila sem Long Horizon.
pub fn update_plain_queue_path(app_data_dir: &PathBuf, id: &str, path: Option<String>) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.plain_queue_path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    let dir = session_dir(app_data_dir, id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("session.json"), serde_json::to_string_pretty(&session)?)?;
    Ok(session)
}

/// Caminho configurado da fila; erro claro quando o usuário ainda não escolheu um.
fn configured_plain_queue_path(app_data_dir: &PathBuf, id: &str) -> Result<PathBuf> {
    let session = get_session(app_data_dir, id)?;
    match session.plain_queue_path {
        Some(path) => Ok(PathBuf::from(path)),
        None => Err(anyhow::anyhow!("Informe o caminho do arquivo JSON da fila.")),
    }
}

/// Lê o JSON da fila no caminho configurado.
pub fn read_plain_queue(app_data_dir: &PathBuf, id: &str) -> Result<String> {
    let path = configured_plain_queue_path(app_data_dir, id)?;
    std::fs::read_to_string(&path)
        .map_err(|error| anyhow::anyhow!("Não foi possível ler {}: {error}", path.display()))
}

/// Regrava o JSON da fila no caminho configurado (usado para marcar `testar`).
pub fn write_plain_queue_tasks(app_data_dir: &PathBuf, id: &str, tasks: &[serde_json::Value]) -> Result<()> {
    let path = configured_plain_queue_path(app_data_dir, id)?;
    write_atomic(&path, &crate::agent::task_queue::serialize_tasks(tasks))
}

/// Memória da sessão (`agent::plain_queue`): fatos desta sessão só. Sem limite
/// de tamanho e nunca tocada pela compactação — vai inteira no system prompt
/// de cada turno, não dentro de `messages`.
pub fn read_session_memory(app_data_dir: &PathBuf, id: &str) -> Result<String> {
    let path = session_dir(app_data_dir, id).join("memoria_sessao.md");
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(anyhow::anyhow!("Não foi possível ler {}: {error}", path.display())),
    }
}

pub fn write_session_memory(app_data_dir: &PathBuf, id: &str, conteudo: &str) -> Result<()> {
    let dir = session_dir(app_data_dir, id);
    std::fs::create_dir_all(&dir)?;
    write_atomic(&dir.join("memoria_sessao.md"), conteudo)
}

/// Teto da memória da sessão quando é o AGENTE quem grava (`update_session_memory`).
/// Ela vai inteira no prompt de todo turno e, a cada reescrita completa, o modelo gera o
/// arquivo todo de novo: uma memória de 30 KB custa uns 4 minutos por gravação num modelo
/// local. A edição manual pelo usuário (`write_session_memory`) não tem esse limite.
pub const SESSION_MEMORY_MAX_BYTES: usize = 12_000;

/// Seção onde o modo "acrescentar" grava. Quando o arquivo passa do limite, as linhas mais
/// antigas DESTA seção são apagadas sozinhas; as demais seções (o mapa do projeto) só mudam
/// quando o modelo manda.
pub const SESSION_MEMORY_LOG_SECTION: &str = "Estado recente";

/// Como o agente quer alterar a memória.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMode {
    /// Reescreve o arquivo inteiro (padrão).
    Replace,
    /// Acrescenta linhas na seção "Estado recente".
    Append,
    /// Troca só o corpo de uma seção `## título`.
    ReplaceSection,
}

impl MemoryMode {
    /// Aceita "acrescentar", "trocar_secao" (também "trocar seção"); qualquer outra coisa,
    /// ou ausente, é "substituir".
    pub fn parse(modo: Option<&str>) -> Self {
        let normal = modo
            .unwrap_or("")
            .trim()
            .to_lowercase()
            .replace('ç', "c")
            .replace('ã', "a")
            .replace(' ', "_");
        match normal.as_str() {
            "acrescentar" => Self::Append,
            "trocar_secao" => Self::ReplaceSection,
            _ => Self::Replace,
        }
    }
}

/// Resultado de uma gravação do agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryUpdate {
    pub bytes: usize,
    /// Linhas antigas de "Estado recente" apagadas para caber no limite.
    pub dropped_lines: usize,
}

/// Título de uma linha `## título` (só nível 2); `None` para qualquer outra linha.
fn memory_heading(line: &str) -> Option<&str> {
    line.trim_end().strip_prefix("## ").map(str::trim)
}

/// Título limpo, sem `#` nem espaços nas pontas.
fn memory_title(secao: &str) -> &str {
    secao.trim().trim_start_matches('#').trim()
}

/// Faixa `[início, fim)` de linhas da seção `titulo` (o início é a linha do título).
fn memory_section(lines: &[&str], titulo: &str) -> Option<(usize, usize)> {
    let start = lines
        .iter()
        .position(|l| memory_heading(l).map_or(false, |t| t.eq_ignore_ascii_case(titulo)))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| memory_heading(l).is_some())
        .map_or(lines.len(), |p| start + 1 + p);
    Some((start, end))
}

/// Troca o corpo da seção `secao`; se ela não existir, cria no fim do arquivo.
fn memory_replace_section(texto: &str, secao: &str, corpo: &str) -> String {
    let titulo = memory_title(secao);
    let mut corpo_linhas: Vec<&str> = corpo.trim().lines().collect();
    // Se o modelo repetiu o título da seção na primeira linha do corpo, ignora.
    if corpo_linhas
        .first()
        .and_then(|l| memory_heading(l))
        .map_or(false, |t| t.eq_ignore_ascii_case(titulo))
    {
        corpo_linhas.remove(0);
    }
    let lines: Vec<&str> = texto.lines().collect();
    let mut out: Vec<String> = Vec::new();
    match memory_section(&lines, titulo) {
        Some((start, end)) => {
            out.extend(lines[..start].iter().map(|l| l.to_string()));
            out.push(lines[start].to_string());
            out.extend(corpo_linhas.iter().map(|l| l.to_string()));
            if end < lines.len() {
                out.push(String::new());
                out.extend(lines[end..].iter().map(|l| l.to_string()));
            }
        }
        None => {
            out.extend(lines.iter().map(|l| l.to_string()));
            if !texto.trim().is_empty() {
                out.push(String::new());
            }
            out.push(format!("## {titulo}"));
            out.extend(corpo_linhas.iter().map(|l| l.to_string()));
        }
    }
    out.join("\n").trim_end().to_string()
}

/// Acrescenta `linhas` no fim da seção "Estado recente" (cria a seção se faltar).
fn memory_append(texto: &str, linhas: &str) -> String {
    let novas: Vec<&str> = linhas.trim().lines().collect();
    let lines: Vec<&str> = texto.lines().collect();
    let mut out: Vec<String> = Vec::new();
    match memory_section(&lines, SESSION_MEMORY_LOG_SECTION) {
        Some((_, end)) => {
            // Entra antes das linhas em branco que separam esta seção da próxima.
            let mut ponto = end;
            while ponto > 0 && lines[ponto - 1].trim().is_empty() {
                ponto -= 1;
            }
            out.extend(lines[..ponto].iter().map(|l| l.to_string()));
            out.extend(novas.iter().map(|l| l.to_string()));
            out.extend(lines[ponto..].iter().map(|l| l.to_string()));
        }
        None => {
            out.extend(lines.iter().map(|l| l.to_string()));
            if !texto.trim().is_empty() {
                out.push(String::new());
            }
            out.push(format!("## {SESSION_MEMORY_LOG_SECTION}"));
            out.extend(novas.iter().map(|l| l.to_string()));
        }
    }
    out.join("\n").trim_end().to_string()
}

/// Apaga as linhas mais antigas de "Estado recente" até o texto caber em `max` bytes, sem
/// tocar nas `protegidas` últimas linhas não vazias da seção (as que acabaram de ser
/// acrescentadas). `None` se não der para caber só apagando linhas antigas. Devolve o texto
/// e quantas linhas foram apagadas.
fn memory_trim_log(texto: &str, max: usize, protegidas: usize) -> Option<(String, usize)> {
    let mut lines: Vec<String> = texto.lines().map(|l| l.to_string()).collect();
    let mut removidas = 0;
    loop {
        let atual = lines.join("\n");
        if atual.len() <= max {
            return Some((atual, removidas));
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let (start, end) = memory_section(&refs, SESSION_MEMORY_LOG_SECTION)?;
        let nao_vazias: Vec<usize> = (start + 1..end).filter(|&i| !lines[i].trim().is_empty()).collect();
        if nao_vazias.len() <= protegidas {
            return None;
        }
        lines.remove(nao_vazias[0]);
        removidas += 1;
    }
}

/// Cabe no limite? Se não, tenta apagar linhas antigas de "Estado recente".
fn memory_fit(novo: String, protegidas: usize) -> Result<(String, usize)> {
    if novo.len() <= SESSION_MEMORY_MAX_BYTES {
        return Ok((novo, 0));
    }
    let tamanho = novo.len();
    memory_trim_log(&novo, SESSION_MEMORY_MAX_BYTES, protegidas).ok_or_else(|| {
        anyhow::anyhow!(
            "a memória ficaria com {tamanho} bytes e o limite é {SESSION_MEMORY_MAX_BYTES}, mesmo apagando as linhas antigas de \"{SESSION_MEMORY_LOG_SECTION}\". Nada foi gravado. Enxugue o que está grande: use modo \"trocar_secao\" (campo `secao`) para resumir uma seção, ou \"substituir\" com a memória COMPLETA mais curta. Mantenha o mapa do projeto compacto e não registre o que o código ou o git já mostram."
        )
    })
}

/// Grava a memória a pedido do agente. Devolve o tamanho final e quantas linhas antigas de
/// "Estado recente" foram apagadas para caber. Recusa (sem gravar nada) o que não couber em
/// [`SESSION_MEMORY_MAX_BYTES`], com uma mensagem que diz ao modelo como resolver.
///
/// - `Replace`: `conteudo` é a memória COMPLETA.
/// - `Append`: `conteudo` são linhas novas; entram em "Estado recente" e, se o arquivo
///   passar do limite, as linhas mais antigas dessa seção somem sozinhas.
/// - `ReplaceSection`: `conteudo` é o novo corpo da seção `secao`; o resto fica como está.
pub fn update_session_memory(
    app_data_dir: &PathBuf,
    id: &str,
    conteudo: &str,
    mode: MemoryMode,
    secao: Option<&str>,
) -> Result<MemoryUpdate> {
    if conteudo.trim().is_empty() {
        return Err(anyhow::anyhow!("conteudo vazio: nada foi gravado"));
    }
    let atual = read_session_memory(app_data_dir, id)?;
    let (novo, dropped_lines) = match mode {
        MemoryMode::Replace => {
            if conteudo.len() > SESSION_MEMORY_MAX_BYTES {
                return Err(anyhow::anyhow!(
                    "a memória ficaria com {} bytes e o limite é {SESSION_MEMORY_MAX_BYTES}. Nada foi gravado. Chame de novo com modo \"substituir\" e a memória COMPLETA mais curta: mantenha só o mapa do projeto, decisões, comandos que funcionaram e erros a não repetir; apague o histórico de tarefas e o que o código ou o git já mostram.",
                    conteudo.len()
                ));
            }
            (conteudo.to_string(), 0)
        }
        MemoryMode::ReplaceSection => {
            let titulo = secao.map(memory_title).filter(|s| !s.is_empty()).ok_or_else(|| {
                anyhow::anyhow!("o modo \"trocar_secao\" precisa do campo `secao` (o título, sem os ##). Nada foi gravado.")
            })?;
            memory_fit(memory_replace_section(&atual, titulo, conteudo), 0)?
        }
        MemoryMode::Append => {
            let novas = conteudo.trim().lines().filter(|l| !l.trim().is_empty()).count();
            memory_fit(memory_append(&atual, conteudo), novas)?
        }
    };
    write_session_memory(app_data_dir, id, &novo)?;
    Ok(MemoryUpdate { bytes: novo.len(), dropped_lines })
}

/// Incrementa `long_horizon.iteracao_atual` — chamado a cada turno em que o
/// modo está ligado (ver `agent::long_horizon::apply_reset`, chamado de
/// `agent::run_turn`). Recarrega a sessão do disco antes de mutar (mesmo
/// cuidado de `accumulate_usage`, que mora logo abaixo): `run_turn` já tem
/// uma cópia de `Session` em memória, mas mutá-la ali não persiste sozinho —
/// só as funções deste arquivo gravam `session.json`.
pub fn advance_long_horizon_iteration(app_data_dir: &PathBuf, id: &str) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.long_horizon.iteracao_atual += 1;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Fecha o turno do modo Long Horizon: registra o desfecho e, quando o
/// turno terminou com uma resposta final de verdade (`nova_resposta`,
/// sem tool calls pendentes — os desfechos "teto_de_ferramenta"/"travou"
/// não têm uma resposta final, então passam `None`), compara com a última
/// resposta registrada pra detectar estagnação (resposta idêntica byte a
/// byte se repetindo). `teto_falha_repetida` some do `desfecho` recebido
/// quando o contador bate o teto — "sucesso" vira "estagnado".
///
/// Chamado UMA vez por turno, depois do laço de chamadas de ferramenta
/// (`agent::run_turn`) — mesmo espírito de `accumulate_usage`: recarrega a
/// sessão do disco antes de mutar, porque `run_turn` já tem uma cópia em
/// memória que mutar sozinha não persiste.
#[allow(clippy::too_many_arguments)]
pub fn finalize_long_horizon_turn(
    app_data_dir: &PathBuf,
    id: &str,
    desfecho: &str,
    nova_resposta: Option<&str>,
    teto_falha_repetida: u32,
    trabalhou_sem_persistir_estado: bool,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;

    let mut desfecho_final = desfecho.to_string();
    if let Some(resposta) = nova_resposta {
        let repetiu = session.long_horizon.ultima_resposta.as_deref() == Some(resposta);
        session.long_horizon.respostas_identicas_seguidas =
            if repetiu { session.long_horizon.respostas_identicas_seguidas + 1 } else { 1 };
        session.long_horizon.ultima_resposta = Some(resposta.to_string());
        if session.long_horizon.respostas_identicas_seguidas >= teto_falha_repetida {
            desfecho_final = "estagnado".to_string();
        }
    }
    session.long_horizon.ultimo_desfecho = Some(desfecho_final);
    session.long_horizon.ultimo_turno_sem_persistir_estado = trabalhou_sem_persistir_estado;

    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn update_persona(app_data_dir: &PathBuf, id: &str, persona_id: Option<String>) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.persona_id = persona_id;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// T29 — move a sessão pra outra pasta da barra lateral (ou pra raiz, com
/// `folder_id: None`).
pub fn update_folder(app_data_dir: &PathBuf, id: &str, folder_id: Option<String>) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.folder_id = folder_id;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Grava o estado do modo Anything da sessão e devolve a sessão atualizada.
pub fn update_anything(app_data_dir: &PathBuf, id: &str, state: crate::models::AnythingState) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.anything = state;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(dir.join("session.json"), serde_json::to_string_pretty(&session)?)?;
    Ok(session)
}

pub fn update_browser_profile(
    app_data_dir: &PathBuf,
    id: &str,
    profile: Option<String>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.browser_profile = profile;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn update_enabled_mcp_servers(
    app_data_dir: &PathBuf,
    id: &str,
    enabled_names: Option<Vec<String>>,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.enabled_mcp_servers = enabled_names;
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

/// Soma o consumo da sessão e **guarda o `prompt_tokens` desta requisição**
/// como o tamanho atual do contexto (`last_prompt_tokens`).
///
/// Os dois são coisas diferentes de propósito: `total_prompt_tokens` é a soma
/// histórica (só pra mostrar consumo acumulado), enquanto `last_prompt_tokens`
/// é o estado ATUAL — é o que o medidor de contexto exibe, porque é o número
/// real que o provider contou do request que acabou de ir.
///
/// `prompt_tokens` só sobrescreve o `last_` quando é maior que zero: provider
/// que não reporta usage em streaming (ou que manda 0 no último chunk) não
/// pode zerar o último valor bom que a gente tinha.
pub fn accumulate_usage(
    app_data_dir: &PathBuf,
    id: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.total_prompt_tokens += prompt_tokens;
    session.total_completion_tokens += completion_tokens;
    session.total_requests += 1;
    if prompt_tokens > 0 {
        session.last_prompt_tokens = Some(prompt_tokens);
    }
    let dir = session_dir(app_data_dir, id);
    std::fs::write(
        dir.join("session.json"),
        serde_json::to_string_pretty(&session)?,
    )?;
    Ok(session)
}

pub fn invalidate_prompt_usage(app_data_dir: &PathBuf, id: &str) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.last_prompt_tokens = None;
    std::fs::write(session_dir(app_data_dir, id).join("session.json"), serde_json::to_vec_pretty(&session)?)?;
    Ok(session)
}

pub fn load_messages(app_data_dir: &PathBuf, id: &str) -> Result<Vec<ChatMessage>> {
    let path = session_dir(app_data_dir, id).join("chat_log.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
        Err(_) => Ok(Vec::new()),
    }
}

/// Carga lenta do histórico (2026-09-20): pega até `limit` mensagens
/// terminando no índice `before` (exclusivo) — `before = None` pega a
/// página mais recente (o fim do histórico). Lê `chat_log.json` inteiro
/// (não há um índice de arquivo por enquanto — o ganho aqui é não mandar
/// tudo pro frontend/renderizar tudo no DOM de uma vez, não evitar a
/// leitura de disco em si), então o índice usado é sempre absoluto e
/// estável entre chamadas: mensagens só são ACRESCENTADAS ao arquivo,
/// nunca reordenadas.
pub fn load_messages_page(
    app_data_dir: &PathBuf,
    id: &str,
    before: Option<usize>,
    limit: usize,
) -> Result<MessagesPage> {
    let all = load_messages(app_data_dir, id)?;
    let end = before.unwrap_or(all.len()).min(all.len());
    let start = end.saturating_sub(limit);
    let user_turns_before = all[..start].iter().filter(|m| m.role == "user").count();
    Ok(MessagesPage {
        messages: all[start..end].to_vec(),
        has_more: start > 0,
        next_before: start,
        user_turns_before,
    })
}

/// Recarrega SEM perder a janela já carregada (2026-09-20): devolve tudo a
/// partir do índice absoluto `since` (inclusive) até o fim — usado quando a
/// sessão já tinha uma janela carregada (via `load_messages_page`) e só
/// precisa sincronizar com o que foi acrescentado desde então (turno
/// terminou, job em segundo plano concluiu, etc.). Diferente de
/// `load_messages_page`, não pagina pra trás — sempre pega até o fim.
pub fn load_messages_since(app_data_dir: &PathBuf, id: &str, since: usize) -> Result<Vec<ChatMessage>> {
    let all = load_messages(app_data_dir, id)?;
    let since = since.min(all.len());
    Ok(all[since..].to_vec())
}

pub fn save_messages(app_data_dir: &PathBuf, id: &str, messages: &[ChatMessage]) -> Result<()> {
    let dir = session_dir(app_data_dir, id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("chat_log.json"),
        serde_json::to_string_pretty(messages)?,
    )?;
    Ok(())
}

pub fn load_tasks(app_data_dir: &PathBuf, id: &str) -> Result<Vec<TaskItem>> {
    let path = session_dir(app_data_dir, id).join("tasks.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
        Err(_) => Ok(Vec::new()),
    }
}

pub fn save_tasks(app_data_dir: &PathBuf, id: &str, tasks: &[TaskItem]) -> Result<()> {
    let dir = session_dir(app_data_dir, id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("tasks.json"), serde_json::to_string_pretty(tasks)?)?;
    Ok(())
}

pub fn delete_session(app_data_dir: &PathBuf, id: &str) -> Result<()> {
    let dir = session_dir(app_data_dir, id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Limpa sessões "zuadas": diretorios de sessao que existem mas nao tem
/// session.json valido, ou que estao com corrompido. Retorna o numero de
/// sessoes removidas.
pub fn cleanup_orphaned_sessions(app_data_dir: &PathBuf) -> Result<u32> {
    let dir = sessions_dir(app_data_dir);
    let mut removed = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries {
            if let Ok(e) = entry {
                if e.path().is_dir() {
                    let session_path = e.path().join("session.json");
                    if !session_path.exists() {
                        // Diretorio sem session.json — remover.
                        if let Err(e) = std::fs::remove_dir_all(e.path()) {
                            eprintln!("[sessions] Erro ao remover sessao orphaned {}: {}", e, e);
                        } else {
                            removed += 1;
                        }
                    } else if let Ok(text) = std::fs::read_to_string(&session_path) {
                        if serde_json::from_str::<Session>(&text).is_err() {
                            // session.json corrompido — remover.
                            if let Err(e) = std::fs::remove_dir_all(e.path()) {
                                eprintln!("[sessions] Erro ao remover sessao corrompida {}: {}", e, e);
                            } else {
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cerne-sessions-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn update_title_persists_the_new_title() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "titulo original".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated = update_title(&dir, &session.id, "titulo novo".to_string()).unwrap();
        assert_eq!(updated.title, "titulo novo");

        let reloaded = get_session(&dir, &session.id).unwrap();
        assert_eq!(reloaded.title, "titulo novo");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_sessions_have_no_parent_session_id_by_default() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao normal".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(session.parent_session_id, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_parent_session_id_persists_and_roundtrips() {
        // Fase G: marca uma sessao como "criada por" outra (orquestrada).
        let dir = scratch_dir();
        let parent = create_session(
            &dir,
            "sessao pai".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let child = create_session(
            &dir,
            "sessao orquestrada".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            update_parent_session_id(&dir, &child.id, Some(parent.id.clone())).unwrap();
        assert_eq!(updated.parent_session_id, Some(parent.id.clone()));

        let reloaded = get_session(&dir, &child.id).unwrap();
        assert_eq!(reloaded.parent_session_id, Some(parent.id));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_session_without_parent_session_id_field_still_deserializes() {
        let dir = scratch_dir();
        let id = uuid::Uuid::new_v4().to_string();
        let dir_path = dir.join("sessions").join(&id);
        std::fs::create_dir_all(&dir_path).unwrap();
        std::fs::write(
            dir_path.join("session.json"),
            format!(
                r#"{{"id":"{id}","title":"antiga","created_at":"2024-01-01T00:00:00Z","provider":"ollama","model":"qwen3.5","project_root":null,"extra_read_paths":[]}}"#
            ),
        )
        .unwrap();
        let loaded = get_session(&dir, &id).unwrap();
        assert_eq!(loaded.parent_session_id, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_sessions_default_to_manual_execution_mode() {
        // Default agora é Manual — toda ferramenta pede aprovação antes de rodar.
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(session.execution_mode, ExecutionMode::Manual);
    }

    #[test]
    fn update_execution_mode_persists_the_new_mode() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated = update_execution_mode(&dir, &session.id, ExecutionMode::Manual).unwrap();
        assert_eq!(updated.execution_mode, ExecutionMode::Manual);

        let reloaded = get_session(&dir, &session.id).unwrap();
        assert_eq!(reloaded.execution_mode, ExecutionMode::Manual);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_context_length_persists_and_can_be_reset_to_automatic() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Custom,
            "qwen3.8-max-preview".to_string(),
            None,
            None,
            None,
            Some("qwen".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(session.context_length, None);

        let updated = update_context_length(&dir, &session.id, Some(131072)).unwrap();
        assert_eq!(updated.context_length, Some(131072));
        assert_eq!(get_session(&dir, &session.id).unwrap().context_length, Some(131072));

        let reset = update_context_length(&dir, &session.id, None).unwrap();
        assert_eq!(reset.context_length, None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn all_new_sessions_default_to_reasoning_off() {
        // Pensamento sempre desligado por padrão para todos os providers.
        let dir = scratch_dir();
        for kind in [
            ProviderKind::LlamaCpp,
            ProviderKind::Ollama,
            ProviderKind::LmStudio,
            ProviderKind::Openrouter,
            ProviderKind::Custom,
        ] {
            let s = create_session(
                &dir,
                "s".to_string(),
                kind,
                "m".to_string(),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            assert_eq!(
                s.reasoning_effort,
                Some(ReasoningEffort::Off),
                "provider {kind:?} deveria default Off"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_sessions_have_long_horizon_disabled_by_default() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao normal".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(!session.long_horizon.enabled);
        assert_eq!(session.long_horizon.iteracao_atual, 0);
        assert_eq!(session.long_horizon.ultimo_desfecho, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sessions_gravadas_antes_do_campo_long_horizon_existir_desserializam_desligadas() {
        // Simula um session.json antigo, gravado antes do campo `long_horizon`
        // existir no struct Session — o #[serde(default)] tem que cobrir isso
        // sem quebrar a leitura (mesmo cuidado que `fable_method`/`folder_id`
        // já tinham).
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao antiga".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let session_dir = dir.join("sessions").join(&session.id);
        let mut valor: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(session_dir.join("session.json")).unwrap())
                .unwrap();
        valor.as_object_mut().unwrap().remove("long_horizon");
        std::fs::write(
            session_dir.join("session.json"),
            serde_json::to_string_pretty(&valor).unwrap(),
        )
        .unwrap();

        let reloaded = get_session(&dir, &session.id).unwrap();
        assert!(!reloaded.long_horizon.enabled);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_long_horizon_enabled_persiste_e_cria_a_pasta_de_estado() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated = update_long_horizon_enabled(&dir, &session.id, true).unwrap();
        assert!(updated.long_horizon.enabled);

        let reloaded = get_session(&dir, &session.id).unwrap();
        assert!(reloaded.long_horizon.enabled);

        let lh_dir = session_dir(&dir, &session.id).join("long_horizon");
        assert!(lh_dir.join("memoria.md").exists());
        assert!(lh_dir.join("projeto.md").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn desligar_e_religar_long_horizon_nao_apaga_o_estado_ja_escrito() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        update_long_horizon_enabled(&dir, &session.id, true).unwrap();

        let lh_dir = session_dir(&dir, &session.id).join("long_horizon");
        std::fs::write(lh_dir.join("memoria.md"), "algo que o agente aprendeu").unwrap();

        update_long_horizon_enabled(&dir, &session.id, false).unwrap();
        let religada = update_long_horizon_enabled(&dir, &session.id, true).unwrap();
        assert!(religada.long_horizon.enabled);

        let conteudo = std::fs::read_to_string(lh_dir.join("memoria.md")).unwrap();
        assert_eq!(conteudo, "algo que o agente aprendeu");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn advance_long_horizon_iteration_incrementa_e_persiste() {
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        update_long_horizon_enabled(&dir, &session.id, true).unwrap();

        let depois_1 = advance_long_horizon_iteration(&dir, &session.id).unwrap();
        assert_eq!(depois_1.long_horizon.iteracao_atual, 1);
        let depois_2 = advance_long_horizon_iteration(&dir, &session.id).unwrap();
        assert_eq!(depois_2.long_horizon.iteracao_atual, 2);

        let reloaded = get_session(&dir, &session.id).unwrap();
        assert_eq!(reloaded.long_horizon.iteracao_atual, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_long_horizon_memoria_e_projeto_vazios_antes_de_ligar_o_modo() {
        // Sessão que nunca ligou o modo: a pasta long_horizon/ nem existe
        // ainda. Ler não deve dar erro — string vazia é o valor certo, não
        // um caso de exceção (o modal de configuração da sessão precisa
        // conseguir abrir mesmo numa sessão que ligou o modo mas ainda não
        // rodou nenhum turno).
        let dir = scratch_dir();
        let session = create_session(
            &dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(read_long_horizon_memoria(&dir, &session.id).unwrap(), "");
        assert_eq!(read_long_horizon_projeto(&dir, &session.id).unwrap(), "");

        std::fs::remove_dir_all(&dir).ok();
    }

    fn sessao_long_horizon(dir: &PathBuf) -> Session {
        let session = create_session(
            dir,
            "sessao".to_string(),
            ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        update_long_horizon_enabled(dir, &session.id, true).unwrap()
    }

    #[test]
    fn finalize_primeira_resposta_registra_sucesso_e_nao_estagna() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);

        let atualizada = finalize_long_horizon_turn(
            &dir,
            &session.id,
            "sucesso",
            Some("primeira resposta"),
            5,
            false,
        )
        .unwrap();

        assert_eq!(atualizada.long_horizon.ultimo_desfecho.as_deref(), Some("sucesso"));
        assert_eq!(atualizada.long_horizon.respostas_identicas_seguidas, 1);
        assert_eq!(atualizada.long_horizon.ultima_resposta.as_deref(), Some("primeira resposta"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_resposta_diferente_reseta_o_contador() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        finalize_long_horizon_turn(&dir, &session.id, "sucesso", Some("resposta A"), 5, false).unwrap();
        finalize_long_horizon_turn(&dir, &session.id, "sucesso", Some("resposta A"), 5, false).unwrap();

        let atualizada = finalize_long_horizon_turn(
            &dir,
            &session.id,
            "sucesso",
            Some("resposta B (diferente)"),
            5,
            false,
        )
        .unwrap();

        assert_eq!(atualizada.long_horizon.respostas_identicas_seguidas, 1);
        assert_eq!(atualizada.long_horizon.ultimo_desfecho.as_deref(), Some("sucesso"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_repeticao_ate_o_teto_vira_estagnado() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        let teto = 3;

        let mut ultima = None;
        for _ in 0..teto {
            ultima = Some(
                finalize_long_horizon_turn(&dir, &session.id, "sucesso", Some("sempre igual"), teto, false)
                    .unwrap(),
            );
        }
        let final_ = ultima.unwrap();

        assert_eq!(final_.long_horizon.respostas_identicas_seguidas, teto);
        assert_eq!(final_.long_horizon.ultimo_desfecho.as_deref(), Some("estagnado"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_com_desfecho_pronto_nao_mexe_no_contador_de_repeticao() {
        // "teto_de_ferramenta"/"travou" nao tem resposta final (o turno foi
        // cortado no meio) -- passar nova_resposta=None preserva o desfecho
        // recebido sem tocar em ultima_resposta/respostas_identicas_seguidas.
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        finalize_long_horizon_turn(&dir, &session.id, "sucesso", Some("resposta real"), 5, false).unwrap();

        let atualizada =
            finalize_long_horizon_turn(&dir, &session.id, "teto_de_ferramenta", None, 5, false).unwrap();

        assert_eq!(atualizada.long_horizon.ultimo_desfecho.as_deref(), Some("teto_de_ferramenta"));
        assert_eq!(atualizada.long_horizon.respostas_identicas_seguidas, 1);
        assert_eq!(atualizada.long_horizon.ultima_resposta.as_deref(), Some("resposta real"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_marca_quando_trabalhou_sem_persistir_estado() {
        // Achado ao vivo (2026-09-20): o modelo disse "vou atualizar
        // projeto.md" na resposta final, sem chamar a ferramenta.
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);

        let atualizada = finalize_long_horizon_turn(
            &dir,
            &session.id,
            "sucesso",
            Some("Agora vou atualizar o projeto.md com o estado real."),
            5,
            true,
        )
        .unwrap();

        assert!(atualizada.long_horizon.ultimo_turno_sem_persistir_estado);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_nao_marca_quando_o_estado_foi_persistido() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);

        let atualizada = finalize_long_horizon_turn(
            &dir,
            &session.id,
            "sucesso",
            Some("Atualizei o projeto.md."),
            5,
            false,
        )
        .unwrap();

        assert!(!atualizada.long_horizon.ultimo_turno_sem_persistir_estado);

        std::fs::remove_dir_all(&dir).ok();
    }

    fn mensagens_numeradas(n: usize) -> Vec<ChatMessage> {
        (0..n)
            .map(|i| ChatMessage {
                role: "user".to_string(),
                content: format!("mensagem {i}"),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: None,
            })
            .collect()
    }

    #[test]
    fn load_messages_page_sem_before_pega_a_pagina_mais_recente() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(10)).unwrap();

        let pagina = load_messages_page(&dir, &session.id, None, 4).unwrap();

        assert_eq!(pagina.messages.len(), 4);
        assert_eq!(pagina.messages[0].content, "mensagem 6");
        assert_eq!(pagina.messages[3].content, "mensagem 9");
        assert!(pagina.has_more);
        assert_eq!(pagina.next_before, 6);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_messages_page_conta_turnos_do_usuario_antes_da_pagina() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        // Helper: todas as 10 mensagens sao do usuario. A pagina (before=6, limit=4)
        // comeca no indice 2, entao 2 turnos vieram antes dela.
        save_messages(&dir, &session.id, &mensagens_numeradas(10)).unwrap();

        let pagina = load_messages_page(&dir, &session.id, Some(6), 4).unwrap();

        assert_eq!(pagina.user_turns_before, 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_messages_page_com_before_pega_a_pagina_anterior() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(10)).unwrap();

        let pagina = load_messages_page(&dir, &session.id, Some(6), 4).unwrap();

        assert_eq!(pagina.messages.len(), 4);
        assert_eq!(pagina.messages[0].content, "mensagem 2");
        assert_eq!(pagina.messages[3].content, "mensagem 5");
        assert!(pagina.has_more);
        assert_eq!(pagina.next_before, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_messages_page_chega_ao_inicio_e_has_more_vira_falso() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(10)).unwrap();

        let pagina = load_messages_page(&dir, &session.id, Some(2), 10).unwrap();

        assert_eq!(pagina.messages.len(), 2);
        assert_eq!(pagina.messages[0].content, "mensagem 0");
        assert!(!pagina.has_more);
        assert_eq!(pagina.next_before, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_messages_page_sessao_curta_cabe_numa_pagina_so() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(3)).unwrap();

        let pagina = load_messages_page(&dir, &session.id, None, 50).unwrap();

        assert_eq!(pagina.messages.len(), 3);
        assert!(!pagina.has_more);
        assert_eq!(pagina.next_before, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_messages_since_devolve_do_indice_ate_o_fim() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(10)).unwrap();

        let novas = load_messages_since(&dir, &session.id, 6).unwrap();

        assert_eq!(novas.len(), 4);
        assert_eq!(novas[0].content, "mensagem 6");
        assert_eq!(novas[3].content, "mensagem 9");
    }

    #[test]
    fn load_messages_since_zero_devolve_tudo() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(5)).unwrap();

        let novas = load_messages_since(&dir, &session.id, 0).unwrap();

        assert_eq!(novas.len(), 5);
    }

    #[test]
    fn load_messages_since_alem_do_fim_devolve_vazio() {
        let dir = scratch_dir();
        let session = sessao_long_horizon(&dir);
        save_messages(&dir, &session.id, &mensagens_numeradas(5)).unwrap();

        let novas = load_messages_since(&dir, &session.id, 999).unwrap();

        assert!(novas.is_empty());
    }
}
