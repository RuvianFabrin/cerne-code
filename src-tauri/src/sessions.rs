use crate::models::{
    ChatMessage, ExecutionMode, LongHorizonState, MessagesPage, ProviderKind, ReasoningEffort,
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
        task_queue_enabled: false,
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

pub fn update_long_horizon_enabled(
    app_data_dir: &PathBuf,
    id: &str,
    enabled: bool,
) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
    session.long_horizon.enabled = enabled;
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
    Ok(std::fs::read_to_string(path).unwrap_or_default())
}

pub fn write_long_horizon_memoria(app_data_dir: &PathBuf, id: &str, conteudo: &str) -> Result<()> {
    let dir = session_dir(app_data_dir, id).join("long_horizon");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("memoria.md"), conteudo)?;
    Ok(())
}

pub fn read_long_horizon_projeto(app_data_dir: &PathBuf, id: &str) -> Result<String> {
    let path = session_dir(app_data_dir, id).join("long_horizon").join("projeto.md");
    Ok(std::fs::read_to_string(path).unwrap_or_default())
}

pub fn write_long_horizon_projeto(app_data_dir: &PathBuf, id: &str, conteudo: &str) -> Result<()> {
    let dir = session_dir(app_data_dir, id).join("long_horizon");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("projeto.md"), conteudo)?;
    Ok(())
}

/// Grava via tmp + rename — um crash/kill NO MEIO da escrita deixa o `.tmp`
/// órfão (inofensivo, ignorado na próxima leitura) em vez de truncar o
/// `tarefas.json` de verdade. A fila de tarefas reescreve este arquivo a
/// cada item processado (bem mais vezes que memoria.md/projeto.md), então
/// vale a pena aqui mesmo sem mexer nos outros `write_*` já existentes.
fn write_atomic(path: &std::path::Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn update_task_queue_enabled(app_data_dir: &PathBuf, id: &str, enabled: bool) -> Result<Session> {
    let mut session = get_session(app_data_dir, id)?;
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
    Ok(std::fs::read_to_string(path).unwrap_or_default())
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
    Ok(MessagesPage {
        messages: all[start..end].to_vec(),
        has_more: start > 0,
        next_before: start,
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
