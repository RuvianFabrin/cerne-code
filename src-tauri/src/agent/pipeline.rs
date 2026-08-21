//! Fase 3 do roteiro de Agentes/Skills: pipeline Dev → QA → Analista com
//! orquestração DETERMINÍSTICA — a sequência (dev→qa→analista, e voltar pro
//! dev se algum dos dois refutar) é fixa em código Rust, não escolhida pelo
//! LLM turno a turno. O LLM do topo só decide QUANDO usar o pipeline
//! (chamando a tool `run_pipeline`, mesmo espírito de "LLM decide chamar,
//! Rust decide o que acontece dentro" que `task`/`verify_completion` já
//! usam) — o que roda depois de começar não é escolha dele.
//!
//! As 3 etapas reaproveitam módulos já existentes sem modificá-los:
//! - DEV → `subagent::run` (toolset completo, mesmo usado por `task`).
//! - QA → `verifier::run` (veredito técnico: o código FUNCIONA?).
//! - ANALISTA → `analyst::run` (veredito de requisito: o código faz o que
//!   foi PEDIDO? — ver `analyst.rs` pra por que isso é uma etapa separada
//!   do QA em vez de generalizar `verifier.rs`).

use super::{analyst, subagent, verifier};
use crate::models::{ExecutionMode, ProviderConfig};
use crate::AppState;
use anyhow::Result;
use std::path::Path;
use tauri::{AppHandle, Emitter};

pub const DEFAULT_MAX_ROUNDS: u32 = 3;

/// Evento de progresso do pipeline — mesmo espírito de `agent:agent_status`
/// (turno normal), só que com a etapa/round atual em vez de "thinking"/
/// "running_tool". Frontend usa isso pra mostrar "🛠️ Dev implementando
/// (round 1/3)...", "🧪 QA testando...", "📋 Analista conferindo...".
#[derive(serde::Serialize, Clone)]
struct PipelineStatusEvent {
    session_id: String,
    step: &'static str, // "dev" | "qa" | "analista"
    round: u32,
    max_rounds: u32,
}

fn emit_status(app: &AppHandle, session_id: &str, step: &'static str, round: u32, max_rounds: u32) {
    let _ = app.emit(
        "agent:pipeline_status",
        PipelineStatusEvent {
            session_id: session_id.to_string(),
            step,
            round,
            max_rounds,
        },
    );
}

/// Roda o pipeline completo e devolve o relatório final (sucesso ou
/// pendências residuais se `max_rounds` estourar — nunca trava
/// silenciosamente, sempre devolve algo pro usuário ver).
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
    enabled_mcp_servers: Option<&[String]>,
    execution_mode: &ExecutionMode,
    pipeline_execution_id: &str,
    requirement: &str,
    max_rounds: u32,
) -> Result<String> {
    let max_rounds = max_rounds.max(1);

    let mut current_prompt = format!(
        "Implemente: {requirement}\n\n\
         Quando terminar, resuma EXATAMENTE o que foi feito e quais arquivos mudaram — o relatorio \
         final e a UNICA coisa que as proximas etapas do pipeline (QA e Analista) vao ver, entao \
         inclua tudo que elas precisam saber."
    );
    let mut round = 1u32;

    loop {
        // --- DEV ---
        emit_status(app, session_id, "dev", round, max_rounds);
        let dev_execution_id = super::start_agent_execution(
            state,
            session_id,
            "task",
            &format!("Pipeline: Dev (round {round}/{max_rounds})"),
            Some(pipeline_execution_id),
        );
        let dev_result = subagent::run(
            app,
            state,
            session_id,
            &dev_execution_id,
            cfg,
            api_key.clone(),
            model,
            project_root,
            extra_read_paths,
            "Pipeline: implementacao",
            &current_prompt,
            enabled_mcp_servers,
            execution_mode,
        )
        .await;
        super::finish_agent_execution(state, &dev_execution_id, dev_result.is_ok());
        let dev_report = match dev_result {
            Ok(report) => report,
            Err(e) => {
                super::finish_agent_execution(state, pipeline_execution_id, false);
                return Err(e);
            }
        };

        // --- QA ---
        emit_status(app, session_id, "qa", round, max_rounds);
        let qa_execution_id = super::start_agent_execution(
            state,
            session_id,
            "verify_completion",
            &format!("Pipeline: QA (round {round}/{max_rounds})"),
            Some(pipeline_execution_id),
        );
        let qa_result = verifier::run(
            app,
            state,
            session_id,
            &qa_execution_id,
            cfg,
            api_key.clone(),
            model,
            project_root,
            extra_read_paths,
            &dev_report,
            "Rode os testes/build/lint relevantes e confirme se a implementacao esta tecnicamente \
             correta (roda sem erro, testes existentes continuam passando).",
        )
        .await;
        super::finish_agent_execution(state, &qa_execution_id, qa_result.is_ok());
        let qa_verdict = match qa_result {
            Ok(v) => v,
            Err(e) => {
                super::finish_agent_execution(state, pipeline_execution_id, false);
                return Err(e);
            }
        };

        if is_refutado(&qa_verdict) {
            if round >= max_rounds {
                super::finish_agent_execution(state, pipeline_execution_id, false);
                return Ok(pending_report(round, max_rounds, "QA", &dev_report, &qa_verdict));
            }
            round += 1;
            current_prompt = format!(
                "Requisito original: {requirement}\n\n\
                 O QA encontrou pendencias tecnicas na sua ultima tentativa:\n{qa_verdict}\n\n\
                 Corrija e tente novamente. Ao terminar, resuma de novo o que foi feito."
            );
            continue;
        }

        // --- ANALISTA ---
        emit_status(app, session_id, "analista", round, max_rounds);
        let analyst_execution_id = super::start_agent_execution(
            state,
            session_id,
            "verify_completion",
            &format!("Pipeline: Analista (round {round}/{max_rounds})"),
            Some(pipeline_execution_id),
        );
        let analyst_result = analyst::run(
            app,
            state,
            session_id,
            &analyst_execution_id,
            cfg,
            api_key.clone(),
            model,
            project_root,
            extra_read_paths,
            requirement,
            &dev_report,
        )
        .await;
        super::finish_agent_execution(state, &analyst_execution_id, analyst_result.is_ok());
        let analyst_verdict = match analyst_result {
            Ok(v) => v,
            Err(e) => {
                super::finish_agent_execution(state, pipeline_execution_id, false);
                return Err(e);
            }
        };

        if is_refutado(&analyst_verdict) {
            if round >= max_rounds {
                super::finish_agent_execution(state, pipeline_execution_id, false);
                return Ok(pending_report(round, max_rounds, "Analista", &dev_report, &analyst_verdict));
            }
            round += 1;
            current_prompt = format!(
                "Requisito original: {requirement}\n\n\
                 O Analista encontrou pendencias em relacao ao pedido original (o codigo funciona \
                 mas nao atende tudo que foi pedido):\n{analyst_verdict}\n\n\
                 Corrija e tente novamente. Ao terminar, resuma de novo o que foi feito."
            );
            continue;
        }

        super::finish_agent_execution(state, pipeline_execution_id, true);
        return Ok(format!(
            "✅ Pipeline concluido com sucesso no round {round}/{max_rounds}.\n\n\
             ## Relatorio do Dev\n{dev_report}\n\n\
             ## Veredito do QA\n{qa_verdict}\n\n\
             ## Veredito do Analista\n{analyst_verdict}"
        ));
    }
}

fn is_refutado(verdict: &str) -> bool {
    verdict.trim_start().to_uppercase().starts_with("REFUTADO")
}

fn pending_report(round: u32, max_rounds: u32, refused_by: &str, dev_report: &str, verdict: &str) -> String {
    format!(
        "⚠️ Pipeline NAO concluido apos {round}/{max_rounds} rounds — {refused_by} continuou \
         encontrando pendencias.\n\n\
         ## Ultimo relatorio do Dev\n{dev_report}\n\n\
         ## Ultimo veredito ({refused_by})\n{verdict}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_refutado_recognizes_prefix_case_insensitive() {
        assert!(is_refutado("REFUTADO\nfaltou algo"));
        assert!(is_refutado("refutado\nfaltou algo"));
        assert!(!is_refutado("APROVADO\ntudo certo"));
    }

    #[test]
    fn pending_report_includes_round_and_verdict() {
        let report = pending_report(3, 3, "QA", "implementei X", "REFUTADO\nteste Y falhou");
        assert!(report.contains("3/3"));
        assert!(report.contains("QA"));
        assert!(report.contains("implementei X"));
        assert!(report.contains("teste Y falhou"));
    }
}
