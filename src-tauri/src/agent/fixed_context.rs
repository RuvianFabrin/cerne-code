//! "Ctx fixo" (`FixedContextState`): limita o que é MANDADO ao modelo a cada
//! envio, sem tocar no histórico salvo nem no modo Long Horizon.
//!
//! Dois modos:
//! - cortado (`rolling = false`): sistema + o trecho recente que cabe. Rápido,
//!   mas o que saiu da janela some do envio.
//! - rolante (`rolling = true`): sistema + um resumo do que saiu + o trecho
//!   recente. O resumo é feito pelo próprio modelo, em segundo plano, depois
//!   de cada turno, e só resume o que acabou de sair da janela. Assim a
//!   pessoa não espera o resumo na hora de enviar; em troca, o resumo pode
//!   estar um turno atrasado.

use crate::context;
use crate::models::{ChatMessage, FixedContextState, ProviderConfig, ToolSpec};
use crate::{providers, sessions};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

/// Fração do orçamento que o resumo pode ocupar. O resto é o trecho recente.
const SUMMARY_BUDGET_RATIO: f32 = 0.20;

const ROLLING_SUMMARY_PROMPT: &str = "Voce mantem o resumo rolante de uma conversa longa \
entre uma pessoa e um agente. Recebe o resumo anterior e mensagens novas que saíram da \
janela de contexto, e devolve o resumo atualizado. Mantenha: objetivo da pessoa, decisões \
tomadas, arquivos e comandos mexidos, erros e como foram resolvidos, pendências e \
preferências declaradas. Escreva em português, denso e factual, sem saudação.";

/// Monta o que vai ao modelo neste envio, conforme o modo do Ctx fixo.
pub fn payload(
    app_data_dir: &PathBuf,
    session_id: &str,
    messages: &[ChatMessage],
    tool_specs: &[ToolSpec],
    model: &str,
    state: &FixedContextState,
    context_length: u32,
) -> Vec<ChatMessage> {
    let budget = state.tokens.min(context_length);
    if !state.rolling {
        return context::fixed_window(messages, budget, tool_specs, model);
    }
    let saved = sessions::read_rolling_summary(app_data_dir, session_id);
    context::rolling_window(messages, budget, tool_specs, model, &saved.text, saved.covered)
}

/// Uma atualização por sessão por vez. Se já há uma rodando, a nova não entra.
fn in_progress() -> &'static Mutex<HashSet<String>> {
    static IN_PROGRESS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_PROGRESS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Atualiza o resumo rolante da sessão com o que saiu do trecho recente.
/// Erros não chegam à pessoa: o resumo anterior continua valendo.
#[allow(clippy::too_many_arguments)]
pub async fn roll(
    app: AppHandle,
    app_data_dir: PathBuf,
    session_id: String,
    cfg: ProviderConfig,
    api_key: Option<String>,
    model: String,
    messages: Vec<ChatMessage>,
    tool_specs: Vec<ToolSpec>,
    state: FixedContextState,
    context_length: u32,
) {
    if !in_progress().lock().unwrap().insert(session_id.clone()) {
        return;
    }
    let _ = roll_inner(
        &app, &app_data_dir, &session_id, &cfg, api_key, &model, &messages, &tool_specs, &state, context_length,
    )
    .await;
    in_progress().lock().unwrap().remove(&session_id);
}

#[allow(clippy::too_many_arguments)]
async fn roll_inner(
    app: &AppHandle,
    app_data_dir: &PathBuf,
    session_id: &str,
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    messages: &[ChatMessage],
    tool_specs: &[ToolSpec],
    state: &FixedContextState,
    context_length: u32,
) -> anyhow::Result<()> {
    let budget = state.tokens.min(context_length);
    let mut saved = sessions::read_rolling_summary(app_data_dir, session_id);
    if saved.covered > messages.len() {
        // O histórico encolheu (ex.: mensagem apagada): o resumo não bate mais.
        saved = sessions::RollingSummary::default();
    }
    let covered_from = saved.covered.max(1);
    let start = context::rolling_split(messages, budget, tool_specs, model, &saved.text, saved.covered);
    if start <= covered_from {
        return Ok(());
    }

    let transcript = messages[covered_from..start]
        .iter()
        .map(|m| match m.role.as_str() {
            "tool" => format!("[ferramenta {}] {}", m.name.as_deref().unwrap_or("?"), m.content),
            other => format!("[{other}] {}", m.content),
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let words = ((budget as f32 * SUMMARY_BUDGET_RATIO) * 0.75).max(150.0) as u32;
    let previous = if saved.text.trim().is_empty() { "(nenhum)" } else { saved.text.as_str() };
    let request = vec![
        ChatMessage {
            role: "system".to_string(),
            content: ROLLING_SUMMARY_PROMPT.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Resumo anterior:\n{previous}\n\nMensagens novas:\n{transcript}\n\n\
                 Devolva o resumo atualizado, no máximo {words} palavras."
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
    ];
    // Canal sintético: os tokens do resumo não aparecem no chat.
    let channel = format!("{session_id}::rolling");
    let reply = providers::chat_stream(
        app,
        &channel,
        cfg,
        api_key,
        model,
        &request,
        &[],
        cfg.kind.default_reasoning_effort(),
        None,
    )
    .await?
    .message;
    sessions::write_rolling_summary(
        app_data_dir,
        session_id,
        &sessions::RollingSummary { text: reply.content.trim().to_string(), covered: start },
    )?;
    Ok(())
}
