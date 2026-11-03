//! Fila de tarefas SEM Long Horizon (pedido do usuário, 2026-11-03).
//!
//! Cópia do fluxo de `task_queue.rs`, com as diferenças pedidas:
//! - roda em contexto normal do Cerne (sem contexto limpo nem briefing do LH);
//! - sem limite de etapas: nenhum teto de turnos, só a confirmação "feito"
//!   (`CONFIRM_RETRIES`, pra nunca avançar um item que não foi confirmado);
//! - ao confirmar um item ele vira `testar`, depois a sessão salva na
//!   **memória da sessão** o que for relevante e, só então, compacta o
//!   contexto antes de partir pro próximo item;
//! - a memória da sessão é só desta sessão, vai INTEIRA no system prompt de
//!   todo turno e nunca passa pela compactação (que mexe só em `messages`).
//!
//! A lista usa o mesmo formato de `tarefas.json` (ver `task_queue::parse_tasks`),
//! mas fica numa pasta própria (`plain_queue/`), separada da fila do LH.

use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use super::task_queue::{self, CONFIRM_QUESTION, CONFIRM_RETRIES, STATUS_TESTAR};
use crate::models::{ToolFunctionSpec, ToolSpec};
use crate::sessions;

/// Nome do evento de status desta fila (diferente de `agent:task_queue_status`,
/// pra que as duas filas não se confundam no frontend).
pub const STATUS_EVENT: &str = "agent:plain_queue_status";

pub const MEMORY_TOOL: &str = "update_session_memory";

/// Instruções de sistema desta fila e da memória da sessão.
const PLAIN_QUEUE_PROMPT: &str = "## Fila de tarefas (sem Long Horizon)\n\
Você está executando uma lista de tarefas, uma por vez. Cada tarefa chega como uma mensagem do usuário. \
Quando perguntarem se a tarefa foi feita, responda somente 'feito' se ela foi realmente executada e verificada; \
caso contrário, diga o que falta.\n\n\
## Memória da sessão\n\
Abaixo está a memória desta sessão, inteira. Ela vale só para esta sessão e nunca é compactada. \
Para alterá-la use `update_session_memory` com o conteúdo COMPLETO (substitui o arquivo inteiro), \
mantendo o que já estava lá. Registre só o que ajuda as próximas tarefas desta sessão: decisões, \
caminhos de arquivos, comandos que funcionaram e erros a não repetir.";

/// Mandado depois que um item foi confirmado: pede o registro na memória.
fn memory_save_prompt(item_id: &str) -> String {
    format!(
        "A tarefa {item_id} foi concluída e marcada como testar. Agora registre na memória da sessão, \
com `{MEMORY_TOOL}`, somente o que for relevante para as próximas tarefas desta sessão (decisões, \
caminhos de arquivos, comandos que funcionaram, erros a não repetir). Reescreva o conteúdo COMPLETO, \
mantendo o que já estava na memória. Se nada for relevante, não chame a ferramenta e responda 'nada a salvar'."
    )
}

/// Bloco do system prompt com a memória da sessão INTEIRA. Chamado a cada
/// montagem do prompt (início de cada turno), então uma gravação feita por
/// `update_session_memory` aparece no turno seguinte.
pub fn memory_prompt_block(app_data_dir: &PathBuf, session_id: &str) -> Result<String> {
    let memoria = sessions::read_session_memory(app_data_dir, session_id)?;
    let conteudo = if memoria.trim().is_empty() { "(vazia)" } else { memoria.as_str() };
    Ok(format!("\n\n{PLAIN_QUEUE_PROMPT}\n\n<memoria_da_sessao>\n{conteudo}\n</memoria_da_sessao>"))
}

/// Ferramenta que reescreve a memória da sessão (sem limite de tamanho).
pub fn tool_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        kind: "function".to_string(),
        function: ToolFunctionSpec {
            name: MEMORY_TOOL.to_string(),
            description: "SUBSTITUI o conteúdo inteiro da memória desta sessão. Use só para registrar \
fatos úteis para as próximas tarefas desta mesma sessão. Inclua o conteúdo antigo que ainda vale, \
porque o arquivo é reescrito por completo."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "conteudo": { "type": "string", "description": "Conteúdo COMPLETO da memória da sessão" }
                },
                "required": ["conteudo"]
            }),
        },
    }]
}

#[derive(Serialize, Clone)]
struct PlainQueueEvent {
    session_id: String,
    /// "processing" | "confirmed" | "saving_memory" | "compacting" | "stuck" | "finished" | "error"
    status: &'static str,
    item_id: Option<String>,
    message: Option<String>,
}

/// Mesmo papel de `task_queue::emit_error`: o chamador emite "error" quando
/// `run_queue` falha.
pub fn emit_error(app: &AppHandle, session_id: &str, message: String) {
    emit_status(app, session_id, "error", None, Some(message));
}

fn emit_status(app: &AppHandle, session_id: &str, status: &'static str, item_id: Option<String>, message: Option<String>) {
    let _ = app.emit(
        STATUS_EVENT,
        PlainQueueEvent { session_id: session_id.to_string(), status, item_id, message },
    );
}

/// Uma volta completa de turno normal (mesmo mecanismo de `send_message`).
async fn run_turn(app: &AppHandle, session_id: &str, text: String) -> Result<()> {
    let state = app.state::<crate::AppState>();
    super::run_turn(app.clone(), &state, session_id.to_string(), text, Vec::new(), None).await
}

/// Roda a fila inteira sem interação: próximo `fazer` → turno com o item →
/// confirmação "feito" (até `CONFIRM_RETRIES`) → `testar` → salvar memória →
/// compactar → próximo. Para só quando a lista acaba ou quando um item não é
/// confirmado (nunca pula um item). Cancelamento é externo, via `.abort()`.
pub async fn run_queue(app: AppHandle, session_id: String) -> Result<()> {
    let app_data_dir = app.state::<crate::AppState>().app_data_dir.clone();

    loop {
        let tasks_json = sessions::read_plain_queue(&app_data_dir, &session_id)?;
        let mut tasks = task_queue::parse_tasks(&tasks_json)?;

        let Some(idx) = task_queue::find_next_fazer(&tasks) else {
            emit_status(&app, &session_id, "finished", None, None);
            return Ok(());
        };
        let item_id = tasks[idx]
            .get("id")
            .map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))
            .unwrap_or_else(|| "?".to_string());
        let prompt_text = task_queue::item_prompt_text(&tasks[idx])?;

        emit_status(&app, &session_id, "processing", Some(item_id.clone()), None);
        run_turn(&app, &session_id, prompt_text.clone()).await?;

        let mut confirmed = false;
        let mut confirm_attempts = 0;
        for _attempt in 0..CONFIRM_RETRIES {
            confirm_attempts += 1;
            run_turn(
                &app,
                &session_id,
                format!("Item da fila: {item_id}\nDescrição: {prompt_text}\n\n{CONFIRM_QUESTION}\nConfirme somente o resultado deste item, com base no que foi realmente executado e verificado. Não refaça o trabalho nem regrave a memória só para confirmar."),
            )
            .await?;
            let messages = sessions::load_messages(&app_data_dir, &session_id)?;
            let confirmed_now = messages
                .iter()
                .rev()
                .find(|m| m.role == "assistant")
                .is_some_and(|reply| task_queue::confirms_done(&reply.content));
            if confirmed_now {
                confirmed = true;
                break;
            }
        }

        if !confirmed {
            emit_status(
                &app,
                &session_id,
                "stuck",
                Some(item_id.clone()),
                Some(format!(
                    "O item {item_id} não foi confirmado como concluído depois de {confirm_attempts} tentativas — fila parada, o item continua com status \"fazer\"."
                )),
            );
            return Ok(());
        }

        task_queue::set_status(&mut tasks, idx, STATUS_TESTAR);
        sessions::write_plain_queue_tasks(&app_data_dir, &session_id, &tasks)?;
        emit_status(&app, &session_id, "confirmed", Some(item_id.clone()), None);

        emit_status(&app, &session_id, "saving_memory", Some(item_id.clone()), None);
        run_turn(&app, &session_id, memory_save_prompt(&item_id)).await?;

        // Depois de salvar a memória: compacta o contexto do histórico antes da
        // próxima tarefa. A memória da sessão não entra aqui (vai no prompt).
        emit_status(&app, &session_id, "compacting", Some(item_id), None);
        {
            let state = app.state::<crate::AppState>();
            super::compact_now(app.clone(), &state, session_id.clone()).await?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memoria_vazia_aparece_como_vazia_e_depois_com_conteudo() {
        let dir = std::env::temp_dir().join(format!("cerne-plain-{}", uuid::Uuid::new_v4()));
        let block = memory_prompt_block(&dir, "s1").unwrap();
        assert!(block.contains("(vazia)"));
        sessions::write_session_memory(&dir, "s1", "Usar Angular 18").unwrap();
        let block = memory_prompt_block(&dir, "s1").unwrap();
        assert!(block.contains("Usar Angular 18"));
        assert!(!block.contains("(vazia)"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn ferramenta_usa_o_nome_do_despacho() {
        assert_eq!(tool_specs()[0].function.name, MEMORY_TOOL);
    }
}
