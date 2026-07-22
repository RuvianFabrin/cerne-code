use crate::models::{ChatMessage, ExecutionMode, ProviderKind, Session, TaskItem};
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
        extra_read_paths: Vec::new(),
        execution_mode: ExecutionMode::default(),
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
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.provider = provider;
    session.model = model;
    session.context_length = context_length;
    session.llama_fork = llama_fork;
    session.custom_provider_id = custom_provider_id;
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
    extra_read_paths: Vec<String>,
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

pub fn load_messages(app_data_dir: &PathBuf, id: &str) -> Result<Vec<ChatMessage>> {
    let path = session_dir(app_data_dir, id).join("chat_log.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
        Err(_) => Ok(Vec::new()),
    }
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
        )
        .unwrap();

        let updated = update_title(&dir, &session.id, "titulo novo".to_string()).unwrap();
        assert_eq!(updated.title, "titulo novo");

        let reloaded = get_session(&dir, &session.id).unwrap();
        assert_eq!(reloaded.title, "titulo novo");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_sessions_default_to_auto_execution_mode() {
        // Nao muda o comportamento de quem ja usa o Cerne - so quem trocar
        // explicitamente pelo seletor no composer entra no modo Manual.
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
        )
        .unwrap();
        assert_eq!(session.execution_mode, ExecutionMode::Auto);
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
}
