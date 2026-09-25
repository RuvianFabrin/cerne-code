//! Fila de tarefas: roda uma lista de itens (JSON colado pelo usuário, na
//! convenção `tarefas.json` que ele já usa no "segundo cérebro" — id +
//! descrição + status `fazer`/`testar`/`pronto`/`bloqueada`/`cancelada`)
//! contra o modo Long Horizon, um item de cada vez, SEM interação do
//! usuário até acabar ou até `stop`.
//!
//! **Por que existe** (pedido do usuário, 2026-09-24): rodar uma lista de
//! implementação inteira com um modelo LOCAL fraco, sem precisar ficar
//! clicando "continuar" — cada item vira um turno completo (com o contexto
//! limpo do Long Horizon), seguido de uma pergunta de confirmação
//! ("respondeu 'feito'?") que decide se o item avança pra `testar`.
//!
//! **Formato aceito**: um objeto `{"tarefas": [...]}` ou um array solto
//! `[...]` no topo — cada item é um `serde_json::Value` (objeto) tratado de
//! forma flexível: só `id` e `status` são exigidos, tudo mais (descrição,
//! campos extras que o cofre já usa) é preservado como está, nunca
//! reescrito além do campo `status`. Isso deixa colar o JSON de um projeto
//! real do cofre sem precisar converter nada.

use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::sessions;

pub const STATUS_FAZER: &str = "fazer";
pub const STATUS_TESTAR: &str = "testar";

/// Quantas vezes repete a pergunta de confirmação antes de desistir e parar
/// a fila inteira (pedido do usuário: nunca pular um item sozinho — melhor
/// parar e avisar qual travou do que avançar errado).
pub const CONFIRM_RETRIES: u32 = 3;

/// Bloco de prompt injetado no system prompt quando `Session.task_queue_enabled`
/// está ligado — few-shot ensinando o modelo a responder `feito` (e só
/// `feito`) na pergunta de confirmação. Ver `docs/comparison` (pesquisa
/// 2026-09-24): 3-5 exemplos é o ponto ideal pra formato de saída confiável
/// em modelos pequenos.
pub const TASK_QUEUE_PROMPT: &str = include_str!("task_queue_prompt.md");

/// Pergunta de confirmação mandada como se fosse o usuário, depois que o
/// modelo termina de trabalhar num item — o texto exato é parte do contrato
/// com `TASK_QUEUE_PROMPT` (os exemplos citam esta frase literalmente).
pub const CONFIRM_QUESTION: &str =
    "Esse item foi feito, posso colocar para testar? Responda: 'feito' se foi feito.";

/// Extrai o array de tarefas de um JSON colado pelo usuário — aceita tanto
/// `{"tarefas": [...]}` quanto um array solto `[...]`. Erro claro quando
/// nenhum dos dois formatos bate, em vez de um erro de parse genérico.
pub fn parse_tasks(json_text: &str) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_str(json_text).map_err(|e| anyhow!("JSON invalido: {e}"))?;
    match value {
        Value::Array(items) => Ok(items),
        Value::Object(mut obj) => match obj.remove("tarefas") {
            Some(Value::Array(items)) => Ok(items),
            Some(_) => Err(anyhow!("o campo \"tarefas\" precisa ser um array")),
            None => Err(anyhow!(
                "JSON sem o campo \"tarefas\" (esperado {{\"tarefas\": [...]}} ou um array solto no topo)"
            )),
        },
        _ => Err(anyhow!("esperado um objeto ou array no topo do JSON")),
    }
}

/// Serializa de volta pro mesmo formato `{"tarefas": [...]}` — sempre nesse
/// formato na gravação (independente de o usuário ter colado um array solto),
/// já que é o formato que `read_task_queue`/`parse_tasks` sempre aceitam de
/// volta sem ambiguidade.
pub fn serialize_tasks(tasks: &[Value]) -> String {
    let wrapped = serde_json::json!({ "tarefas": tasks });
    serde_json::to_string_pretty(&wrapped).unwrap_or_default()
}

fn item_status(item: &Value) -> Option<&str> {
    item.get("status").and_then(|v| v.as_str())
}

fn item_id_display(item: &Value) -> String {
    match item.get("id") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "?".to_string(),
    }
}

/// Texto mandado pro modelo como se fosse a mensagem do usuário pra este
/// item — junta `titulo` (se existir) e `descricao`/`descrição` de forma
/// tolerante a qual chave o usuário usou (o cofre usa "descricao" sem
/// acento na maioria dos arquivos vistos nesta sessão, mas aceita os dois).
pub fn item_prompt_text(item: &Value) -> Result<String> {
    let titulo = item.get("titulo").and_then(|v| v.as_str());
    let descricao = item
        .get("descricao")
        .or_else(|| item.get("descrição"))
        .and_then(|v| v.as_str());
    match (titulo, descricao) {
        (Some(t), Some(d)) => Ok(format!("{t}\n\n{d}")),
        (None, Some(d)) => Ok(d.to_string()),
        (Some(t), None) => Ok(t.to_string()),
        (None, None) => Err(anyhow!(
            "item {} sem \"titulo\" nem \"descricao\" — nada pra mandar pro modelo",
            item_id_display(item)
        )),
    }
}

/// Índice do primeiro item com `status == "fazer"`, na ordem do array —
/// mesma leitura top-to-bottom que o `todo_list` interno já usa.
pub fn find_next_fazer(tasks: &[Value]) -> Option<usize> {
    tasks.iter().position(|t| item_status(t) == Some(STATUS_FAZER))
}

/// Muda o `status` de um item in-place, preservando todos os outros campos
/// do objeto (extras do cofre incluídos) — nunca reconstrói o item do zero.
pub fn set_status(tasks: &mut [Value], index: usize, new_status: &str) {
    if let Some(obj) = tasks.get_mut(index).and_then(|v| v.as_object_mut()) {
        obj.insert("status".to_string(), Value::String(new_status.to_string()));
    }
}

/// `true` quando a resposta do modelo confirma o item — busca a palavra
/// `feito` como token isolado (não substring de outra palavra), sem exigir
/// que seja a resposta INTEIRA: modelos pequenos frequentemente adicionam
/// pontuação ou uma palavra a mais mesmo quando instruídos a não fazer isso
/// (achado da pesquisa 2026-09-24) — checagem tolerante no backend é o que
/// realmente torna isso confiável, não só a instrução no prompt.
pub fn confirms_done(reply: &str) -> bool {
    let lower = reply.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == "feito")
}

#[derive(Serialize, Clone)]
struct TaskQueueEvent {
    session_id: String,
    /// "processing" | "confirmed" | "stuck" | "finished" | "error"
    status: &'static str,
    item_id: Option<String>,
    message: Option<String>,
}

/// Exposto pro chamador (`lib.rs::start_task_queue`) emitir o evento
/// "error" quando `run_queue` retorna `Err` — mantém `TaskQueueEvent`
/// privado (só este arquivo monta o payload) sem duplicar o formato do
/// evento em dois lugares.
pub fn emit_error(app: &AppHandle, session_id: &str, message: String) {
    emit_status(app, session_id, "error", None, Some(message));
}

fn emit_status(app: &AppHandle, session_id: &str, status: &'static str, item_id: Option<String>, message: Option<String>) {
    let _ = app.emit(
        "agent:task_queue_status",
        TaskQueueEvent {
            session_id: session_id.to_string(),
            status,
            item_id,
            message,
        },
    );
}

/// Roda a fila inteira, sem interação: acha o próximo item `fazer`, manda
/// como se fosse o usuário (via `run_turn`, reusando TODO o mecanismo
/// existente — Long Horizon incluso, se a sessão tiver ligado), pergunta se
/// terminou, e só avança pra `testar` se a resposta confirmar. Sem confirmar
/// em `CONFIRM_RETRIES` tentativas, PARA a fila inteira (nunca pula um item
/// sozinho — pedido explícito do usuário, 2026-09-24). Cancelamento é
/// externo: quem chama isto guarda o `JoinHandle` e usa `.abort()` (mesmo
/// mecanismo que `cancel_turn` já usa pra um turno normal) — não há checagem
/// cooperativa de "stop" aqui dentro.
pub async fn run_queue(app: AppHandle, session_id: String) -> Result<()> {
    let state = app.state::<crate::AppState>();
    let app_data_dir = state.app_data_dir.clone();

    loop {
        let tasks_json = sessions::read_task_queue(&app_data_dir, &session_id)?;
        let mut tasks = parse_tasks(&tasks_json)?;

        let Some(idx) = find_next_fazer(&tasks) else {
            emit_status(&app, &session_id, "finished", None, None);
            return Ok(());
        };
        let item_id = item_id_display(&tasks[idx]);
        let prompt_text = item_prompt_text(&tasks[idx])?;

        emit_status(&app, &session_id, "processing", Some(item_id.clone()), None);

        {
            let state = app.state::<crate::AppState>();
            super::run_turn(app.clone(), &state, session_id.clone(), prompt_text, Vec::new(), None).await?;
        }

        let mut confirmed = false;
        for _attempt in 0..CONFIRM_RETRIES {
            {
                let state = app.state::<crate::AppState>();
                super::run_turn(
                    app.clone(),
                    &state,
                    session_id.clone(),
                    CONFIRM_QUESTION.to_string(),
                    Vec::new(),
                    None,
                )
                .await?;
            }
            let messages = sessions::load_messages(&app_data_dir, &session_id)?;
            let last_reply = messages.iter().rev().find(|m| m.role == "assistant");
            if let Some(reply) = last_reply {
                if confirms_done(&reply.content) {
                    confirmed = true;
                    break;
                }
            }
        }

        if !confirmed {
            emit_status(
                &app,
                &session_id,
                "stuck",
                Some(item_id.clone()),
                Some(format!(
                    "O item {item_id} nao foi confirmado como concluido depois de {CONFIRM_RETRIES} tentativas — fila parada, o item continua com status \"fazer\"."
                )),
            );
            return Ok(());
        }

        set_status(&mut tasks, idx, STATUS_TESTAR);
        sessions::write_task_queue_tasks(&app_data_dir, &session_id, &tasks)?;
        emit_status(&app, &session_id, "confirmed", Some(item_id), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tasks_aceita_objeto_com_campo_tarefas() {
        let json = r#"{"tarefas": [{"id": 1, "status": "fazer"}]}"#;
        let tasks = parse_tasks(json).unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn parse_tasks_aceita_array_solto() {
        let json = r#"[{"id": 1, "status": "fazer"}, {"id": 2, "status": "pronto"}]"#;
        let tasks = parse_tasks(json).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn parse_tasks_erro_claro_sem_campo_tarefas() {
        let json = r#"{"outra_coisa": []}"#;
        let err = parse_tasks(json).unwrap_err();
        assert!(err.to_string().contains("tarefas"));
    }

    #[test]
    fn parse_tasks_erro_json_invalido() {
        assert!(parse_tasks("{ nao é json").is_err());
    }

    #[test]
    fn serialize_tasks_sempre_no_formato_com_campo_tarefas() {
        let tasks = vec![json!({"id": 1, "status": "fazer"})];
        let out = serialize_tasks(&tasks);
        assert!(out.contains("\"tarefas\""));
        // roundtrip
        let reparsed = parse_tasks(&out).unwrap();
        assert_eq!(reparsed.len(), 1);
    }

    #[test]
    fn item_prompt_text_junta_titulo_e_descricao() {
        let item = json!({"id": 1, "titulo": "Fazer X", "descricao": "Detalhes de X", "status": "fazer"});
        let text = item_prompt_text(&item).unwrap();
        assert!(text.contains("Fazer X"));
        assert!(text.contains("Detalhes de X"));
    }

    #[test]
    fn item_prompt_text_so_descricao() {
        let item = json!({"id": 1, "descricao": "Detalhes de X", "status": "fazer"});
        assert_eq!(item_prompt_text(&item).unwrap(), "Detalhes de X");
    }

    #[test]
    fn item_prompt_text_aceita_descricao_com_acento() {
        let item = json!({"id": 1, "descrição": "Com acento", "status": "fazer"});
        assert_eq!(item_prompt_text(&item).unwrap(), "Com acento");
    }

    #[test]
    fn item_prompt_text_erro_sem_titulo_nem_descricao() {
        let item = json!({"id": 1, "status": "fazer"});
        assert!(item_prompt_text(&item).is_err());
    }

    #[test]
    fn find_next_fazer_acha_o_primeiro() {
        let tasks = vec![
            json!({"id": 1, "status": "pronto"}),
            json!({"id": 2, "status": "fazer"}),
            json!({"id": 3, "status": "fazer"}),
        ];
        assert_eq!(find_next_fazer(&tasks), Some(1));
    }

    #[test]
    fn find_next_fazer_none_quando_nao_ha_nenhum() {
        let tasks = vec![json!({"id": 1, "status": "pronto"})];
        assert_eq!(find_next_fazer(&tasks), None);
    }

    #[test]
    fn set_status_muda_so_o_status_preserva_o_resto() {
        let mut tasks = vec![json!({"id": 1, "status": "fazer", "descricao": "X", "motivo_bloqueio": null})];
        set_status(&mut tasks, 0, STATUS_TESTAR);
        assert_eq!(tasks[0]["status"], "testar");
        assert_eq!(tasks[0]["descricao"], "X");
        assert!(tasks[0].get("motivo_bloqueio").is_some());
    }

    #[test]
    fn confirms_done_reconhece_resposta_exata() {
        assert!(confirms_done("feito"));
    }

    #[test]
    fn confirms_done_reconhece_com_pontuacao_extra() {
        assert!(confirms_done("Feito."));
        assert!(confirms_done("Sim, feito!"));
    }

    #[test]
    fn confirms_done_nao_confunde_substring() {
        // "malfeito"/"desfeito" nao deveriam contar como confirmacao
        assert!(!confirms_done("Ficou malfeito, preciso refazer"));
        assert!(!confirms_done("desfeito"));
    }

    #[test]
    fn confirms_done_falso_quando_explica_pendencia() {
        assert!(!confirms_done("Não consegui terminar, falta configurar a API."));
    }
}
