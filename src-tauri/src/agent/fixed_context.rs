//! "Ctx fixo" (`FixedContextState`): limita o que é MANDADO ao modelo a cada
//! envio, sem tocar no histórico salvo (o chat que o usuário vê) nem no modo
//! Long Horizon. O corte feito é gravado em `model_window.json` e continua
//! valendo se o Ctx fixo for desligado (ver `payload_off`).
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

/// Com o Ctx fixo DESLIGADO numa sessão que já usou o Ctx fixo, o modelo continua sem
/// receber o que já saiu da janela (ver [`payload_off`]). O que ele recebe cabe nesta
/// fração da janela de contexto do provedor; o resto fica pra resposta, raciocínio e
/// ferramentas.
const OFF_BUDGET_RATIO: f32 = 0.80;

/// Começo gravado da janela do modelo, se ainda vale pra este histórico. Histórico que
/// encolheu (mensagem apagada, compactação antiga) invalida o índice guardado.
fn saved_start(app_data_dir: &PathBuf, session_id: &str, len: usize) -> Option<usize> {
    sessions::read_model_window(app_data_dir, session_id)
        .filter(|window| window.log_len <= len)
        .map(|window| window.start.min(len))
}

/// A sessão tem janela do modelo gravada (e ela vale pra este histórico)? Enquanto tiver,
/// compactar o histórico inteiro de uma vez (resumir tudo no mesmo pedido) é perigoso:
/// o pedido pode não caber na janela do servidor.
pub fn has_window(app_data_dir: &PathBuf, session_id: &str, len: usize) -> bool {
    saved_start(app_data_dir, session_id, len).is_some()
}

/// Grava o começo da janela do modelo. Nunca recua: o que já saiu do envio não volta.
/// Só o que o modelo recebe muda; `chat_log.json` (o chat que o usuário vê) não é tocado.
/// Gravação por melhor esforço: se falhar, o envio segue e a próxima chamada grava de novo.
fn remember_window(app_data_dir: &PathBuf, session_id: &str, start: usize, len: usize) {
    let start = saved_start(app_data_dir, session_id, len).map_or(start, |old| old.max(start));
    let _ = sessions::write_model_window(
        app_data_dir,
        session_id,
        &sessions::ModelWindow { start, log_len: len },
    );
}

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
    let (out, start) = if !state.rolling && state.blocks {
        let hint = saved_start(app_data_dir, session_id, messages.len());
        context::fixed_window_blocks(
            messages, budget, tool_specs, model, hint, context::FIXED_BLOCKS_KEEP_RATIO,
        )
    } else if !state.rolling {
        (
            context::fixed_window(messages, budget, tool_specs, model),
            context::fixed_window_start(messages, budget, tool_specs, model),
        )
    } else {
        let saved = sessions::read_rolling_summary(app_data_dir, session_id);
        (
            context::rolling_window(messages, budget, tool_specs, model, &saved.text, saved.covered),
            context::rolling_split(messages, budget, tool_specs, model, &saved.text, saved.covered),
        )
    };
    // O corte vale também depois: se o Ctx fixo for desligado, o modelo não volta a
    // receber o que já saiu (ver `payload_off`).
    remember_window(app_data_dir, session_id, start, messages.len());
    out
}

/// Envio com o Ctx fixo DESLIGADO. Se a sessão já tem janela gravada, o modelo continua
/// recebendo só o que vem depois do corte, e o corte anda em bloco quando o que sobra passa
/// de [`OFF_BUDGET_RATIO`] da janela de contexto — então desligar o Ctx fixo nunca despeja o
/// histórico inteiro no modelo. O resumo rolante, se existir, volta logo depois do prompt de
/// sistema. `None` = sessão sem janela gravada: segue o caminho normal (sem mudança).
/// O chat visível (`messages`) não é alterado.
pub fn payload_off(
    app_data_dir: &PathBuf,
    session_id: &str,
    messages: &[ChatMessage],
    tool_specs: &[ToolSpec],
    model: &str,
    context_length: u32,
) -> Option<Vec<ChatMessage>> {
    let hint = saved_start(app_data_dir, session_id, messages.len())?;
    let budget = (context_length as f32 * OFF_BUDGET_RATIO) as u32;
    let (out, start) = context::fixed_window_blocks(
        messages, budget, tool_specs, model, Some(hint), context::FIXED_BLOCKS_KEEP_RATIO,
    );
    remember_window(app_data_dir, session_id, start, messages.len());
    let summary = sessions::read_rolling_summary(app_data_dir, session_id);
    Some(context::with_summary(out, &summary.text))
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

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "gpt-4o";
    const SESSAO: &str = "s1";

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        }
    }

    fn cost(m: &ChatMessage) -> u32 {
        context::estimate_messages_tokens(std::slice::from_ref(m), &[], MODEL)
    }

    fn total(out: &[ChatMessage]) -> u32 {
        out.iter().map(cost).sum()
    }

    fn texto(i: usize) -> String {
        format!("mensagem numero {i} com algum texto de apoio")
    }

    fn conversa(n: usize) -> Vec<ChatMessage> {
        let mut messages = vec![msg("system", "sistema")];
        for i in 0..n {
            messages.push(msg(if i % 2 == 0 { "user" } else { "assistant" }, &texto(i)));
        }
        messages
    }

    fn custo_max() -> u32 {
        (0..200).map(|i| cost(&msg("user", &texto(i)))).max().unwrap()
    }

    fn pasta() -> PathBuf {
        std::env::temp_dir().join(format!("cerne-fixed-{}", uuid::Uuid::new_v4()))
    }

    fn estado(tokens: u32, blocks: bool) -> FixedContextState {
        FixedContextState { enabled: true, tokens, rolling: false, blocks }
    }

    #[test]
    fn ctx_fixo_grava_o_corte_e_ele_nunca_recua() {
        let dir = pasta();
        let messages = conversa(30);
        let tokens = cost(&messages[0]) + 10 * custo_max();

        payload(&dir, SESSAO, &messages, &[], MODEL, &estado(tokens, false), 1_000_000);
        let gravada = sessions::read_model_window(&dir, SESSAO).expect("o Ctx fixo grava a janela");
        assert_eq!(gravada.log_len, messages.len());
        assert!(gravada.start >= 20, "só as ~10 últimas entram: começo em {}", gravada.start);

        // Um orçamento bem maior depois não traz de volta o que já saiu.
        payload(&dir, SESSAO, &messages, &[], MODEL, &estado(1_000_000, false), 1_000_000);
        assert_eq!(sessions::read_model_window(&dir, SESSAO).unwrap().start, gravada.start);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn desligado_o_modelo_continua_sem_o_que_ja_saiu_e_o_historico_nao_muda() {
        let dir = pasta();
        let messages = conversa(30);
        let tokens = cost(&messages[0]) + 10 * custo_max();
        payload(&dir, SESSAO, &messages, &[], MODEL, &estado(tokens, true), 1_000_000);
        let start = sessions::read_model_window(&dir, SESSAO).unwrap().start;

        // Janela do provedor bem maior que o histórico inteiro: sem a gravação, tudo iria.
        let out = payload_off(&dir, SESSAO, &messages, &[], MODEL, 1_000_000).expect("tem janela gravada");
        assert_eq!(out[0].role, "system");
        assert_eq!(out[1].content, messages[start].content, "começa no corte gravado");
        assert!(out.len() < messages.len() / 2, "não volta o histórico inteiro: {} de {}", out.len(), messages.len());
        assert!(!out.iter().any(|m| m.content == texto(0)));
        assert_eq!(messages.len(), 31, "o histórico visível não é alterado");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn desligado_corta_em_bloco_quando_enche_a_janela_do_provedor() {
        let dir = pasta();
        let mut messages = conversa(30);
        let tokens = cost(&messages[0]) + 10 * custo_max();
        payload(&dir, SESSAO, &messages, &[], MODEL, &estado(tokens, true), 1_000_000);

        // 0,80 da janela do provedor = o sistema + 40 mensagens.
        let janela = ((cost(&messages[0]) + 40 * custo_max()) as f32 / 0.80) as u32 + 1;
        let limite = (janela as f32 * OFF_BUDGET_RATIO) as u32;
        let mut starts = vec![sessions::read_model_window(&dir, SESSAO).unwrap().start];
        for i in 30..200 {
            messages.push(msg(if i % 2 == 0 { "user" } else { "assistant" }, &texto(i)));
            let out = payload_off(&dir, SESSAO, &messages, &[], MODEL, janela).unwrap();
            assert!(total(&out) <= limite, "envio {i} passou do limite: {} > {limite}", total(&out));
            starts.push(sessions::read_model_window(&dir, SESSAO).unwrap().start);
        }
        assert!(starts.windows(2).all(|w| w[0] <= w[1]), "o corte nunca recua");
        let cortes = starts.windows(2).filter(|w| w[0] != w[1]).count();
        assert!(cortes > 0 && cortes < 40, "corta em bloco, não a cada mensagem: {cortes} cortes em 170");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn sem_janela_gravada_nada_muda() {
        let dir = pasta();
        let messages = conversa(10);
        assert!(payload_off(&dir, SESSAO, &messages, &[], MODEL, 1_000_000).is_none());
        assert!(!has_window(&dir, SESSAO, messages.len()));
    }

    #[test]
    fn historico_que_encolheu_invalida_a_janela_gravada() {
        let dir = pasta();
        sessions::write_model_window(&dir, SESSAO, &sessions::ModelWindow { start: 20, log_len: 100 }).unwrap();
        let messages = conversa(30);
        assert!(!has_window(&dir, SESSAO, messages.len()));
        assert!(payload_off(&dir, SESSAO, &messages, &[], MODEL, 1_000_000).is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
