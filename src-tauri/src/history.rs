//! Conserta o histórico de mensagens antes de mandar pro provider.
//!
//! Toda API compatível com OpenAI exige que uma mensagem `assistant` com
//! `tool_calls` seja seguida IMEDIATAMENTE pelas mensagens `tool` que
//! respondem a cada `tool_call_id`. Se não for, o provider devolve
//! `400 Bad Request` com *"An assistant message with 'tool_calls' must be
//! followed by tool messages responding to each 'tool_call_id'"* — e a sessão
//! inteira fica travada, porque toda mensagem nova reenvia o mesmo histórico
//! quebrado (achado ao vivo em 2026-09-12, sessão "Checar projeto e gerar
//! build": a conversa parou de responder de vez).
//!
//! Três caminhos do Cerne quebravam essa invariante:
//!
//! 1. `agent::run_turn` salva a mensagem do assistente com `tool_calls` em
//!    disco ANTES de executar as ferramentas (de propósito: se o processo
//!    morrer, o pedido não se perde). Se o turno for abortado nesse meio tempo
//!    (`cancel_turn`), ou se um `?` derrubar o turno entre o pedido e o
//!    resultado, o histórico fica com o pedido e sem a resposta.
//! 2. `background::on_job_finished` injeta a nota de "comando em segundo plano
//!    concluído" no histórico a qualquer momento — inclusive no meio de um
//!    turno, entre o `tool_calls` já salvo e o resultado ainda não salvo.
//! 3. `agent::maybe_compact` corta o histórico num ponto arbitrário pra
//!    resumir (`KEEP_LAST_MESSAGES`), sem olhar se o corte separa um
//!    `tool_calls` das respostas dele.
//!
//! `repair` normaliza tudo isso de uma vez: descarta respostas `tool` órfãs
//! (sem pedido correspondente) e sintetiza uma resposta para todo
//! `tool_call_id` que ficou sem a sua. Sintetiza em vez de apagar o pedido
//! porque apagar perderia a informação de que a ferramenta foi chamada — o
//! modelo repetiria a chamada sem entender por quê, e a UI perderia o passo.

use crate::models::{ChatMessage, ToolCall};

/// Fecha as chamadas que ficaram sem resposta, inserindo uma resposta
/// sintética no lugar exato onde elas deveriam estar (logo depois do pedido).
fn flush_pending(pending: &mut Vec<ToolCall>, out: &mut Vec<ChatMessage>, fixed: &mut usize) {
    for call in pending.drain(..) {
        out.push(synthetic_result(&call));
        *fixed += 1;
    }
}

fn synthetic_result(call: &ToolCall) -> ChatMessage {
    ChatMessage {
        role: "tool".to_string(),
        content: format!(
            "⚠️ Resultado perdido: o Cerne foi interrompido (cancelado, reiniciado ou deu erro) \
             antes desta ferramenta terminar de responder. Ela pode nao ter rodado por completo — \
             nao assuma nenhum resultado; se precisar de verdade, chame '{}' de novo.",
            call.function.name
        ),
        tool_calls: None,
        tool_call_id: Some(call.id.clone()),
        name: Some(call.function.name.clone()),
        images: Vec::new(),
        display_content: None,
    }
}

/// Conserta o histórico no lugar. Devolve quantas mensagens foram
/// acrescentadas (respostas sintéticas) ou descartadas (respostas órfãs) —
/// `0` significa que o histórico já estava íntegro e nada mudou.
pub fn repair(messages: &mut Vec<ChatMessage>) -> usize {
    let mut fixed = 0usize;
    let mut out: Vec<ChatMessage> = Vec::with_capacity(messages.len());
    // Chamadas da última mensagem de assistente que ainda esperam resposta,
    // na ordem em que o modelo pediu.
    let mut pending: Vec<ToolCall> = Vec::new();

    for msg in messages.drain(..) {
        if msg.role == "tool" {
            let answers_pending = msg
                .tool_call_id
                .as_deref()
                .map(|id| pending.iter().any(|c| c.id == id))
                .unwrap_or(false);
            if answers_pending {
                let id = msg.tool_call_id.clone();
                pending.retain(|c| Some(c.id.as_str()) != id.as_deref());
                out.push(msg);
            } else {
                // Resposta sem pedido (sobrou de um corte de resumo, ou é
                // duplicata da mesma chamada) — o provider rejeitaria.
                fixed += 1;
            }
            continue;
        }

        // Qualquer mensagem que não seja `tool` fecha o grupo pendente: as
        // respostas precisam vir imediatamente depois do pedido, então elas
        // entram antes desta mensagem (que pode ser uma nota de job em
        // background injetada no meio, uma nova mensagem do usuário, etc).
        flush_pending(&mut pending, &mut out, &mut fixed);

        if msg.role == "assistant" {
            pending = msg.tool_calls.clone().unwrap_or_default();
        }
        out.push(msg);
    }

    // Fim da lista com pedido em aberto (turno abortado depois de salvar o
    // pedido, por exemplo).
    flush_pending(&mut pending, &mut out, &mut fixed);

    *messages = out;
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ToolCallFunction;

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

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: "{}".to_string(),
            },
        }
    }

    fn assistant_with_calls(calls: Vec<ToolCall>) -> ChatMessage {
        let mut m = msg("assistant", "");
        m.tool_calls = Some(calls);
        m
    }

    fn tool_result(id: &str, name: &str) -> ChatMessage {
        let mut m = msg("tool", "resultado");
        m.tool_call_id = Some(id.to_string());
        m.name = Some(name.to_string());
        m
    }

    fn roles(messages: &[ChatMessage]) -> Vec<&str> {
        messages.iter().map(|m| m.role.as_str()).collect()
    }

    #[test]
    fn valid_history_is_left_untouched() {
        let mut messages = vec![
            msg("system", "prompt"),
            msg("user", "oi"),
            assistant_with_calls(vec![call("c1", "read_file")]),
            tool_result("c1", "read_file"),
            msg("assistant", "pronto"),
        ];
        let before: Vec<String> = roles(&messages).into_iter().map(String::from).collect();
        assert_eq!(repair(&mut messages), 0);
        assert_eq!(roles(&messages), before.iter().map(String::as_str).collect::<Vec<_>>());
    }

    #[test]
    fn trailing_tool_call_gets_a_synthetic_response() {
        // Turno abortado (`cancel_turn`) depois do pedido ser salvo em disco.
        let mut messages = vec![
            msg("system", "prompt"),
            msg("user", "roda o teste"),
            assistant_with_calls(vec![call("c1", "run_command")]),
        ];
        assert_eq!(repair(&mut messages), 1);
        assert_eq!(roles(&messages), vec!["system", "user", "assistant", "tool"]);
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("c1"));
        assert_eq!(messages[3].name.as_deref(), Some("run_command"));
    }

    #[test]
    fn note_injected_between_call_and_result_is_pushed_after_the_result() {
        // Bug ao vivo de 2026-09-12: a nota de "comando em segundo plano
        // concluido" entrou entre o `tool_calls` e a resposta dele.
        let mut messages = vec![
            msg("system", "prompt"),
            msg("user", "roda em background"),
            assistant_with_calls(vec![call("c1", "run_command")]),
            msg("system", "comando concluido"),
            msg("user", "e agora?"),
        ];
        assert_eq!(repair(&mut messages), 1);
        assert_eq!(
            roles(&messages),
            vec!["system", "user", "assistant", "tool", "system", "user"]
        );
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("c1"));
        assert_eq!(messages[4].content, "comando concluido");
    }

    #[test]
    fn orphan_tool_message_is_dropped() {
        // Corte de resumo que deixou a resposta sem o pedido.
        let mut messages = vec![
            msg("system", "prompt"),
            tool_result("sumiu", "grep"),
            msg("user", "oi"),
        ];
        assert_eq!(repair(&mut messages), 1);
        assert_eq!(roles(&messages), vec!["system", "user"]);
    }

    #[test]
    fn partial_group_answers_only_the_missing_calls() {
        let mut messages = vec![
            assistant_with_calls(vec![call("c1", "read_file"), call("c2", "grep")]),
            tool_result("c1", "read_file"),
            msg("user", "e o grep?"),
        ];
        assert_eq!(repair(&mut messages), 1);
        assert_eq!(
            roles(&messages),
            vec!["assistant", "tool", "tool", "user"]
        );
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("c1"));
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("c2"));
    }

    #[test]
    fn duplicate_answer_for_the_same_call_is_dropped() {
        let mut messages = vec![
            assistant_with_calls(vec![call("c1", "read_file")]),
            tool_result("c1", "read_file"),
            tool_result("c1", "read_file"),
            msg("assistant", "pronto"),
        ];
        assert_eq!(repair(&mut messages), 1);
        assert_eq!(
            roles(&messages),
            vec!["assistant", "tool", "assistant"]
        );
    }

    #[test]
    fn realistic_corrupted_session_is_repaired_in_full() {
        // Reprodução da sessão travada de verdade: pedido + nota de
        // background + as mensagens do usuário que vinham tomando 400.
        let mut messages = vec![
            msg("system", "prompt"),
            msg("user", "roda o build em background"),
            assistant_with_calls(vec![call(
                "call_00_ET_qBu2Ha5e23hcc18cuW5g2629",
                "run_command",
            )]),
            msg("system", "comando em segundo plano concluido"),
            msg("user", "O que aconteceu?"),
            msg("user", "e agora?"),
        ];
        assert_eq!(repair(&mut messages), 1);
        assert_eq!(
            roles(&messages),
            vec!["system", "user", "assistant", "tool", "system", "user", "user"]
        );
        // Nenhum `tool_calls` fica sem resposta imediata depois do reparo.
        for (i, m) in messages.iter().enumerate() {
            if let Some(calls) = m.tool_calls.as_ref().filter(|c| !c.is_empty()) {
                for c in calls {
                    assert_eq!(
                        messages[i + 1].tool_call_id.as_deref(),
                        Some(c.id.as_str()),
                        "chamada {} não seguida de resposta",
                        c.id
                    );
                }
            }
        }
    }
}
