//! "Goal mode": verificador adversarial independente, chamado via a tool
//! `verify_completion` quando o agente principal acha que terminou uma
//! tarefa complexa e quer confirmar antes de declarar sucesso pro usuario.
//!
//! Padrao vem do "goal mode" do grok-build (`agent-architecture-research.md`
//! seção 3.3): quando o modelo chama `update_goal(completed: true)`, o
//! harness deles dispara um "painel cetico" de subagentes verificadores
//! independentes que reconferem o trabalho contra o plano original antes de
//! aceitar a conclusao — a peca central sendo uma persona **adversarial**
//! que assume "refutado" por padrao quando incerto, e audita evidencia real
//! (rodar teste/build) em vez de aceitar so a narrativa de quem alega ter
//! terminado.
//!
//! Reduzido de proposito em relacao ao grok-build: sem criterio de aceite
//! "congelado" antes de comecar (`goal_planner_prompt.md`) nem cutucao a
//! cada turno (`goal_continuation_directive.md`) — so a peca que a pesquisa
//! aponta como a que realmente muda o resultado (o veredito adversarial em
//! si, `goal_verifier_prompt.md`), acionada sob demanda pelo proprio modelo
//! via uma tool, nao um "modo" separado com config propria. Reusa a mesma
//! maquina de loop de ferramentas do `subagent.rs` (`task`), so com prompt e
//! toolset diferentes.
//!
//! **Toolset e so leitura/execucao** (`read_file`/`list_dir`/`grep`/
//! `ast_grep`/`run_command`) — sem `write_file`/`edit_file`/`ast_edit`: o
//! verificador so observa e reporta, nunca "conserta" o que encontrar (isso
//! cabe ao agente principal, depois de ouvir o veredito). Mesma guarda de
//! profundidade do sub-agente normal — nada de `task`/`ask`/`verify_completion`
//! recursivo.

use super::tools;
use crate::models::{ChatMessage, ProviderConfig, ToolSpec};
use crate::{providers, AppState};
use anyhow::Result;
use std::path::Path;
use tauri::{AppHandle, Emitter};

const MAX_VERIFIER_STEPS: usize = 8;

/// Ferramentas que o verificador pode usar — so observar/executar, nunca
/// editar. `run_command` fica de proposito (a pesquisa e explicita: o
/// veredito precisa estar condicionado a pelo menos uma execucao real de
/// teste/build/lint, nao so leitura de codigo).
const VERIFIER_ALLOWED_TOOLS: &[&str] =
    &["read_file", "list_dir", "grep", "ast_grep", "run_command"];

const VERIFIER_SYSTEM_PROMPT: &str = "Voce e um VERIFICADOR independente e cetico - NAO a mesma \
entidade que alega ter concluido a tarefa, e seu unico trabalho e confirmar com evidencia real se \
ela foi de fato concluida. Assuma REFUTADO por padrao quando houver qualquer duvida - o onus da \
prova e de quem alega sucesso, nao seu. NUNCA aceite so a narrativa de que algo foi feito: confira \
voce mesmo lendo o codigo (read_file/grep/ast_grep) ou rodando um comando de verdade (run_command \
- teste, build, lint, o que for aplicavel). Se a tarefa envolveu editar arquivo, lembre que \
write_file/edit_file/ast_edit escrevem numa sandbox que precisa ser aceita pelo usuario antes de \
valer pro arquivo real - se voce ler o arquivo real e ele nao refletir a mudanca, isso NAO e prova \
de que a mudanca falhou, pode so estar pendente de aceite; nesse caso, confira o conteudo proposto \
descrito no relato da tarefa em vez de exigir que o arquivo real ja reflita. Voce NAO tem \
write_file/edit_file/ast_edit - so pode observar e reportar, nunca consertar o que encontrar. \
Responda comecando com a palavra EXATA 'APROVADO' ou 'REFUTADO' sozinha na primeira linha, seguida \
da evidencia concreta que embasa o veredito (o que voce leu, ou a saida exata do comando que \
rodou) - sem essa evidencia concreta, o veredito e invalido.";

/// Roda o verificador contra `task_summary`/`how_to_verify` e devolve o
/// veredito formatado (sempre comecando com "APROVADO"/"REFUTADO" na
/// primeira linha, mesmo que o proprio modelo verificador nao tenha seguido
/// o formato — nesse caso vira "REFUTADO" por seguranca, ver
/// [`extract_verdict`]).
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
    task_summary: &str,
    how_to_verify: &str,
) -> Result<String> {
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: VERIFIER_SYSTEM_PROMPT.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Tarefa que o agente principal alega ter concluido:\n{task_summary}\n\n\
                 Como verificar:\n{how_to_verify}\n\n\
                 Confira de verdade (leia o codigo relevante e/ou rode o comando indicado) antes \
                 de dar o veredito."
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
    ];

    let tool_specs = verifier_tool_specs();
    let mut recent_calls: Vec<(String, String)> = Vec::new();

    for step in 0..MAX_VERIFIER_STEPS {
        let assistant = providers::chat_stream(
            app,
            session_id,
            cfg,
            api_key.clone(),
            model,
            &messages,
            &tool_specs,
            // Verificador é chamada utilitária: em locais força Off (senão
            // pensa à toa); em cloud deixa Auto pra não mandar campos que um
            // backend OpenAI estrito rejeitaria.
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

            let _ = app.emit(
                "agent:tool_call",
                super::ToolCallEvent {
                    session_id: session_id.to_string(),
                    tool: format!("🔎 verificador: {}", call.function.name),
                    args: call.function.arguments.clone(),
                },
            );

            // Sem background_jobs/mcp_clients reais de proposito seria mais
            // codigo pra pouco ganho - o verificador reusa os do app, mas
            // seu toolset ja exclui as ferramentas de controle de background
            // (list_background/etc nao estao no allowlist).
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
                    "REFUTADO\n[verificador parou: chamou '{}' {} vezes seguidas com os mesmos \
                     argumentos, sem sinal de progresso - parece um loop, nao deu pra confirmar nada]",
                    call.function.name,
                    super::DOOM_LOOP_THRESHOLD
                ));
            }
        }

        if step + 1 == MAX_VERIFIER_STEPS {
            break;
        }
    }

    Ok(format!(
        "REFUTADO\n[verificador atingiu o limite de {MAX_VERIFIER_STEPS} passos sem dar um veredito \
         claro - por seguranca, trate como nao confirmado]"
    ))
}

/// Le so a primeira linha em busca de "APROVADO"/"REFUTADO" (o prompt pede
/// exatamente isso); se o modelo verificador nao seguir o formato, o
/// veredito vira REFUTADO por seguranca em vez de silenciosamente aprovar
/// algo que nao foi claramente confirmado.
fn extract_verdict(response: &str) -> String {
    let first_line = response.lines().next().unwrap_or("").trim().to_uppercase();
    if first_line.starts_with("APROVADO") {
        response.to_string()
    } else if first_line.starts_with("REFUTADO") {
        response.to_string()
    } else {
        format!("REFUTADO\n[verificador nao devolveu um veredito no formato esperado - resposta original abaixo, trate como nao confirmado]\n{response}")
    }
}

/// So ferramentas de leitura/busca/execucao — sem escrita, sem recursao
/// (`task`/`ask`/`verify_completion` de fora), sem controle de processo em
/// segundo plano (o verificador roda comando sincrono, nao gerencia dev
/// server).
fn verifier_tool_specs() -> Vec<ToolSpec> {
    tools::project_tool_specs()
        .into_iter()
        .filter(|t| VERIFIER_ALLOWED_TOOLS.contains(&t.function.name.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_toolset_is_read_only_allowlist() {
        let specs = verifier_tool_specs();
        let names: Vec<&str> = specs.iter().map(|t| t.function.name.as_str()).collect();
        for expected in VERIFIER_ALLOWED_TOOLS {
            assert!(
                names.contains(expected),
                "verificador deveria ter '{expected}'"
            );
        }
        for forbidden in [
            "write_file",
            "edit_file",
            "ast_edit",
            "task",
            "ask",
            "check_background_output",
            "stop_background",
            "list_background",
        ] {
            assert!(
                !names.contains(&forbidden),
                "verificador NAO deveria ter '{forbidden}'"
            );
        }
    }

    #[test]
    fn extract_verdict_recognizes_aprovado() {
        let verdict = extract_verdict("APROVADO\nrodei cargo test e passou, 12 testes ok");
        assert!(verdict.starts_with("APROVADO"));
    }

    #[test]
    fn extract_verdict_recognizes_refutado() {
        let verdict = extract_verdict("REFUTADO\ncargo test falhou com 2 erros de compilacao");
        assert!(verdict.starts_with("REFUTADO"));
    }

    #[test]
    fn extract_verdict_is_case_insensitive() {
        let verdict = extract_verdict("aprovado\nfoo");
        assert!(verdict.starts_with("aprovado"));
    }

    #[test]
    fn extract_verdict_defaults_to_refutado_when_format_not_followed() {
        let verdict = extract_verdict("Acho que ficou tudo certo, o codigo parece bom.");
        assert!(
            verdict.starts_with("REFUTADO"),
            "sem veredito claro deveria ser tratado como nao confirmado: {verdict}"
        );
    }
}
