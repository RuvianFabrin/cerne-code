//! Etapa ANALISTA do pipeline Dev → QA → Analista (a última etapa do
//! ciclo de qualidade).
//!
//! Estruturalmente é uma cópia de `verifier.rs` (mesmo toolset read-only,
//! mesmo formato de veredito APROVADO/REFUTADO + evidência) — decisão de
//! design deliberada: generalizar `verifier.rs` com um parâmetro `role` era
//! a alternativa mais "DRY", mas arriscaria um módulo já testado e usado em
//! produção (Fase A2/T28) por causa de uma feature nova. Duplicar ~80 linhas
//! é mais seguro.
//!
//! **A diferença real não é o mecanismo, é O QUE cada um audita**: o QA
//! (`verifier.rs`) confirma se a implementação FUNCIONA tecnicamente (roda
//! teste/build de verdade). O Analista aqui confirma se ela satisfaz o
//! PEDIDO ORIGINAL do usuário — pode aprovar um código que roda mas não faz
//! o que foi pedido, ou vice-versa. Por isso o Analista recebe o requisito
//! ORIGINAL (não só o relatório do DEV) e não tem ênfase em rodar comando —
//! o foco dele é ler e comparar, não validar tecnicamente (isso já é
//! trabalho do QA, que roda antes na sequência do pipeline).

use super::tools;
use crate::models::{ChatMessage, ProviderConfig, ToolSpec};
use crate::{providers, AppState};
use anyhow::Result;
use std::path::Path;
use tauri::{AppHandle, Emitter};

const MAX_ANALYST_STEPS: usize = 8;

/// Mesmo allowlist do QA — só observar, nunca consertar (isso cabe ao DEV,
/// numa próxima rodada do pipeline se o Analista refutar).
const ANALYST_ALLOWED_TOOLS: &[&str] =
    &["read_file", "list_dir", "grep", "ast_grep", "run_command"];

const ANALYST_SYSTEM_PROMPT: &str = "Voce e um ANALISTA DE REQUISITOS independente, parte de um \
pipeline Dev -> QA -> Analista. O QA ja confirmou que a implementacao FUNCIONA tecnicamente (testes/ \
build passam) - seu trabalho e DIFERENTE: confirmar se ela realmente satisfaz o que o usuario PEDIU, \
nao se o codigo roda. Um codigo pode passar em todo teste e mesmo assim nao fazer o que foi pedido \
(ex: implementou a funcionalidade errada, ignorou um requisito explicito, resolveu so parte do \
pedido). Leia o requisito original com atencao e confira o resultado (leia o codigo/arquivos \
relevantes) contra ele, criterio por criterio. Assuma REFUTADO por padrao quando houver qualquer \
duvida sobre cobertura do requisito - o onus da prova e de quem alega ter atendido o pedido, nao \
seu. Voce NAO tem write_file/edit_file/ast_edit - so pode observar e reportar, nunca consertar o que \
encontrar. Responda comecando com a palavra EXATA 'APROVADO' ou 'REFUTADO' sozinha na primeira \
linha, seguida de EXATAMENTE quais partes do requisito foram atendidas e quais nao (se REFUTADO) - \
sem isso o veredito e invalido.";

/// Roda o Analista contra o requisito original + relatório do DEV e devolve
/// o veredito formatado — mesmo contrato de `verifier::run` (sempre começa
/// com "APROVADO"/"REFUTADO" na primeira linha).
#[allow(clippy::too_many_arguments)]
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
    requirement: &str,
    dev_report: &str,
) -> Result<String> {
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: ANALYST_SYSTEM_PROMPT.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Requisito original do usuario:\n{requirement}\n\n\
                 Relatorio do DEV sobre o que foi implementado:\n{dev_report}\n\n\
                 Confira se o requisito foi realmente atendido (leia o codigo/arquivos relevantes) \
                 antes de dar o veredito."
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
    ];

    let tool_specs = analyst_tool_specs();
    let mut recent_calls: Vec<(String, String)> = Vec::new();

    // Canal sintetico (mesmo padrao do verifier/subagent): o texto do
    // analista nao pode vazar no `chat:token` da sessao real.
    let stream_channel = format!("{session_id}::exec::{execution_id}");

    for step in 0..MAX_ANALYST_STEPS {
        let assistant = providers::chat_stream(
            app,
            &stream_channel,
            cfg,
            api_key.clone(),
            model,
            &messages,
            &tool_specs,
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
            return Ok(extract_verdict(&assistant.content));
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
                    tool: format!("📋 analista: {}", call.function.name),
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
            let result = tools::execute_tool(
                &call.function.name,
                &args,
                Some(project_root),
                &folder_entries,
                &state.background_jobs,
                &state.mcp_clients,
                &state.app_data_dir,
                &crate::models::ExecutionMode::Auto,
                session_id,
            )
            .await;
            let observation = match &result {
                Ok(outcome) => outcome.observation.clone(),
                Err(e) => format!("erro executando ferramenta: {e}"),
            };

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
                    additions: 0,
                    deletions: 0,
                    duration_ms: Some(elapsed_ms),
                    execution_id: Some(execution_id.to_string()),
                    images: Vec::new(),
                },
            );
            super::update_execution_step(state, execution_id, &call.id, result_status, Some(result_detail), 0, 0, elapsed_ms);

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
                    "REFUTADO\n[analista parou: chamou '{}' {} vezes seguidas com os mesmos \
                     argumentos, sem sinal de progresso - parece um loop, nao deu pra confirmar nada]",
                    call.function.name,
                    super::DOOM_LOOP_THRESHOLD
                ));
            }
        }

        if step + 1 == MAX_ANALYST_STEPS {
            break;
        }
    }

    Ok(format!(
        "REFUTADO\n[analista atingiu o limite de {MAX_ANALYST_STEPS} passos sem dar um veredito \
         claro - por seguranca, trate como nao confirmado]"
    ))
}

/// Mesma lógica de `verifier::extract_verdict` — mantida duplicada por
/// consistência com o resto do módulo (ver nota de topo sobre não
/// compartilhar código com `verifier.rs`).
fn extract_verdict(response: &str) -> String {
    let first_line = response.lines().next().unwrap_or("").trim().to_uppercase();
    if first_line.starts_with("APROVADO") {
        response.to_string()
    } else if first_line.starts_with("REFUTADO") {
        response.to_string()
    } else {
        format!("REFUTADO\n[analista nao devolveu um veredito no formato esperado - resposta original abaixo, trate como nao confirmado]\n{response}")
    }
}

fn analyst_tool_specs() -> Vec<ToolSpec> {
    tools::project_tool_specs()
        .into_iter()
        .filter(|t| ANALYST_ALLOWED_TOOLS.contains(&t.function.name.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyst_toolset_is_read_only_allowlist() {
        let specs = analyst_tool_specs();
        let names: Vec<&str> = specs.iter().map(|t| t.function.name.as_str()).collect();
        for expected in ANALYST_ALLOWED_TOOLS {
            assert!(names.contains(expected), "analista deveria ter '{expected}'");
        }
        for forbidden in [
            "write_file",
            "edit_file",
            "ast_edit",
            "task",
            "ask",
            "verify_completion",
            "run_pipeline",
            "check_background_output",
            "stop_background",
            "list_background",
        ] {
            assert!(!names.contains(&forbidden), "analista NAO deveria ter '{forbidden}'");
        }
    }

    #[test]
    fn extract_verdict_recognizes_aprovado() {
        let verdict = extract_verdict("APROVADO\ntodos os criterios do requisito foram atendidos");
        assert!(verdict.starts_with("APROVADO"));
    }

    #[test]
    fn extract_verdict_recognizes_refutado() {
        let verdict = extract_verdict("REFUTADO\nfaltou implementar o criterio X do requisito");
        assert!(verdict.starts_with("REFUTADO"));
    }

    #[test]
    fn extract_verdict_defaults_to_refutado_when_format_not_followed() {
        let verdict = extract_verdict("Acho que atende bem o que foi pedido.");
        assert!(
            verdict.starts_with("REFUTADO"),
            "sem veredito claro deveria ser tratado como nao confirmado: {verdict}"
        );
    }
}
