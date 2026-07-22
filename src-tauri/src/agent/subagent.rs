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
use anyhow::Result;
use std::path::Path;
use tauri::{AppHandle, Emitter};

/// Passos de ferramenta que o sub-agente pode dar antes de ser forcado a
/// parar e devolver o que tiver — bem menor que o `MAX_AGENTIC_STEPS` (12) da
/// sessao principal, ja que e uma sub-tarefa deliberadamente mais estreita.
const MAX_SUBAGENT_STEPS: usize = 8;

const SUBAGENT_SYSTEM_PROMPT: &str = "Voce e um sub-agente descartavel, delegado por um agente \
principal do Cerne pra resolver UMA sub-tarefa especifica e bem definida. Use as ferramentas \
disponiveis pra investigar e resolver a tarefa de verdade - nunca alegue ter feito algo sem ter \
chamado a ferramenta correspondente. write_file/edit_file/ast_edit escrevem numa sandbox e \
dependem do usuario aceitar o diff na interface - deixe isso claro no seu relatorio se editou \
algo. Quando terminar (ou concluir que nao da pra resolver), responda com um RELATORIO FINAL \
conciso do que foi feito/encontrado - o agente principal so ve essa resposta final, nao os seus \
passos intermediarios, entao inclua tudo que ele precisa saber pra continuar. Voce NAO tem a \
ferramenta 'task' - nao pode delegar pra outro sub-agente.";

/// Roda o sub-agente ate ele parar de chamar ferramentas (ou bater o limite
/// de passos) e devolve o texto da resposta final dele.
///
/// `session_id` e reusado (nao criado um novo) so pra emitir os eventos de UI
/// (`agent:tool_call`, `agent:pending_edit`) na mesma sessao que o usuario ja
/// esta olhando — o sub-agente nao tem sessao/historico persistido proprio,
/// sua conversa e efemera (existe so na memoria desta chamada).
#[allow(clippy::too_many_arguments)]
pub async fn run(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    project_root: &Path,
    extra_read_paths: &[String],
    description: &str,
    task_prompt: &str,
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
    let mcp_servers = crate::mcp::load_servers(&state.app_data_dir).unwrap_or_default();
    tool_specs.extend(state.mcp_clients.tool_specs(&mcp_servers).await);
    let mut recent_calls: Vec<(String, String)> = Vec::new();

    for step in 0..MAX_SUBAGENT_STEPS {
        let assistant = providers::chat_stream(
            app,
            session_id,
            cfg,
            api_key.clone(),
            model,
            &messages,
            &tool_specs,
        )
        .await?;
        let has_tool_calls = assistant
            .tool_calls
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);
        messages.push(assistant.clone());

        if !has_tool_calls {
            return Ok(assistant.content);
        }

        for call in assistant.tool_calls.iter().flatten() {
            let args: serde_json::Value =
                serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);

            let _ = app.emit(
                "agent:tool_call",
                super::ToolCallEvent {
                    session_id: session_id.to_string(),
                    tool: format!("↳ sub-agente ({description}): {}", call.function.name),
                    args: call.function.arguments.clone(),
                },
            );

            let result = tools::execute_tool(
                &call.function.name,
                &args,
                Some(project_root),
                extra_read_paths,
                &state.background_jobs,
                &state.mcp_clients,
                &state.app_data_dir,
            )
            .await;
            let observation = match &result {
                Ok(outcome) => outcome.observation.clone(),
                Err(e) => format!("erro executando ferramenta: {e}"),
            };

            if let Ok(outcome) = &result {
                if let Some((target_path, sandbox_path, diff, is_new_file)) = &outcome.pending_edit
                {
                    let edit = PendingEdit {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.to_string(),
                        target_path: target_path.clone(),
                        sandbox_path: sandbox_path.clone(),
                        diff: diff.clone(),
                        is_new_file: *is_new_file,
                    };
                    state
                        .pending_edits
                        .lock()
                        .unwrap()
                        .insert(edit.id.clone(), edit.clone());
                    let _ = app.emit("agent:pending_edit", edit);
                }
            }

            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: observation,
                tool_calls: None,
                tool_call_id: Some(call.id.clone()),
                name: Some(call.function.name.clone()),
                images: Vec::new(),
                display_content: None,
            });

            recent_calls.push((call.function.name.clone(), call.function.arguments.clone()));
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

/// Guarda de profundidade: mesmo toolset da sessao pai, menos `task` (nao
/// pode recursivamente delegar) e `verify_completion` (verificacao fica a
/// cargo do agente principal, nao de cada sub-tarefa).
fn subagent_tool_specs() -> Vec<crate::models::ToolSpec> {
    tools::project_tool_specs()
        .into_iter()
        .filter(|t| !matches!(t.function.name.as_str(), "task" | "verify_completion"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_toolset_excludes_task_but_keeps_everything_else() {
        let parent_specs = tools::project_tool_specs();
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
        assert_eq!(
            sub_names.len(),
            parent_names.len() - 2,
            "deveria ser o toolset do pai inteiro, menos task e verify_completion"
        );
        for name in &parent_names {
            if *name != "task" && *name != "verify_completion" {
                assert!(
                    sub_names.contains(name),
                    "sub-agente deveria manter a ferramenta '{name}' da sessao pai"
                );
            }
        }
    }
}
