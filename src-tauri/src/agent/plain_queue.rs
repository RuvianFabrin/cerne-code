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
//! mas fica no arquivo que o usuário escolhe (`plain_queue_path`), separado da
//! fila do LH. A memória da sessão não depende da fila: vale em qualquer sessão.

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

/// Instruções da fila (só entram quando a fila está ligada na sessão).
const QUEUE_PROMPT: &str = "## Fila de tarefas (sem Long Horizon)\n\
Você está executando uma lista de tarefas, uma por vez. Cada tarefa chega como uma mensagem do usuário. \
Quando perguntarem se a tarefa foi feita, responda somente 'feito' se ela foi realmente executada e verificada; \
caso contrário, diga o que falta.";

/// Versão da fila para sessões com o Ctx fixo ativo: o modelo local pode perder o fio do que
/// já fez, então as instruções cortam as conferências repetidas.
const QUEUE_PROMPT_CTX_FIXO: &str = "## Fila de tarefas (sem Long Horizon)\n\
Você está executando uma lista de tarefas, uma por vez. Cada tarefa chega como uma mensagem do usuário. \
Faça só o que a tarefa pede.\n\
- Se o que a tarefa pede já existe e está correto, não refaça: confira o arquivo uma vez e siga.\n\
- Não use o histórico do git (log, show) para provar que a tarefa foi feita; olhe o arquivo e o resultado da verificação.\n\
- Rode a verificação da tarefa uma vez depois da última alteração; só repita se mudar algo.\n\
- Não grave a memória durante a tarefa: a fila pede isso depois, e só com o que for novo.\n\
- Quando perguntarem se a tarefa foi feita, responda somente 'feito' se ela foi realmente executada e verificada; \
caso contrário, diga o que falta.";

/// Instruções da memória da sessão. Entra em TODAS as sessões que têm memória
/// ou a ferramenta, independente da fila.
const MEMORY_PROMPT: &str = "## Memória da sessão\n\
Abaixo está a memória desta sessão, inteira. Ela vale só para esta sessão e nunca é compactada. \
Para alterá-la use `update_session_memory` com o conteúdo COMPLETO (substitui o arquivo inteiro), \
mantendo o que já estava lá. Registre só o que ajuda as próximas vezes nesta sessão: decisões, \
caminhos de arquivos, comandos que funcionaram e erros a não repetir.";

/// Memória com o Ctx fixo ativo: tem limite de tamanho, seções e gravação barata.
const MEMORY_PROMPT_CTX_FIXO: &str = "## Memória da sessão\n\
Abaixo está a memória desta sessão, inteira. Ela vale só para esta sessão e nunca é compactada. \
Ela é enviada em todo turno e tem limite de tamanho, então mantenha-a curta e organizada em seções `## Título`.\n\
Para alterá-la use `update_session_memory`:\n\
- modo \"trocar_secao\" (campo `secao`): atualiza só uma seção do mapa do projeto (arquivos, assinaturas, decisões), sem reescrever o resto. Use quando algo do mapa mudou.\n\
- modo \"acrescentar\": adiciona 1 linha curta ao fim da seção \"Estado recente\". Quando o limite enche, as linhas mais antigas dessa seção são apagadas sozinhas.\n\
- modo \"substituir\": reescreve o arquivo inteiro. Use só para reorganizar ou enxugar.\n\
Registre só o que ajuda as próximas vezes nesta sessão. Não registre o que o código ou o git já mostram. \
Se uma regra da memória contradiz as instruções da fila, siga as da fila e remova essa regra na próxima gravação.";

/// Mandado depois que um item foi confirmado: pede o registro na memória.
fn memory_save_prompt(item_id: &str, ctx_fixo: bool) -> String {
    if ctx_fixo {
        return format!(
            "A tarefa {item_id} foi concluída e marcada como testar. Se você aprendeu algo NOVO e útil para as próximas \
tarefas (decisão, comando que funcionou, erro a não repetir), registre com `{MEMORY_TOOL}` no modo \"acrescentar\", \
em 1 linha curta. Se um arquivo ou uma assinatura do mapa do projeto mudou, atualize só a seção correspondente com \
o modo \"trocar_secao\". Não registre o que o código ou o git já mostram, nem o que a tarefa fez. \
Se nada for novo, não chame a ferramenta e responda 'nada a salvar'."
        );
    }
    format!(
        "A tarefa {item_id} foi concluída e marcada como testar. Agora registre na memória da sessão, \
com `{MEMORY_TOOL}`, somente o que for relevante para as próximas tarefas desta sessão (decisões, \
caminhos de arquivos, comandos que funcionaram, erros a não repetir). Reescreva o conteúdo COMPLETO, \
mantendo o que já estava na memória. Se nada for relevante, não chame a ferramenta e responda 'nada a salvar'."
    )
}

/// Pergunta de confirmação do item. Com o Ctx fixo ativo ela também proíbe verificar de novo e
/// consultar o git, que era onde o modelo local se perdia em conferências repetidas.
fn confirm_message(item_id: &str, prompt_text: &str, ctx_fixo: bool) -> String {
    let regra = if ctx_fixo {
        "Confirme somente o resultado deste item, com base no que foi realmente executado e verificado. Não refaça o trabalho, não rode a verificação de novo, não consulte o git e não regrave a memória só para confirmar: use o resultado que você já tem desta tarefa."
    } else {
        "Confirme somente o resultado deste item, com base no que foi realmente executado e verificado. Não refaça o trabalho nem regrave a memória só para confirmar."
    };
    format!("Item da fila: {item_id}\nDescrição: {prompt_text}\n\n{CONFIRM_QUESTION}\n{regra}")
}

/// Bloco do system prompt com a memória da sessão INTEIRA. Chamado a cada
/// montagem do prompt (início de cada turno), então uma gravação feita por
/// `update_session_memory` aparece no turno seguinte. `ctx_fixo` = o Ctx fixo está ativo na
/// sessão: só então o prompt descreve o limite, as seções e os modos novos.
pub fn memory_prompt_block(app_data_dir: &PathBuf, session_id: &str, ctx_fixo: bool) -> Result<String> {
    let memoria = sessions::read_session_memory(app_data_dir, session_id)?;
    let conteudo = if memoria.trim().is_empty() { "(vazia)" } else { memoria.as_str() };
    if !ctx_fixo {
        return Ok(format!("\n\n{MEMORY_PROMPT}\n\n<memoria_da_sessao>\n{conteudo}\n</memoria_da_sessao>"));
    }
    Ok(format!(
        "\n\n{MEMORY_PROMPT_CTX_FIXO}\nTamanho atual: {} de {} bytes.\n\n<memoria_da_sessao>\n{conteudo}\n</memoria_da_sessao>",
        memoria.len(),
        sessions::SESSION_MEMORY_MAX_BYTES
    ))
}

/// Instruções da fila, anexadas ao prompt só quando a fila está ligada.
pub fn queue_prompt_block(ctx_fixo: bool) -> String {
    format!("\n\n{}", if ctx_fixo { QUEUE_PROMPT_CTX_FIXO } else { QUEUE_PROMPT })
}

/// Ferramenta que altera a memória da sessão. Sem o Ctx fixo ela é a de sempre (reescreve o
/// arquivo inteiro, sem limite); com o Ctx fixo ativo ganha os modos "acrescentar" e
/// "trocar_secao", e o tamanho passa a ser limitado (ver [`sessions::SESSION_MEMORY_MAX_BYTES`]).
pub fn tool_specs(ctx_fixo: bool) -> Vec<ToolSpec> {
    let (description, parameters) = if ctx_fixo {
        (
            "Altera a memória desta sessão, que tem limite de tamanho e é organizada em seções \
`## Título`. Modo \"trocar_secao\" (com `secao`): troca só o corpo de uma seção, para atualizar o mapa do \
projeto. Modo \"acrescentar\": adiciona 1 linha curta à seção \"Estado recente\" (as mais antigas somem \
sozinhas quando enche). Modo \"substituir\" (padrão): reescreve o arquivo inteiro, então `conteudo` deve ser \
a memória COMPLETA e resumida; use só para reorganizar ou enxugar.",
            json!({
                "type": "object",
                "properties": {
                    "conteudo": { "type": "string", "description": "A linha a acrescentar, o novo corpo da seção, ou a memória COMPLETA, conforme o modo" },
                    "modo": { "type": "string", "enum": ["acrescentar", "trocar_secao", "substituir"], "description": "acrescentar = linha em 'Estado recente'; trocar_secao = troca o corpo de uma seção; substituir = reescreve tudo (padrão)" },
                    "secao": { "type": "string", "description": "Título da seção (sem ##). Obrigatório no modo trocar_secao" }
                },
                "required": ["conteudo"]
            }),
        )
    } else {
        (
            "SUBSTITUI o conteúdo inteiro da memória desta sessão. Use só para registrar \
fatos úteis para as próximas tarefas desta mesma sessão. Inclua o conteúdo antigo que ainda vale, \
porque o arquivo é reescrito por completo.",
            json!({
                "type": "object",
                "properties": {
                    "conteudo": { "type": "string", "description": "Conteúdo COMPLETO da memória da sessão" }
                },
                "required": ["conteudo"]
            }),
        )
    };
    vec![ToolSpec {
        kind: "function".to_string(),
        function: ToolFunctionSpec {
            name: MEMORY_TOOL.to_string(),
            description: description.to_string(),
            parameters,
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

/// Arquivo da fila de uma sessão do modo Anything: `.anything/tarefas.json` do projeto. `None` quando o
/// modo está desligado ou a sessão não tem pasta de projeto (a fila segue o arquivo escolhido).
pub fn anything_queue_file(session: &crate::models::Session) -> Option<PathBuf> {
    anything_workspace_dir(session.anything.enabled, crate::anything::session::raiz_de_trabalho(session).as_deref()).map(|d| d.join("tarefas.json"))
}

/// Pasta onde está o `tarefas.json` da fila: a `.anything` do projeto, se o arquivo está lá; senão a
/// pasta do primeiro alvo `planejado` do lote. Sem nada planejado, a `.anything` (a leitura avisa que
/// o arquivo não existe). `None` com o modo desligado ou sem pasta de projeto.
pub fn anything_workspace_dir(enabled: bool, project_root: Option<&str>) -> Option<PathBuf> {
    if !enabled {
        return None;
    }
    let root = PathBuf::from(project_root?);
    let base = crate::anything::workspace::pasta_do_modo(&root);
    if base.join("tarefas.json").is_file() {
        return Some(base);
    }
    Some(crate::anything::lote::proximo_planejado(&root).map(|(_, pasta)| pasta).unwrap_or(base))
}

fn read_tasks(app_data_dir: &PathBuf, session_id: &str, anything: Option<&std::path::Path>) -> Result<String> {
    match anything {
        Some(p) => std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("Não foi possível ler {}: {e}", p.display())),
        None => sessions::read_plain_queue(app_data_dir, session_id),
    }
}

fn write_tasks(app_data_dir: &PathBuf, session_id: &str, anything: Option<&std::path::Path>, tasks: &[serde_json::Value]) -> Result<()> {
    match anything {
        Some(p) => crate::anything::workspace::escrever_atomico(p, &task_queue::serialize_tasks(tasks)),
        None => sessions::write_plain_queue_tasks(app_data_dir, session_id, tasks),
    }
}

/// Confere a ficha de um item de função pelo código: o arquivo existe, passa na validação e é da
/// função do item. Devolve as frases de erro que voltam ao modelo.
fn checar_ficha(root: &std::path::Path, saida: &str, endereco: &str) -> std::result::Result<(), Vec<String>> {
    let caminho = root.join(saida);
    let texto = std::fs::read_to_string(&caminho).map_err(|_| vec![format!("a ficha ainda não existe em {saida}")])?;
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&texto) {
        if v.get("pseudocode").is_some() || v.get("assembly").is_some() || v.get("sha256").is_some() {
            return Err(vec![
                "o arquivo é a saída bruta da ferramenta do motor, não a ficha. Escreva a ficha com as suas palavras, só com os campos pedidos (endereco, nome_original, nome_sugerido, resumo, o_que_faz, constantes, base, confianca, evidencias, pendencias)".to_string(),
            ]);
        }
    }
    let ficha = crate::anything::workspace::parse_ficha(&texto)?;
    if ficha.endereco != endereco {
        return Err(vec![format!("a ficha tem endereco {} mas o item é {endereco}", ficha.endereco)]);
    }
    Ok(())
}

/// Confere a saída de qualquer item do modo Anything, pelo tipo: função (ficha), módulo ou relatório
/// (arquivo Markdown com as seções pedidas).
fn checar_item(root: &std::path::Path, item: &serde_json::Value) -> std::result::Result<(), Vec<String>> {
    let saida = item.get("saida").and_then(|v| v.as_str()).unwrap_or_default();
    if !crate::anything::workspace::saida_segura(saida) {
        return Err(vec![format!("o caminho de saída '{saida}' não é permitido; use funcoes/, modulos/ ou relatorio.md")]);
    }
    match item.get("tipo").and_then(|v| v.as_str()) {
        Some("funcao") => checar_ficha(root, saida, item.get("endereco").and_then(|v| v.as_str()).unwrap_or_default()),
        Some("modulo") => checar_markdown(root, saida, crate::anything::modulos::SECOES_MODULO),
        Some("relatorio") => checar_markdown(root, saida, crate::anything::modulos::SECOES_RELATORIO),
        _ => Err(vec!["tipo de item desconhecido".to_string()]),
    }
}

/// Arquivo Markdown do modo Anything: existe e tem as seções pedidas.
fn checar_markdown(root: &std::path::Path, saida: &str, secoes: &[&str]) -> std::result::Result<(), Vec<String>> {
    let caminho = root.join(saida);
    let texto = std::fs::read_to_string(&caminho).map_err(|_| vec![format!("o arquivo ainda não existe em {saida}")])?;
    let faltando = crate::anything::modulos::secoes_faltando(&texto, secoes);
    if faltando.is_empty() {
        Ok(())
    } else {
        Err(faltando)
    }
}

/// Lê a ficha que o modelo gravou, confere de novo e a registra no `indice.json` (a ficha é reescrita
/// no formato do modo; o índice é o que os resumos de outras funções leem).
fn registrar_no_indice(root: &std::path::Path, saida: &str) -> Result<()> {
    anyhow::ensure!(crate::anything::workspace::saida_segura(saida), "saída insegura: {saida}");
    let texto = std::fs::read_to_string(root.join(saida))?;
    let ficha = crate::anything::workspace::parse_ficha(&texto).map_err(|e| anyhow::anyhow!(e.join("; ")))?;
    crate::anything::workspace::Workspace { root: root.to_path_buf() }.gravar_ficha(&ficha)
}

/// Modo Anything, item de função: o modelo estuda e grava a ficha, e o código confere. Até
/// `CONFIRM_RETRIES` correções; se ainda não passar, o item vai para `bloqueada` com o motivo e a
/// fila segue (diferente das tarefas comuns, que param). Não há memória nem compactação por item:
/// a ficha é a memória do modo.
async fn item_de_funcao(
    app: &AppHandle,
    session_id: &str,
    root: &std::path::Path,
    arquivo: &std::path::Path,
    app_data_dir: &PathBuf,
    tasks: &mut Vec<serde_json::Value>,
    idx: usize,
    prompt: &str,
) -> Result<()> {
    let item_id = tasks[idx].get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
    let saida = tasks[idx].get("saida").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    emit_status(app, session_id, "processing", Some(item_id.clone()), None);
    // Resumo de uma linha de cada função que esta chama (já estudadas): é o que o item precisa saber.
    let chama: Vec<String> = tasks[idx]
        .get("chama")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let linhas = if chama.is_empty() {
        Vec::new()
    } else {
        crate::anything::workspace::Workspace { root: root.to_path_buf() }.resumos(&chama)?
    };
    let tipo = tasks[idx].get("tipo").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let mut texto = if tipo == "funcao" {
        // Modelo pequeno não adivinha o formato da ficha: o prompt completo (regras + passos + exemplo)
        // é montado aqui, e não a descrição curta que o plano gravou.
        use crate::anything::prompts::{preencher, FUNCAO, SISTEMA};
        let objetivo = sessions::get_session(app_data_dir, session_id)
            .ok()
            .and_then(|s| s.anything.goal)
            .filter(|g| !g.trim().is_empty())
            .unwrap_or_else(|| "entender como o programa funciona (sem objetivo específico)".to_string());
        let endereco = tasks[idx].get("endereco").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let nome = tasks[idx].get("nome").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(|| format!("sub_{}", endereco.trim_start_matches("0x")));
        let resumos = if linhas.is_empty() { "nenhuma (esta função não chama outras já estudadas)".to_string() } else { linhas.join("
") };
        let corpo = preencher(FUNCAO, &[("endereco", &endereco), ("nome", &nome), ("objetivo", &objetivo), ("resumos", &resumos)]);
        format!("{SISTEMA}

{corpo}")
    } else {
        let mut t = prompt.to_string();
        if !linhas.is_empty() {
            t.push_str("

Funções que esta chama, já estudadas:
");
            t.push_str(&linhas.join("
"));
        }
        t
    };
    // O item traz `saida` relativa (funcoes/0x....json), e o modelo não sabe a que pasta ela se refere: na
    // prática ele grava na pasta do projeto, e a conferência (que olha a pasta do alvo) não acha a ficha.
    // Por isso o caminho completo vai no texto, e é o único que vale.
    let caminho_completo = root.join(&saida).display().to_string();
    texto.push_str(&format!(
        "\n\nGrave o arquivo exatamente neste caminho completo (crie a pasta se faltar; outro lugar não é conferido):\n{caminho_completo}"
    ));
    run_turn(app, session_id, texto).await?;
    let mut erros;
    for tentativa in 0..=CONFIRM_RETRIES {
        match checar_item(root, &tasks[idx]) {
            Ok(()) => {
                if tasks[idx].get("tipo").and_then(|v| v.as_str()) == Some("funcao") {
                    registrar_no_indice(root, &saida)?;
                }
                task_queue::set_status(tasks, idx, crate::anything::STATUS_PRONTO);
                write_tasks(app_data_dir, session_id, Some(arquivo), tasks)?;
                emit_status(app, session_id, "confirmed", Some(item_id), None);
                return Ok(());
            }
            Err(e) => erros = e,
        }
        if tentativa == CONFIRM_RETRIES {
            let motivo = erros.join("; ");
            task_queue::set_status(tasks, idx, crate::anything::STATUS_BLOQUEADA);
            if let Some(obj) = tasks[idx].as_object_mut() {
                obj.insert("motivo".into(), serde_json::Value::String(motivo.clone()));
            }
            write_tasks(app_data_dir, session_id, Some(arquivo), tasks)?;
            emit_status(app, session_id, "blocked", Some(item_id), Some(format!("Item bloqueado: {motivo}")));
            return Ok(());
        }
        let correcao = format!(
            "A ficha em {caminho_completo} não passou na conferência:\n- {}\nCorrija e grave o arquivo de novo nesse mesmo caminho completo, no formato do modo. Responda gravando o arquivo.",
            erros.join("\n- ")
        );
        run_turn(app, session_id, correcao).await?;
    }
    Ok(())
}

/// Roda a fila inteira sem interação: próximo `fazer` → turno com o item →
/// confirmação "feito" (até `CONFIRM_RETRIES`) → `testar` → salvar memória →
/// compactar → próximo. Para só quando a lista acaba ou quando um item não é
/// confirmado (nunca pula um item). Cancelamento é externo, via `.abort()`.
pub async fn run_queue(app: AppHandle, session_id: String) -> Result<()> {
    let app_data_dir = app.state::<crate::AppState>().app_data_dir.clone();

    loop {
        let sessao = sessions::get_session(&app_data_dir, &session_id)?;
        let arquivo_anything = anything_queue_file(&sessao);
        let tasks_json = read_tasks(&app_data_dir, &session_id, arquivo_anything.as_deref())?;
        let mut tasks = task_queue::parse_tasks(&tasks_json)?;

        let Some(idx) = task_queue::find_next_fazer(&tasks) else {
            // Modo Anything: quando as funções acabaram, a síntese (módulos e relatório) entra na fila.
            if let Some(arquivo) = arquivo_anything.as_deref() {
                let ws_dir = arquivo.parent().unwrap_or(arquivo);
                let ws = crate::anything::workspace::Workspace { root: ws_dir.to_path_buf() };
                let objetivo = sessao.anything.goal.clone().unwrap_or_default();
                match crate::anything::plan::acrescentar_sintese(&ws, &objetivo) {
                    Ok(n) if n > 0 => continue,
                    Ok(_) => {}
                    Err(e) => eprintln!("[anything] síntese não foi acrescentada: {e}"),
                }
                // Lote: este alvo terminou. Se há outro planejado, a fila segue para ele.
                if let Some(root) = crate::anything::session::raiz_de_trabalho(&sessao) {
                    if crate::anything::lote::concluir_alvo(std::path::Path::new(&root), ws_dir) {
                        continue;
                    }
                }
            }
            emit_status(&app, &session_id, "finished", None, None);
            return Ok(());
        };
        let item_id = tasks[idx]
            .get("id")
            .map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))
            .unwrap_or_else(|| "?".to_string());
        let prompt_text = task_queue::item_prompt_text(&tasks[idx])?;
        // Modo Anything, item de função: a ficha é conferida pelo código, não pela pergunta "feito?".
        if let Some(arquivo) = arquivo_anything.as_deref() {
            if matches!(tasks[idx].get("tipo").and_then(|v| v.as_str()), Some("funcao") | Some("modulo") | Some("relatorio")) {
                let ws_dir = arquivo.parent().unwrap_or(arquivo);
                item_de_funcao(&app, &session_id, ws_dir, arquivo, &app_data_dir, &mut tasks, idx, &prompt_text).await?;
                continue;
            }
        }
        // Os textos e a memória "inteligente" só valem com o Ctx fixo ativo na sessão.
        let ctx_fixo = sessions::get_session(&app_data_dir, &session_id)?.fixed_context.enabled;

        emit_status(&app, &session_id, "processing", Some(item_id.clone()), None);
        run_turn(&app, &session_id, prompt_text.clone()).await?;

        let mut confirmed = false;
        let mut confirm_attempts = 0;
        for _attempt in 0..CONFIRM_RETRIES {
            confirm_attempts += 1;
            run_turn(
                &app,
                &session_id,
                confirm_message(&item_id, &prompt_text, ctx_fixo),
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
        write_tasks(&app_data_dir, &session_id, arquivo_anything.as_deref(), &tasks)?;
        emit_status(&app, &session_id, "confirmed", Some(item_id.clone()), None);

        emit_status(&app, &session_id, "saving_memory", Some(item_id.clone()), None);
        run_turn(&app, &session_id, memory_save_prompt(&item_id, ctx_fixo)).await?;

        // Depois de salvar a memória: compacta o contexto do histórico antes da
        // próxima tarefa. A memória da sessão não entra aqui (vai no prompt).
        // Com "Ctx fixo" ligado a compactação não é necessária: o que vai ao
        // modelo já é limitado por envio, então vai direto para o próximo item.
        let ctx_fixo_agora = sessions::get_session(&app_data_dir, &session_id)?.fixed_context.enabled;
        if !ctx_fixo_agora {
            emit_status(&app, &session_id, "compacting", Some(item_id), None);
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
        let block = memory_prompt_block(&dir, "s1", true).unwrap();
        assert!(block.contains("(vazia)"));
        sessions::write_session_memory(&dir, "s1", "Usar Angular 18").unwrap();
        let block = memory_prompt_block(&dir, "s1", true).unwrap();
        assert!(block.contains("Usar Angular 18"));
        assert!(!block.contains("(vazia)"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn ferramenta_usa_o_nome_do_despacho() {
        assert_eq!(tool_specs(true)[0].function.name, MEMORY_TOOL);
    }

    #[test]
    fn ferramenta_tem_os_tres_modos_e_o_campo_secao() {
        let spec = &tool_specs(true)[0].function.parameters;
        assert_eq!(spec["properties"]["modo"]["enum"], json!(["acrescentar", "trocar_secao", "substituir"]));
        assert!(spec["properties"]["secao"].is_object());
        assert_eq!(spec["required"], json!(["conteudo"]));
    }

    use sessions::MemoryMode::{Append, Replace, ReplaceSection};

    fn pasta_temporaria() -> PathBuf {
        std::env::temp_dir().join(format!("cerne-mem-{}", uuid::Uuid::new_v4()))
    }

    fn grava(
        dir: &PathBuf,
        conteudo: &str,
        modo: sessions::MemoryMode,
        secao: Option<&str>,
    ) -> anyhow::Result<sessions::MemoryUpdate> {
        sessions::update_session_memory(dir, "s1", conteudo, modo, secao)
    }

    fn le(dir: &PathBuf) -> String {
        sessions::read_session_memory(dir, "s1").unwrap()
    }

    #[test]
    fn o_modo_e_lido_do_texto_do_modelo() {
        assert_eq!(sessions::MemoryMode::parse(Some("acrescentar")), Append);
        assert_eq!(sessions::MemoryMode::parse(Some(" Acrescentar ")), Append);
        assert_eq!(sessions::MemoryMode::parse(Some("trocar_secao")), ReplaceSection);
        assert_eq!(sessions::MemoryMode::parse(Some("trocar seção")), ReplaceSection);
        assert_eq!(sessions::MemoryMode::parse(Some("substituir")), Replace);
        assert_eq!(sessions::MemoryMode::parse(None), Replace);
        assert_eq!(sessions::MemoryMode::parse(Some("qualquer coisa")), Replace);
    }

    #[test]
    fn acrescentar_cria_a_secao_estado_recente_e_adiciona_no_fim() {
        let dir = pasta_temporaria();
        grava(&dir, "primeira linha", Append, None).unwrap();
        grava(&dir, "  segunda linha  ", Append, None).unwrap();
        assert_eq!(le(&dir), "## Estado recente\nprimeira linha\nsegunda linha");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn acrescentar_nao_mexe_nas_outras_secoes_e_entra_antes_da_proxima() {
        let dir = pasta_temporaria();
        sessions::write_session_memory(&dir, "s1", "## Mapa\nA\n\n## Estado recente\nvelha\n\n## Outra\nB").unwrap();
        grava(&dir, "nova", Append, None).unwrap();
        assert_eq!(le(&dir), "## Mapa\nA\n\n## Estado recente\nvelha\nnova\n\n## Outra\nB");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trocar_secao_so_troca_o_trecho_pedido() {
        let dir = pasta_temporaria();
        sessions::write_session_memory(
            &dir,
            "s1",
            "## Mapa\nantigo 1\nantigo 2\n\n## Estado recente\nlinha\n\n## Regras\nR",
        )
        .unwrap();
        // O título casa sem diferenciar maiúsculas.
        grava(&dir, "novo corpo", ReplaceSection, Some("mapa")).unwrap();
        assert_eq!(le(&dir), "## Mapa\nnovo corpo\n\n## Estado recente\nlinha\n\n## Regras\nR");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trocar_secao_ignora_o_titulo_repetido_no_corpo_e_cria_a_secao_se_nao_existir() {
        let dir = pasta_temporaria();
        sessions::write_session_memory(&dir, "s1", "## Mapa\nA").unwrap();
        grava(&dir, "## Mapa\nB", ReplaceSection, Some("## Mapa")).unwrap();
        assert_eq!(le(&dir), "## Mapa\nB");
        grava(&dir, "conteudo novo", ReplaceSection, Some("AppEstado")).unwrap();
        assert_eq!(le(&dir), "## Mapa\nB\n\n## AppEstado\nconteudo novo");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trocar_secao_sem_o_campo_secao_e_recusado() {
        let dir = pasta_temporaria();
        assert!(grava(&dir, "x", ReplaceSection, None).is_err());
        assert!(grava(&dir, "x", ReplaceSection, Some("  ## ")).is_err());
        assert_eq!(le(&dir), "");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn estado_recente_apaga_as_linhas_mais_antigas_quando_enche() {
        let dir = pasta_temporaria();
        let max = sessions::SESSION_MEMORY_MAX_BYTES;
        let mapa = "m".repeat(max - 900);
        let mut inicial = format!("## Mapa\n{mapa}\n\n## Estado recente\n");
        for i in 0..20 {
            inicial.push_str(&format!("linha antiga {i:02} {}\n", "a".repeat(40)));
        }
        sessions::write_session_memory(&dir, "s1", inicial.trim_end()).unwrap();

        let novas = format!("NOVA-1 {}\nNOVA-2 {}", "n".repeat(300), "n".repeat(300));
        let resultado = grava(&dir, &novas, Append, None).unwrap();

        let texto = le(&dir);
        assert!(texto.len() <= max, "cabe no limite: {} > {max}", texto.len());
        assert_eq!(resultado.bytes, texto.len());
        assert!(resultado.dropped_lines > 0, "apagou linhas antigas");
        assert!(texto.contains(&mapa), "o mapa não é tocado");
        assert!(texto.contains("NOVA-1") && texto.contains("NOVA-2"), "as linhas novas ficam");
        assert!(!texto.contains("linha antiga 00"), "a mais antiga saiu primeiro");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn acrescentar_recusa_se_o_mapa_sozinho_ja_estoura_o_limite() {
        let dir = pasta_temporaria();
        let grande = "z".repeat(sessions::SESSION_MEMORY_MAX_BYTES + 500);
        sessions::write_session_memory(&dir, "s1", &format!("## Mapa\n{grande}")).unwrap();
        let antes = le(&dir);

        let erro = grava(&dir, "linha nova", Append, None).unwrap_err().to_string();
        assert!(erro.contains("trocar_secao") && erro.contains("substituir"), "diz como resolver: {erro}");
        assert_eq!(le(&dir), antes, "nada foi gravado");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn substituir_troca_tudo_e_recusa_o_que_passa_do_limite() {
        let dir = pasta_temporaria();
        grava(&dir, "antigo", Replace, None).unwrap();
        grava(&dir, "novo e completo", Replace, None).unwrap();
        assert_eq!(le(&dir), "novo e completo");

        let grande = "x".repeat(sessions::SESSION_MEMORY_MAX_BYTES + 1);
        let erro = grava(&dir, &grande, Replace, None).unwrap_err().to_string();
        assert!(erro.contains("substituir"), "a mensagem diz como resolver: {erro}");
        assert_eq!(le(&dir), "novo e completo");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn conteudo_vazio_nao_grava_nada() {
        let dir = pasta_temporaria();
        for modo in [Append, Replace, ReplaceSection] {
            assert!(grava(&dir, "   \n ", modo, Some("Mapa")).is_err());
        }
        assert_eq!(le(&dir), "");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn edicao_manual_do_usuario_nao_tem_o_limite_do_agente() {
        let dir = pasta_temporaria();
        let grande = "z".repeat(sessions::SESSION_MEMORY_MAX_BYTES * 4);
        sessions::write_session_memory(&dir, "s1", &grande).unwrap();
        assert_eq!(le(&dir).len(), grande.len());
        // O agente, em cima de uma memória já grande, não consegue acrescentar: tem que resumir.
        assert!(grava(&dir, "linha", Append, None).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn o_prompt_mostra_o_tamanho_atual_o_limite_e_os_tres_modos() {
        let dir = pasta_temporaria();
        sessions::write_session_memory(&dir, "s1", "abc").unwrap();
        let block = memory_prompt_block(&dir, "s1", true).unwrap();
        assert!(block.contains(&format!("Tamanho atual: 3 de {} bytes", sessions::SESSION_MEMORY_MAX_BYTES)));
        for trecho in ["trocar_secao", "acrescentar", "substituir", "Estado recente", "contradiz"] {
            assert!(block.contains(trecho), "o prompt da memória cita {trecho}");
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn memory_save_prompt_pede_so_o_que_e_novo_e_oferece_nada_a_salvar() {
        let texto = memory_save_prompt("7", true);
        for trecho in ["acrescentar", "trocar_secao", "nada a salvar", "NOVO"] {
            assert!(texto.contains(trecho), "o pedido cita {trecho}");
        }
        assert!(!texto.contains("COMPLETO"), "não pede mais a reescrita completa");
    }

    #[test]
    fn sem_ctx_fixo_os_textos_e_a_ferramenta_sao_os_de_sempre() {
        let dir = pasta_temporaria();
        sessions::write_session_memory(&dir, "s1", "abc").unwrap();

        let bloco = memory_prompt_block(&dir, "s1", false).unwrap();
        assert!(bloco.contains("conteúdo COMPLETO"));
        assert!(!bloco.contains("trocar_secao") && !bloco.contains("Tamanho atual"));

        assert!(!queue_prompt_block(false).contains("git"));
        assert!(queue_prompt_block(true).contains("git"));

        let spec = &tool_specs(false)[0].function;
        assert!(spec.description.starts_with("SUBSTITUI"));
        assert!(spec.parameters["properties"].get("modo").is_none());
        assert!(spec.parameters["properties"].get("secao").is_none());

        let salvar = memory_save_prompt("7", false);
        assert!(salvar.contains("COMPLETO") && !salvar.contains("trocar_secao"));

        let original = format!(
            "Item da fila: 7\nDescrição: x\n\n{CONFIRM_QUESTION}\nConfirme somente o resultado deste item, com base no que foi realmente executado e verificado. Não refaça o trabalho nem regrave a memória só para confirmar."
        );
        assert_eq!(confirm_message("7", "x", false), original);
        assert!(confirm_message("7", "x", true).contains("verificação de novo"));
        std::fs::remove_dir_all(dir).ok();
    }
}

#[cfg(test)]
mod anything_checagem_tests {
    use super::*;

    fn raiz_com_ficha(conteudo: Option<&str>) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anything-fila-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(d.join(".anything").join("funcoes")).unwrap();
        if let Some(c) = conteudo {
            std::fs::write(d.join(".anything").join("funcoes").join("0x401000.json"), c).unwrap();
        }
        d
    }

    const FICHA: &str = r#"{"endereco":"0x401000","nome_original":"FUN_1","nome_sugerido":"f","resumo":"faz algo","o_que_faz":"faz","base":"observado","confianca":"alta"}"#;

    #[test]
    fn a_missing_ficha_is_reported_by_path() {
        let d = raiz_com_ficha(None);
        let e = checar_ficha(&d.join(".anything"), "funcoes/0x401000.json", "0x401000").unwrap_err();
        assert!(e[0].contains("ainda não existe"));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn the_raw_tool_output_copied_into_the_file_is_recognised() {
        // Foi o que um modelo local de 32B fez: copiou o JSON bruto da ferramenta no lugar da ficha.
        let bruto = r#"{"function":"sub_1","address":"0x401000","sha256":"ab","pseudocode":"int f(){}","assembly":[]}"#;
        let d = raiz_com_ficha(Some(bruto));
        let e = checar_ficha(&d.join(".anything"), "funcoes/0x401000.json", "0x401000").unwrap_err();
        assert!(e[0].contains("saída bruta da ferramenta"), "{e:?}");
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn the_function_prompt_names_every_field_the_validation_requires() {
        use crate::anything::prompts::FUNCAO;
        for campo in ["endereco", "nome_original", "nome_sugerido", "resumo", "o_que_faz", "base", "confianca"] {
            assert!(FUNCAO.contains(&format!("\"{campo}\"")), "o prompt não explica o campo {campo}");
        }
    }

    #[test]
    fn a_valid_ficha_for_the_right_function_passes() {
        let d = raiz_com_ficha(Some(FICHA));
        assert!(checar_ficha(&d.join(".anything"), "funcoes/0x401000.json", "0x401000").is_ok());
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_ficha_for_another_function_is_refused() {
        let d = raiz_com_ficha(Some(FICHA));
        let e = checar_ficha(&d.join(".anything"), "funcoes/0x401000.json", "0x999999").unwrap_err();
        assert!(e[0].contains("0x999999"));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_broken_ficha_returns_the_validation_messages() {
        let d = raiz_com_ficha(Some(r#"{"endereco":"0x401000"}"#));
        let e = checar_ficha(&d.join(".anything"), "funcoes/0x401000.json", "0x401000").unwrap_err();
        assert!(e.iter().any(|m| m.contains("falta o campo")));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn only_anything_sessions_use_the_project_queue() {
        assert!(anything_workspace_dir(false, Some("C:/projeto")).is_none());
        assert!(anything_workspace_dir(true, None).is_none());
        let d = anything_workspace_dir(true, Some("C:/projeto-que-nao-existe")).unwrap();
        assert!(d.ends_with(".anything"));
    }
}
