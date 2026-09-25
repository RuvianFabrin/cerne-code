//! Modo Long Horizon: contexto limpo por passo + estado em disco, em vez de
//! acumular o histórico da conversa inteira a cada turno.
//!
//! Porta a tese validada no protótipo `F:\longHorizon` (projeto `long-horizon`
//! no cofre, decisões D2/D8) pro Cerne de verdade — ver o projeto
//! `cerne-long-horizon` no cofre pro plano completo (`tarefas.json`).
//!
//! **Duas "memórias", não uma (2026-09-20, pedido do usuário depois de testar ao
//! vivo):** o que o usuário VÊ (`messages`, salvo em `chat_log.json`) é
//! **imutável** — só cresce, nunca é podado, exatamente como qualquer outra
//! sessão do Cerne. O que é MANDADO AO MODELO é uma **visão separada,
//! recalculada a cada chamada** (`build_model_view`), montada a partir do
//! `messages` de verdade mas nunca gravada em disco por conta própria: system
//! prompt atual + o briefing (memoria.md/projeto.md, a fonte de verdade
//! compactada) + tudo que aconteceu DESTE turno em diante. Turnos anteriores
//! nunca entram na visão do modelo, mas continuam no histórico completo pra
//! sempre — a versão anterior deste módulo (até 2026-09-20) podava
//! `messages` de verdade, igual a compactação normal do Cerne já fazia;
//! achado ao vivo testando com o usuário que isso escondia trabalho já feito
//! do scroll da conversa, contrariando o objetivo do modo ("nada escondido").

use crate::models::ChatMessage;
use crate::sessions;
use anyhow::Result;

/// Marca (`ChatMessage.name`) a nota visível que avisa o usuário de que um
/// novo passo do Long Horizon começou — mesmo mecanismo que `background_job_done`/
/// `auto_continue_stopped` já usam (`ChatView.vue`) pra mostrar um aviso do
/// harness na conversa sem fingir que foi o usuário ou o modelo que "disse"
/// aquilo.
pub const RESET_MARKER_NAME: &str = "long_horizon_reset";

/// Monta o texto do briefing a partir do estado em disco desta sessão.
/// String vazia nos dois arquivos (sessão que acabou de ligar o modo) ainda
/// assim produz um briefing válido — só com as seções vazias, o que já é
/// informação (\"nada registrado ainda\").
pub fn build_briefing(app_data_dir: &std::path::PathBuf, session_id: &str) -> Result<String> {
    let memoria = sessions::read_long_horizon_memoria(app_data_dir, session_id)?;
    let projeto = sessions::read_long_horizon_projeto(app_data_dir, session_id)?;

    let mut briefing = String::from(
        "## Estado desta sessão (Long Horizon)\n\n\
         Isto substitui o histórico da conversa — é a ÚNICA coisa que você sabe sobre o que já \
         aconteceu aqui. Não assuma nada além do que está escrito abaixo.\n\n",
    );

    // Achado no turno anterior: se a resposta final se repetiu byte a byte
    // vezes demais (`sessions::finalize_long_horizon_turn`), avisa aqui —
    // é o único jeito do modelo saber, já que o histórico com a repetição em
    // si foi descartado no reset. Guard "generoso" de propósito (2026-09-20,
    // pedido do usuário): avisa em vez de travar o turno, pra não interromper
    // verificação legítima que só PARECE repetição.
    if let Ok(session) = sessions::get_session(app_data_dir, session_id) {
        if session.long_horizon.ultimo_desfecho.as_deref() == Some("estagnado") {
            briefing.push_str(
                "⚠️ **A resposta final do turno anterior se repetiu, idêntica, várias vezes \
                 seguidas** — o que estava sendo tentado não está funcionando. Mude de \
                 abordagem nesta resposta; insistir do mesmo jeito vai produzir o mesmo \
                 resultado de novo.\n\n",
            );
        }
        // Achado ao vivo (2026-09-20, D6 do projeto cerne-long-horizon): o
        // modelo terminou o turno anterior dizendo que ia atualizar
        // memoria.md/projeto.md, mas nunca chamou a ferramenta — só
        // escreveu a intenção em texto. `projeto.md` acima está
        // desatualizado em relação ao trabalho que foi feito.
        if session.long_horizon.ultimo_turno_sem_persistir_estado {
            briefing.push_str(
                "⚠️ **O turno anterior fez trabalho real (chamou ferramenta) mas terminou SEM \
                 chamar `update_long_horizon_memoria`/`update_long_horizon_projeto`** — o \
                 `projeto.md` acima pode estar desatualizado em relação ao que foi feito de \
                 verdade. Antes de continuar, reconstrua o estado real a partir do que você \
                 consegue verificar agora (relendo arquivos, rodando testes) e chame \
                 `update_long_horizon_projeto` para corrigir isso — não repita o erro de só \
                 escrever a intenção sem chamar a ferramenta.\n\n",
            );
        }
    }

    briefing.push_str("### memoria.md\n");
    if memoria.trim().is_empty() {
        briefing.push_str("_(vazio — nada registrado ainda)_\n\n");
    } else {
        briefing.push_str(memoria.trim());
        briefing.push_str("\n\n");
    }

    briefing.push_str("### projeto.md\n");
    if projeto.trim().is_empty() {
        briefing.push_str("_(vazio — nenhum passo dado ainda; é o início da tarefa)_\n");
    } else {
        briefing.push_str(projeto.trim());
        briefing.push('\n');
    }

    Ok(briefing)
}

/// `true` quando existe conversa suficiente ANTES de `turn_start` pra valer
/// a pena avisar o usuário de que um reset está acontecendo — o primeiro
/// turno (ou logo depois de ligar o modo, sem histórico anterior) não tem
/// nada sendo resumido, então não teria sentido mostrar o aviso.
pub fn deve_marcar_reset(messages: &[ChatMessage], turn_start: usize) -> bool {
    let has_system = messages.first().map(|m| m.role == "system").unwrap_or(false);
    let start_idx = if has_system { 1 } else { 0 };
    turn_start > start_idx
}

/// Insere uma nota VISÍVEL (papel `system`, marcada com `RESET_MARKER_NAME`)
/// avisando que o payload mandado ao modelo, a partir daqui, foi substituído
/// pelo estado em disco. **Nunca remove nada de `messages`** — diferente da
/// versão anterior deste módulo, o histórico que o usuário vê só cresce.
pub fn insert_reset_marker(messages: &mut Vec<ChatMessage>) {
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: String::new(),
        tool_calls: None,
        tool_call_id: None,
        name: Some(RESET_MARKER_NAME.to_string()),
        images: Vec::new(),
        display_content: Some(
            "🔄 *Novo passo do Long Horizon: o contexto mandado ao modelo foi resumido pro \
             estado em disco (veja na engrenagem ao lado do \"+\"). Esta conversa continua \
             completa — nada foi apagado daqui.*"
                .to_string(),
        ),
    });
}

/// Monta o payload mandado ao modelo NESTA chamada: o system prompt atual
/// (`messages[0]`), o briefing (a fonte de verdade compactada, lida do
/// disco) e tudo que aconteceu a partir de `turn_start` — nunca turnos
/// anteriores. Não lê nem grava nada em disco sozinha; quem chama já tem o
/// `briefing` pronto (lido uma vez no início do turno, via `build_briefing`).
///
/// Chamada de novo a cada volta do laço de chamadas de ferramenta (é
/// barata: só um slice + clone) — assim reflete as chamadas de ferramenta
/// que já rolaram DENTRO deste turno, sem precisar manter uma segunda lista
/// sincronizada manualmente. O marcador de reset (`RESET_MARKER_NAME`) é só
/// decoração de UI — filtrado daqui, o modelo nunca o vê.
pub fn build_model_view(messages: &[ChatMessage], turn_start: usize, briefing: &str) -> Vec<ChatMessage> {
    let mut view = Vec::new();
    // Um único `system` — alguns templates de chat (ex.: KAT-Coder) recusam
    // a requisição se houver mais de uma mensagem com esse papel, mesmo que
    // ambas venham antes do resto do histórico. Achado ao vivo (2026-09-21):
    // "Jinja Exception: System message must be at the beginning" com dois
    // `system` seguidos.
    let system_prompt = messages.first().filter(|m| m.role == "system");
    let content = match system_prompt {
        Some(system) => format!("{}\n\n{}", system.content, briefing),
        None => briefing.to_string(),
    };
    view.push(ChatMessage {
        role: "system".to_string(),
        content,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        images: Vec::new(),
        display_content: None,
    });
    let inicio = turn_start.min(messages.len());
    view.extend(
        messages[inicio..]
            .iter()
            .filter(|m| m.name.as_deref() != Some(RESET_MARKER_NAME))
            .cloned(),
    );
    view
}

/// Bloco dinâmico injetado no system prompt (depois do texto estático de
/// `DEFAULT_LONG_HORIZON_PROMPT`/`LongHorizonConfig.system_prompt`) — igual
/// em espírito ao bloco de `project_root` que `run_turn` já monta: caminho
/// real, não genérico, porque o texto estático não sabe o id da sessão.
pub fn dynamic_prompt_block() -> &'static str {
    "\n\nUse `update_long_horizon_memoria`/`update_long_horizon_projeto` pra persistir o \
     estado — são as únicas ferramentas que gravam nesses dois arquivos (write_file/edit_file \
     comuns não alcançam essa pasta, é fora do projeto). O conteúdo atual dos dois já vem \
     junto de cada mensagem sua, na seção \"Estado desta sessão\" — não precisa (nem consegue) \
     lê-los com read_file."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions;

    fn scratch_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cerne-long-horizon-test-{}", uuid::Uuid::new_v4()))
    }

    fn make_message(role: &str, content: &str) -> ChatMessage {
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

    #[test]
    fn briefing_com_os_dois_arquivos_vazios_ainda_e_valido() {
        let dir = scratch_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let id = "sessao-sem-estado";

        let briefing = build_briefing(&dir, id).unwrap();
        assert!(briefing.contains("memoria.md"));
        assert!(briefing.contains("projeto.md"));
        assert!(briefing.contains("vazio"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn briefing_inclui_o_conteudo_real_dos_arquivos() {
        let dir = scratch_dir();
        let id = "sessao-com-estado";
        sessions::write_long_horizon_memoria(&dir, id, "erro X já foi corrigido, não repetir").unwrap();
        sessions::write_long_horizon_projeto(&dir, id, "passo 3 de 5, falta escrever os testes").unwrap();

        let briefing = build_briefing(&dir, id).unwrap();
        assert!(briefing.contains("erro X já foi corrigido"));
        assert!(briefing.contains("passo 3 de 5"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn briefing_avisa_quando_o_turno_anterior_estagnou() {
        let dir = scratch_dir();
        let session = sessions::create_session(
            &dir,
            "sessao".to_string(),
            crate::models::ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        sessions::update_long_horizon_enabled(&dir, &session.id, true).unwrap();
        // teto=1: a primeira chamada ja bate estagnado, sem precisar repetir.
        sessions::finalize_long_horizon_turn(&dir, &session.id, "sucesso", Some("x"), 1, false).unwrap();

        let briefing = build_briefing(&dir, &session.id).unwrap();
        assert!(briefing.contains("se repetiu"), "deveria avisar sobre a estagnacao: {briefing}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn briefing_nao_avisa_quando_o_turno_anterior_foi_sucesso_normal() {
        let dir = scratch_dir();
        let session = sessions::create_session(
            &dir,
            "sessao".to_string(),
            crate::models::ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        sessions::update_long_horizon_enabled(&dir, &session.id, true).unwrap();
        sessions::finalize_long_horizon_turn(&dir, &session.id, "sucesso", Some("x"), 5, false).unwrap();

        let briefing = build_briefing(&dir, &session.id).unwrap();
        assert!(!briefing.contains("se repetiu"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn briefing_avisa_quando_o_turno_anterior_trabalhou_sem_persistir_estado() {
        let dir = scratch_dir();
        let session = sessions::create_session(
            &dir,
            "sessao".to_string(),
            crate::models::ProviderKind::Ollama,
            "qwen3.5".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        sessions::update_long_horizon_enabled(&dir, &session.id, true).unwrap();
        sessions::finalize_long_horizon_turn(
            &dir,
            &session.id,
            "sucesso",
            Some("Agora vou atualizar o projeto.md."),
            5,
            true,
        )
        .unwrap();

        let briefing = build_briefing(&dir, &session.id).unwrap();
        assert!(
            briefing.contains("SEM"),
            "deveria avisar que o turno anterior nao persistiu o estado: {briefing}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deve_marcar_reset_e_falso_no_primeiro_turno() {
        let messages = vec![make_message("system", "prompt"), make_message("user", "oi")];
        // turn_start aponta pra "oi" (indice 1) -- so o system antes dele,
        // nada pra resumir ainda.
        assert!(!deve_marcar_reset(&messages, 1));
    }

    #[test]
    fn deve_marcar_reset_e_verdadeiro_com_turno_anterior() {
        let messages = vec![
            make_message("system", "prompt"),
            make_message("user", "primeira mensagem"),
            make_message("assistant", "primeira resposta"),
            make_message("user", "segunda mensagem"),
        ];
        assert!(deve_marcar_reset(&messages, 3));
    }

    #[test]
    fn insert_reset_marker_so_acrescenta_nunca_remove() {
        let mut messages = vec![
            make_message("system", "prompt"),
            make_message("user", "mensagem antiga"),
        ];
        let antes = messages.len();

        insert_reset_marker(&mut messages);

        assert_eq!(messages.len(), antes + 1);
        assert_eq!(messages[0].content, "prompt");
        assert_eq!(messages[1].content, "mensagem antiga");
        assert_eq!(messages.last().unwrap().name.as_deref(), Some(RESET_MARKER_NAME));
    }

    #[test]
    fn build_model_view_inclui_system_briefing_e_so_o_turno_atual() {
        let messages = vec![
            make_message("system", "prompt do sistema"),
            make_message("user", "mensagem antiga 1"),
            make_message("assistant", "resposta antiga 1"),
            make_message("user", "mensagem nova"),
            make_message("assistant", "resposta desta iteracao"),
        ];
        // turno atual comeca em "mensagem nova" (indice 3).
        let view = build_model_view(&messages, 3, "briefing: estado real, nao o historico");

        // Um unico `system` (prompt + briefing juntos) -- alguns templates de
        // chat rejeitam a requisicao com mais de uma mensagem `system`.
        assert_eq!(view.len(), 3); // system(prompt+briefing) + mensagem nova + resposta desta iteracao
        assert_eq!(view[0].role, "system");
        assert!(view[0].content.contains("prompt do sistema"));
        assert!(view[0].content.contains("estado real, nao o historico"));
        assert!(!view.iter().any(|m| m.content.contains("mensagem antiga")));
        assert_eq!(view[1].content, "mensagem nova");
        assert_eq!(view[2].content, "resposta desta iteracao");
    }

    #[test]
    fn build_model_view_nunca_muta_o_historico_original() {
        let mut messages = vec![
            make_message("system", "prompt"),
            make_message("user", "mensagem antiga"),
            make_message("user", "mensagem nova"),
        ];
        let antes = messages.clone();

        let _ = build_model_view(&messages, 2, "briefing qualquer");

        // ChatMessage nao implementa PartialEq -- compara o que importa pro
        // teste (papel + conteudo, na ordem) em vez do struct inteiro.
        let antes_resumo: Vec<(&str, &str)> =
            antes.iter().map(|m| (m.role.as_str(), m.content.as_str())).collect();
        let depois_resumo: Vec<(&str, &str)> =
            messages.iter().map(|m| (m.role.as_str(), m.content.as_str())).collect();
        assert_eq!(depois_resumo, antes_resumo, "build_model_view nao deveria alterar o historico original");
        // e garante que a funcao realmente recebeu &[...], nao &mut -- se
        // compilar com essa chamada logo abaixo, a assinatura esta certa.
        let _ = &mut messages;
    }

    #[test]
    fn build_model_view_filtra_o_marcador_de_reset_do_payload() {
        let mut messages = vec![
            make_message("system", "prompt"),
            make_message("user", "mensagem antiga"),
        ];
        insert_reset_marker(&mut messages);
        messages.push(make_message("user", "mensagem nova"));

        let view = build_model_view(&messages, 2, "briefing");

        assert!(
            !view.iter().any(|m| m.name.as_deref() == Some(RESET_MARKER_NAME)),
            "o marcador de reset e so decoracao de UI, o modelo nao deveria ver"
        );
    }
}
