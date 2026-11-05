//! Subagentes (`task`): delega uma sub-tarefa bem definida pra um agente
//! descartavel que roda seu proprio loop de ferramentas e devolve so o
//! relatorio final — os passos intermediarios dele nao poluem o historico
//! da conversa principal. Padrao confirmado em duas fontes independentes
//! (grok-build e opencode, ver `agent-architecture-research.md` seção 2.3.2/
//! 6.7): guarda de profundidade que bloqueia recursao por padrao, execucao
//! usando o mesmo provider/modelo/projeto da sessao pai.
//!
//! Reduzido em relacao ao opencode/grok-build de proposito: sem execucao em
//! background com notificacao (roda sincrono, bloqueando o turno do agente
//! pai ate terminar — mais simples, e o Cerne ja tem "voce sera notificado"
//! pra outras coisas assincronas como `check_background_output`, nao
//! precisava duplicar esse mecanismo aqui tambem); sem isolamento de
//! filesystem por sub-agente (usa a mesma sandbox da sessao pai — o
//! `pi-iso`/git-worktree fica pra quando o Cerne precisar rodar tasks
//! concorrentes de verdade, ver item 6 da lista de porte).
//!
//! **Guarda de profundidade**: a unica restricao que realmente importa (ver
//! achado do opencode, `depth >= 1` bloqueia recursao por padrao) — o
//! sub-agente recebe o `project_tool_specs()` inteiro MENOS a propria tool
//! `task`, entao ele estruturalmente nao pode delegar pra outro sub-agente.
//! Fora isso, tem acesso as mesmas ferramentas da sessao pai (ler/buscar/
//! editar arquivo, rodar comando) — restringir mais que isso tiraria valor
//! real de delegacao sem ganho de seguranca adicional (a sandbox de edicao
//! ja protege o arquivo real de qualquer jeito).

use super::tools;
use crate::models::{ChatMessage, PendingEdit, ProviderConfig};
use crate::{providers, AppState};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Passos de ferramenta que o sub-agente pode dar antes de ser forcado a
/// parar e devolver o que tiver — bem menor que o `MAX_AGENTIC_STEPS` (12) da
/// sessao principal, ja que e uma sub-tarefa deliberadamente mais estreita.
const MAX_SUBAGENT_STEPS: usize = 8;

const SUBAGENT_SYSTEM_PROMPT: &str = "Voce e um sub-agente descartavel, delegado por um agente \
principal do Cerne pra resolver UMA sub-tarefa especifica e bem definida. Use as ferramentas \
disponiveis pra investigar e resolver a tarefa de verdade - nunca alegue ter feito algo sem ter \
chamado a ferramenta correspondente. write_file/edit_file/ast_edit podem escrever direto no \
arquivo real OU numa sandbox pendente de aceite, dependendo do modo de execucao da sessao - o \
resultado de CADA chamada informa qual foi o caso ('escrito diretamente' vs 'ainda NAO aplicada, \
precisa ser aceita na interface'); confie nesse texto, nao presuma um ou outro, e deixe claro no \
seu relatorio final qual aconteceu se voce editou algo. Quando terminar (ou concluir que nao da \
pra resolver), responda com um RELATORIO FINAL conciso do que foi feito/encontrado - o agente \
principal so ve essa resposta final, nao os seus passos intermediarios, entao inclua tudo que ele \
precisa saber pra continuar. Voce NAO tem a ferramenta 'task' - nao pode delegar pra outro \
sub-agente.";

/// Copia as mensagens pendentes da caixa de entrada pro historico como turnos
/// do usuario. Devolve true se entrou alguma.
fn drain_inbox(inbox: &mut Option<&mut UnboundedReceiver<String>>, messages: &mut Vec<ChatMessage>) -> bool {
    let mut added = false;
    if let Some(rx) = inbox.as_mut() {
        while let Ok(text) = rx.try_recv() {
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: text,
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: None,
            });
            added = true;
        }
    }
    added
}

/// Roda o sub-agente ate ele parar de chamar ferramentas (ou bater o limite
/// de passos) e devolve o texto da resposta final dele.
///
/// `session_id` e reusado (nao criado um novo) so pra emitir os eventos de UI
/// (`agent:tool_call`, `agent:pending_edit`) na mesma sessao que o usuario ja
/// esta olhando — o sub-agente nao tem sessao/historico persistido proprio,
/// sua conversa e efemera (existe so na memoria desta chamada).
#[allow(clippy::too_many_arguments)]
/// Roda o sub-agente sem caixa de entrada (chamada sincrona).
pub async fn run(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    execution_id: &str,
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    project_root: &Path,
    extra_read_paths: &[String],
    description: &str,
    task_prompt: &str,
    enabled_mcp_servers: Option<&[String]>,
    execution_mode: &crate::models::ExecutionMode,
) -> Result<String> {
    run_with_inbox(app, state, session_id, execution_id, cfg, api_key, model, project_root, extra_read_paths, description, task_prompt, enabled_mcp_servers, execution_mode, None).await
}

/// Roda o sub-agente; `inbox` recebe mensagens do agente pai enquanto ele roda.
pub async fn run_with_inbox(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    execution_id: &str,
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    project_root: &Path,
    extra_read_paths: &[String],
    description: &str,
    task_prompt: &str,
    enabled_mcp_servers: Option<&[String]>,
    execution_mode: &crate::models::ExecutionMode,
    mut inbox: Option<&mut UnboundedReceiver<String>>,
) -> Result<String> {
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: SUBAGENT_SYSTEM_PROMPT.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: task_prompt.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
    ];

    let mut tool_specs = subagent_tool_specs();
    // T13: respeita o mesmo filtro por sessao que o loop principal usa
    // (agent/mod.rs) - antes o sub-agente via TODOS os MCPs configurados
    // globalmente, ignorando o toggle por sessao do composer.
    // O sub-agente usa o mesmo navegador da sessao pai: o perfil ja vem
    // resolvido em enabled_mcp_servers (ver session_mcp_servers no mod.rs).
    let mcp_servers = crate::mcp::session_mcp_servers(&state.app_data_dir, enabled_mcp_servers, None)
        .unwrap_or_default();
    tool_specs.extend(state.mcp_clients.tool_specs(&mcp_servers).await);
    let mut recent_calls: Vec<(String, String)> = Vec::new();

    // Canal sintetico (mesmo padrao do `maybe_compact` em mod.rs): o texto/
    // thinking que o sub-agente gera nao pode ir no `chat:token` da sessao
    // real, senao vaza e se mistura com o streaming do agente principal no
    // chat visivel do usuario - o sub-agente e descartavel, so o relatorio
    // final dele importa pra fora desta funcao (ver doc do modulo).
    let stream_channel = format!("{session_id}::exec::{execution_id}");

    for step in 0..MAX_SUBAGENT_STEPS {
        // Mensagens enviadas pelo agente pai (send_subagent_message) entram antes do proximo passo.
        drain_inbox(&mut inbox, &mut messages);
        let assistant = providers::chat_stream(
            app,
            &stream_channel,
            cfg,
            api_key.clone(),
            model,
            &messages,
            &tool_specs,
            // Sub-agente é chamada utilitária: em locais força Off (senão
            // pensa à toa); em cloud deixa Auto pra não mandar campos que um
            // backend OpenAI estrito rejeitaria.
            cfg.kind.default_reasoning_effort(),
            None,
        )
        .await?
        .message;
        let has_tool_calls = assistant
            .tool_calls
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);
        messages.push(assistant.clone());

        if !has_tool_calls {
            // Se o pai mandou algo enquanto isso, continua em vez de encerrar.
            if drain_inbox(&mut inbox, &mut messages) {
                continue;
            }
            return Ok(assistant.content);
        }

        for call in assistant.tool_calls.iter().flatten() {
            let args: serde_json::Value =
                serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);
            let file_path = super::extract_file_path(&call.function.name, &args);
            let command = super::extract_command_text(&call.function.name, &args);
            let step_started = std::time::Instant::now();

            let _ = app.emit(
                "agent:tool_call",
                super::ToolCallEvent {
                    session_id: session_id.to_string(),
                    id: call.id.clone(),
                    tool: format!("↳ sub-agente ({description}): {}", call.function.name),
                    args: call.function.arguments.clone(),
                    command: command.clone(),
                    file_path: file_path.clone(),
                    execution_id: Some(execution_id.to_string()),
                },
            );
            super::record_execution_step(
                state,
                execution_id,
                crate::models::TaskItem {
                    id: call.id.clone(),
                    label: format!("{}({})", call.function.name, super::truncate(&call.function.arguments, 80)),
                    status: "running".to_string(),
                    detail: None,
                    turn: 0,
                    file_path,
                    additions: 0,
                    deletions: 0,
                    started_at_ms: chrono::Utc::now().timestamp_millis() as u64,
                    duration_ms: None,
                    command,
                    execution_id: None,
                    images: Vec::new(),
                },
            );

            let folder_entries: Vec<crate::models::FolderEntry> = extra_read_paths
                .iter()
                .map(|p| crate::models::FolderEntry { path: p.clone(), mode: crate::models::FolderMode::Read })
                .collect();
            // Ferramentas de chat/skills precisam de AppHandle; o sub-agente as
            // executa aqui, com a mesma logica que o agente principal usa.
            let result = if super::chat_tools::CHAT_TOOL_NAMES.contains(&call.function.name.as_str()) {
                super::chat_tools::run(
                    &call.function.name,
                    app,
                    session_id,
                    Some(project_root),
                    &state.app_data_dir,
                    &args,
                )
            } else {
                tools::execute_tool(
                    &call.function.name,
                    &args,
                    Some(project_root),
                    &folder_entries,
                    &state.background_jobs,
                    &state.mcp_clients,
                    &state.app_data_dir,
                    execution_mode,
                    session_id,
                )
                .await
            };
            let observation = match &result {
                Ok(outcome) => outcome.observation.clone(),
                Err(e) => format!("erro executando ferramenta: {e}"),
            };

            let mut additions = 0u32;
            let mut deletions = 0u32;
            if let Ok(outcome) = &result {
                if let Some((target_path, sandbox_path, diff, is_new_file, already_applied)) = &outcome.pending_edit
                {
                    (additions, deletions) = super::count_diff_stats(diff);
                    let edit = PendingEdit {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.to_string(),
                        target_path: target_path.clone(),
                        sandbox_path: sandbox_path.clone(),
                        diff: diff.clone(),
                        is_new_file: *is_new_file,
                        already_applied: *already_applied,
                    };
                    // Mesmo tratamento do loop principal (agent/mod.rs): em
                    // YOLO (`already_applied`) o arquivo real ja foi escrito,
                    // nao ha accept/reject pendente — so invalida o cache, sem
                    // inserir em `pending_edits` (senao fica "zumbi" la pra
                    // sempre, sem ninguem pra tirar).
                    if *already_applied {
                        super::walk_cache::invalidate(std::path::Path::new(target_path));
                    } else {
                        state
                            .pending_edits
                            .lock()
                            .unwrap()
                            .insert(edit.id.clone(), edit.clone());
                    }
                    let _ = app.emit("agent:pending_edit", edit);
                }
            }

            let result_status = if result.is_ok() { "done" } else { "failed" };
            let result_detail = super::truncate(&observation, 6000);
            let elapsed_ms = step_started.elapsed().as_millis() as u64;
            let _ = app.emit(
                "agent:tool_result",
                super::ToolResultEvent {
                    session_id: session_id.to_string(),
                    id: call.id.clone(),
                    status: result_status.to_string(),
                    detail: Some(result_detail.clone()),
                    additions,
                    deletions,
                    duration_ms: Some(elapsed_ms),
                    execution_id: Some(execution_id.to_string()),
                    images: Vec::new(),
                },
            );
            super::update_execution_step(
                state,
                execution_id,
                &call.id,
                result_status,
                Some(result_detail),
                additions,
                deletions,
                elapsed_ms,
            );

            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: observation,
                tool_calls: None,
                tool_call_id: Some(call.id.clone()),
                name: Some(call.function.name.clone()),
                images: Vec::new(),
                display_content: None,
            });

            if !super::DOOM_LOOP_EXEMPT_TOOLS.contains(&call.function.name.as_str()) {
                recent_calls.push((call.function.name.clone(), call.function.arguments.clone()));
            }
            if super::is_doom_loop(&recent_calls) {
                return Ok(format!(
                    "[sub-agente parou: chamou '{}' {} vezes seguidas com os mesmos argumentos, sem \
                     sinal de progresso - parece um loop]",
                    call.function.name,
                    super::DOOM_LOOP_THRESHOLD
                ));
            }
        }

        if step + 1 == MAX_SUBAGENT_STEPS {
            break;
        }
    }

    let last_text = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && !m.content.is_empty())
        .map(|m| m.content.clone())
        .unwrap_or_default();
    Ok(format!(
        "[sub-agente atingiu o limite de {MAX_SUBAGENT_STEPS} passos sem concluir - relatorio parcial abaixo]\n{last_text}"
    ))
}

/// Guarda de profundidade: mesmo toolset da sessao pai (`always_tool_specs`
/// + `project_tool_specs`, igual `run_turn` monta em `agent/mod.rs:629-631`),
/// menos `task` (nao pode recursivamente delegar), `verify_completion`
/// (verificacao fica a cargo do agente principal, nao de cada sub-tarefa),
/// `run_pipeline` (Fase 3 — o sub-agente E a etapa DEV de um pipeline; nao
/// pode disparar outro pipeline de dentro de si mesmo) e `ask` (sub-agente
/// nao pode pausar esperando o usuario - travaria a fila).
///
/// Bug corrigido (2026-08-16): essa funcao so pegava `project_tool_specs()`,
/// esquecendo `always_tool_specs()` inteiro - na pratica isso deixava o
/// sub-agente sem `web_search`/`web_fetch`/`load_skill`/`todo_list` SEMPRE,
/// mesmo com pasta de projeto configurada (nao era um problema de pasta,
/// como se poderia supor - a funcao simplesmente nunca incluia essas
/// ferramentas).
fn subagent_tool_specs() -> Vec<crate::models::ToolSpec> {
    let mut specs: Vec<crate::models::ToolSpec> = tools::always_tool_specs()
        .into_iter()
        .filter(|t| t.function.name != "ask")
        .collect();
    specs.extend(tools::project_tool_specs().into_iter().filter(|t| {
        !matches!(
            t.function.name.as_str(),
            "task" | "verify_completion" | "run_pipeline"
        )
    }));
    specs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_toolset_excludes_task_but_keeps_everything_else() {
        // Toolset real da sessao pai (run_turn monta assim, agent/mod.rs:629-631):
        // always_tool_specs() + project_tool_specs().
        let mut parent_specs = tools::always_tool_specs();
        parent_specs.extend(tools::project_tool_specs());
        let parent_names: Vec<&str> = parent_specs
            .iter()
            .map(|t| t.function.name.as_str())
            .collect();
        assert!(
            parent_names.contains(&"task"),
            "sanity check: a sessao pai deveria ter a tool task"
        );
        assert!(
            parent_names.contains(&"verify_completion"),
            "sanity check: a sessao pai deveria ter verify_completion"
        );
        assert!(
            parent_names.contains(&"web_search"),
            "sanity check: a sessao pai deveria ter web_search"
        );

        let sub_specs = subagent_tool_specs();
        let sub_names: Vec<&str> = sub_specs.iter().map(|t| t.function.name.as_str()).collect();
        assert!(
            !sub_names.contains(&"task"),
            "sub-agente nao pode ter acesso a propria tool task (guarda de profundidade)"
        );
        assert!(
            !sub_names.contains(&"verify_completion"),
            "sub-agente nao devia poder disparar verificacao (fica a cargo do agente principal)"
        );
        assert!(
            !sub_names.contains(&"run_pipeline"),
            "sub-agente (etapa DEV de um pipeline) nao pode disparar outro pipeline"
        );
        assert!(
            !sub_names.contains(&"ask"),
            "sub-agente nao pode pausar esperando o usuario (travaria a fila)"
        );
        // Regressao do bug corrigido em 2026-08-16: subagent_tool_specs()
        // esquecia always_tool_specs() inteiro, entao o sub-agente nunca
        // tinha web_search/web_fetch/load_skill/todo_list.
        for name in ["web_search", "web_fetch", "load_skill", "todo_list"] {
            assert!(
                sub_names.contains(&name),
                "sub-agente deveria ter a ferramenta '{name}' (vem de always_tool_specs)"
            );
        }
        assert_eq!(
            sub_names.len(),
            parent_names.len() - 4,
            "deveria ser o toolset do pai inteiro, menos task, verify_completion, run_pipeline e ask"
        );
        for name in &parent_names {
            if !matches!(*name, "task" | "verify_completion" | "run_pipeline" | "ask") {
                assert!(
                    sub_names.contains(name),
                    "sub-agente deveria manter a ferramenta '{name}' da sessao pai"
                );
            }
        }
    }
}


// ---------------------------------------------------------------------------
// Sub-agentes em segundo plano (task com background=true).
//
// O agente pai nao espera: recebe um id, consulta com check_subagent e manda
// instrucoes extras com send_subagent_message. Cada um roda numa tarefa do
// Tokio com a sua caixa de entrada; o loop le a caixa entre os passos.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundStatus {
    Running,
    Done(String),
    Failed(String),
}

struct BackgroundEntry {
    id: String,
    parent_session_id: String,
    description: String,
    inbox: UnboundedSender<String>,
    status: BackgroundStatus,
}

static BACKGROUND: Mutex<Vec<BackgroundEntry>> = Mutex::new(Vec::new());

/// Tudo que o sub-agente precisa, com dono (vai pra outra tarefa).
pub struct BackgroundSpec {
    pub parent_session_id: String,
    pub execution_id: String,
    pub cfg: ProviderConfig,
    pub api_key: Option<String>,
    pub model: String,
    pub project_root: PathBuf,
    pub extra_read_paths: Vec<String>,
    pub description: String,
    pub prompt: String,
    pub enabled_mcp_servers: Option<Vec<String>>,
    pub execution_mode: crate::models::ExecutionMode,
}

/// Inicia o sub-agente em segundo plano e devolve o id dele.
pub fn spawn_background(app: AppHandle, spec: BackgroundSpec) -> String {
    let id = format!("sub-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if let Ok(mut list) = BACKGROUND.lock() {
        list.push(BackgroundEntry {
            id: id.clone(),
            parent_session_id: spec.parent_session_id.clone(),
            description: spec.description.clone(),
            inbox: tx,
            status: BackgroundStatus::Running,
        });
    }
    let task_id = id.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let result = run_with_inbox(
            &app,
            &state,
            &spec.parent_session_id,
            &spec.execution_id,
            &spec.cfg,
            spec.api_key.clone(),
            &spec.model,
            &spec.project_root,
            &spec.extra_read_paths,
            &spec.description,
            &spec.prompt,
            spec.enabled_mcp_servers.as_deref(),
            &spec.execution_mode,
            Some(&mut rx),
        )
        .await;
        super::finish_agent_execution(&state, &spec.execution_id, result.is_ok());
        let status = match result {
            Ok(report) => BackgroundStatus::Done(report),
            Err(e) => BackgroundStatus::Failed(e.to_string()),
        };
        set_status(&task_id, status);
    });
    id
}

fn set_status(id: &str, status: BackgroundStatus) {
    if let Ok(mut list) = BACKGROUND.lock() {
        if let Some(entry) = list.iter_mut().find(|e| e.id == id) {
            entry.status = status;
        }
    }
}

/// Manda uma mensagem pra um sub-agente que ainda esta rodando desta sessao.
pub fn send_to_background(id: &str, parent_session_id: &str, message: &str) -> Result<String> {
    let list = BACKGROUND.lock().map_err(|_| anyhow!("registro de sub-agentes indisponivel"))?;
    let entry = list
        .iter()
        .find(|e| e.id == id && e.parent_session_id == parent_session_id)
        .ok_or_else(|| anyhow!("sub-agente {id} nao encontrado nesta sessao"))?;
    if entry.status != BackgroundStatus::Running {
        return Err(anyhow!("sub-agente {id} ja terminou; inicie outro com task"));
    }
    entry
        .inbox
        .send(message.to_string())
        .map_err(|_| anyhow!("sub-agente {id} nao esta mais aceitando mensagens"))?;
    Ok(format!("Mensagem enviada ao sub-agente {id} ({}). Use check_subagent depois.", entry.description))
}

/// Situacao atual de um sub-agente desta sessao, em texto pro modelo.
pub fn check_background(id: &str, parent_session_id: &str) -> Result<String> {
    let list = BACKGROUND.lock().map_err(|_| anyhow!("registro de sub-agentes indisponivel"))?;
    let entry = list
        .iter()
        .find(|e| e.id == id && e.parent_session_id == parent_session_id)
        .ok_or_else(|| anyhow!("sub-agente {id} nao encontrado nesta sessao"))?;
    Ok(match &entry.status {
        BackgroundStatus::Running => format!("Sub-agente {id} ({}) ainda esta rodando.", entry.description),
        BackgroundStatus::Done(report) => report.clone(),
        BackgroundStatus::Failed(error) => format!("Sub-agente {id} falhou: {error}"),
    })
}

#[cfg(test)]
mod background_tests {
    use super::*;

    #[test]
    fn drain_inbox_moves_pending_messages_once() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        tx.send("primeira".to_string()).unwrap();
        tx.send("segunda".to_string()).unwrap();
        let mut inbox = Some(&mut rx);
        let mut messages: Vec<ChatMessage> = Vec::new();
        assert!(drain_inbox(&mut inbox, &mut messages));
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "segunda");
        assert!(!drain_inbox(&mut inbox, &mut messages));
    }

    #[test]
    fn unknown_background_id_is_an_error() {
        assert!(check_background("sub-inexistente", "sessao-x").is_err());
        assert!(send_to_background("sub-inexistente", "sessao-x", "oi").is_err());
    }
}
