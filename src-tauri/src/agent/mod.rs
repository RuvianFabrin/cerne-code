mod ast_tools;
pub mod background;
pub mod computer;
mod subagent;
pub mod tools;
mod verifier;
pub mod walk_cache;
pub mod websearch;

use crate::context;
use crate::models::{
    ChatMessage, ExecutionMode, PendingEdit, ProviderConfig, ProviderKind, Session, TaskItem,
};
use crate::{providers, sessions, skills, AppState};
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter};

const MAX_AGENTIC_STEPS: usize = 50;

/// Quantas chamadas seguidas da MESMA ferramenta com os MESMOS argumentos
/// contam como "o modelo travou num loop" — mesma ideia e valor do
/// `DOOM_LOOP_THRESHOLD` do opencode (`packages/opencode/src/session/
/// processor.ts`, ver seção 2.3.2/6.7 do research doc): olha só as ultimas
/// N chamadas, nao um contador global, entao uma chamada diferente no meio
/// reseta a deteccao. Cerne para e avisa em vez de pedir permissao pra
/// continuar como o opencode faz — ainda nao ha infraestrutura de permissao
/// mid-turn, e "parar e avisar" e mais seguro como default.
const DOOM_LOOP_THRESHOLD: usize = 3;

/// Máximo de nudges (auto-continue) por turno. Valor alto porque o verdadeiro
/// freio é o doom loop detection + o botão stop do usuário. Este limite só
/// existe como safety net contra um modelo que nunca chama ferramentas E
/// nunca diz TAREFA_CONCLUIDA E nunca produz texto "final" (sem indicadores
/// de continuação) — cenário extremamente improvável.
const MAX_NUDGES: usize = 50;

/// Frases que sinalizam que o modelo realmente terminou a tarefa. Se o texto
/// final contiver alguma delas, o loop encerra sem nudge.
const LOOP_BREAKERS: &[&str] = &[
    "TAREFA_CONCLUIDA",
    "Tarefa concluída",
    "tarefa concluida",
    "The task is done",
];

/// Prompt curto injetado como mensagem "user" quando o modelo para sem tool
/// call mas a tarefa parece incompleta. Curto pra economizar tokens.
const NUDGE_PROMPT: &str = "Continue. Use as ferramentas para completar a tarefa. \
Se já terminou tudo, responda apenas: TAREFA_CONCLUIDA.";

/// Heurística: detecta se o texto do modelo indica que ele ia continuar mas
/// parou prematuramente (narrou o próximo passo em vez de executar).
fn looks_incomplete(text: &str) -> bool {
    let lower = text.to_lowercase();
    const INDICATORS: &[&str] = &[
        "agora vou",
        "agora preciso",
        "próximo passo",
        "proximo passo",
        "em seguida",
        "vou chamar",
        "vou executar",
        "vou ler",
        "vou editar",
        "vou criar",
        "vamos",
        "preciso chamar",
        "preciso executar",
        "preciso ler",
        "next step",
        "now i will",
        "now i need",
        "let me",
        "i need to",
        "i'll now",
        "i will now",
    ];
    INDICATORS.iter().any(|i| lower.contains(i))
}

/// Confere se o texto contém um sinal explícito de conclusão.
fn is_task_complete(text: &str) -> bool {
    LOOP_BREAKERS.iter().any(|b| text.contains(*b))
}

/// Confere se as ultimas `DOOM_LOOP_THRESHOLD` chamadas de ferramenta
/// executadas (nome + argumentos brutos, na ordem que rodaram) sao todas
/// identicas. Usado tanto no loop principal quanto no sub-agente (`task`).
fn is_doom_loop(recent_calls: &[(String, String)]) -> bool {
    if recent_calls.len() < DOOM_LOOP_THRESHOLD {
        return false;
    }
    let window = &recent_calls[recent_calls.len() - DOOM_LOOP_THRESHOLD..];
    window.iter().all(|call| call == &window[0])
}

/// Extrai o caminho de arquivo dos argumentos JSON de uma tool call, quando
/// a ferramenta opera em arquivos (read_file, write_file, edit_file, etc.).
fn extract_file_path(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    match tool_name {
        "read_file" | "write_file" | "edit_file" | "ast_edit" | "ast_grep" => {
            args["path"].as_str().map(|s| s.to_string())
        }
        "list_dir" | "grep" => args["path"].as_str().map(|s| s.to_string()),
        _ => None,
    }
}

/// Extrai o texto do comando de tool calls tipo shell, pra UI mostrar um
/// bloco "IN" (terminal) separado do "OUT" (`TaskItem::detail`).
fn extract_command_text(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    match tool_name {
        "run_command" => args["command"].as_str().map(|s| s.to_string()),
        _ => None,
    }
}

/// Conta linhas adicionadas (+) e removidas (-) num unified diff, ignorando
/// os headers (+++/---) e linhas de contexto.
fn count_diff_stats(diff: &str) -> (u32, u32) {
    let mut adds = 0u32;
    let mut dels = 0u32;
    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            adds += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            dels += 1;
        }
    }
    (adds, dels)
}

// Once the running history crosses this fraction of the model's context
// window, older turns get summarized instead of sent verbatim.
const COMPACT_TRIGGER_RATIO: f32 = 0.5;
// Most recent messages that are always kept verbatim, never folded into
// the summary (so the model doesn't lose the immediate thread).
const KEEP_LAST_MESSAGES: usize = 6;

/// Método Fable (github.com/Sahir619/fable-method), embutido no binário e
/// injetado no system prompt só quando o usuário liga o ícone no composer
/// (`Session.fable_method`). Fica fora do catálogo de skills de propósito: em
/// modelos grandes só inflaria o prompt, e a ideia é ser um opt-in pra modelos
/// pequenos/médios que tendem a abandonar tarefas.
const FABLE_METHOD_PROMPT: &str = include_str!("fable_method.md");

const SYSTEM_PROMPT: &str = "Voce e Cerne, um agente de codigo local. Use as ferramentas \
disponiveis para ler, buscar e editar arquivos reais do projeto do usuario e rodar comandos. \
Nunca alegue ter corrigido ou criado algo sem ter chamado a ferramenta correspondente de verdade. \
write_file e edit_file aplicam as alteracoes automaticamente — nao precisa pedir pro usuario \
aceitar diff. Quando a tarefa tiver varios passos, quebre em etapas e continue chamando \
ferramentas ate genuinamente terminar, em vez de parar no meio com uma narracao do que falta. \
Use todo_list pra planejar e mostrar progresso em tarefas com 3+ passos: crie a lista no inicio \
com todos os passos como pending, marque in_progress o que estiver fazendo (no maximo 1 por vez), \
e completed quando terminar. Cada chamada de todo_list SUBSTITUI a lista inteira — mande todos os \
itens sempre. Nao use todo_list pra tarefas simples de 1 passo. A lista aparece visualmente no \
chat do usuario a cada atualizacao. read_file suporta offset (linha inicial, 0-based) e limit \
(max de linhas) pra ler so um trecho de arquivos grandes — use isso pra economizar tokens e \
memoria quando o arquivo for grande. O retorno inclui o total de linhas pra voce saber se precisa \
continuar lendo. Para arquivos pequenos (menos de 2000 linhas) pode ler sem offset/limit. \
web_search e web_fetch estao \
disponiveis mesmo sem projeto associado a sessao; use web_search pra achar fontes e web_fetch \
pra ler uma pagina inteira quando o trecho da busca nao for suficiente, e cite a URL de onde \
tirou cada informacao relevante. Prefira ast_grep a grep, e ast_edit a edit_file, quando a busca \
ou edicao for sobre ESTRUTURA de codigo (uma chamada de funcao, um import, uma declaracao) em vez \
de texto solto - ast_grep/ast_edit casam pela forma da arvore sintatica, entao ignoram espaco, \
quebra de linha e comentario, e nao caem em falso positivo dentro de string/comentario/doc do jeito \
que busca textual cai. Use grep normal quando for busca textual mesmo (nome de variavel em log, \
string arbitraria, etc.) ou quando a linguagem do arquivo nao estiver entre as suportadas por \
ast_grep (bash, c, cpp, csharp, css, dart, elixir, go, haskell, hcl, html, java, javascript, json, \
kotlin, lua, markdown, nix, php, python, ruby, rust, scala, solidity, swift, typescript, tsx, \
yaml). NUNCA chame grep/find/dir/ls/cat/type via run_command - use grep, list_dir e read_file, que \
sao mais rapidos, ja tratam encoding do arquivo automaticamente e nao dependem do shell do sistema. \
Reserve run_command pra o que so um comando de verdade resolve (rodar teste, build, linter, \
instalar dependencia, git). NUNCA chame run_command sincrono (sem background=true) pra dev server, \
watch mode, ou qualquer processo feito pra ficar rodando - a chamada trava esperando o processo \
terminar, e ele nunca termina sozinho. Pra esses casos use run_command com background=true (retorna \
na hora com um id), depois check_background_output(id) pra ver se subiu certo e stop_background(id) \
quando nao precisar mais - por exemplo antes de subir uma versao nova no lugar da antiga. Use \
list_background antes de subir um dev server novo pra checar se ja nao tem um rodando de uma \
sessao anterior. Use a ferramenta task pra delegar uma sub-tarefa que precisa de varias chamadas \
de ferramenta (ler/buscar/editar varios arquivos) mas cujo processo intermediario nao importa pro \
usuario, so o resultado final - por exemplo 'ache todos os usos de X e resuma onde estao' ou \
'implemente a funcao Y seguindo o padrao existente'. Nao use task pra algo que uma unica chamada \
de ferramenta ja resolve, nem pra decisoes que dependem do contexto desta conversa (o sub-agente \
so ve o prompt que voce escrever, nao o historico daqui) - escreva o prompt da task de forma \
autocontida. Use a ferramenta ask quando precisar de uma decisao que so o usuario pode tomar antes \
de continuar - escolher entre abordagens genuinamente diferentes, confirmar uma acao arriscada ou \
irreversivel, ou desambiguar um pedido pouco claro - em vez de assumir uma opcao e seguir sem \
avisar. NAO use ask pra coisa que voce mesmo consegue decidir ou verificar com as outras \
ferramentas (ex: se da pra confirmar checando um arquivo, confira, nao pergunte); a chamada pausa \
o turno esperando resposta, entao use com moderacao. Antes de declarar concluida uma tarefa \
complexa (varios arquivos mexidos, projeto criado do zero, refactor grande), chame \
verify_completion pra um verificador independente confirmar com evidencia real (rodando teste/ \
build, nao so lendo codigo) antes de voce alegar sucesso pro usuario - NAO use isso pra um pedido \
simples que uma unica chamada de ferramenta ja resolve e confirma. Se o veredito vier REFUTADO, \
NAO alegue sucesso - continue trabalhando a partir da evidencia que o verificador trouxe. \
Quando usar computer_use (screenshot, click, type, key, scroll, list_windows, focus_window): voce \
so ve o MONITOR PRIMARIO do usuario. Antes de comecar qualquer automacao de tela, SEMPRE faca um \
screenshot primeiro e descreva o que ve. Se a aplicacao que voce precisa controlar nao estiver \
visivel no monitor primario, use ask para pedir ao usuario: 'Nao vejo a aplicacao [X] no monitor \
primario. Pode move-la para a tela principal?' Nao prossiga sem confirmacao. As coordenadas de \
click sao relativas ao canto superior esquerdo do monitor primario (0,0). Nao tente interagir com \
janelas que estao em outro monitor - peça ao usuario para move-las. IMPORTANTE: como o usuario te \
deu esse pedido conversando DENTRO do proprio Cerne Code, a janela do Cerne Code e quase sempre a \
que esta em primeiro plano no momento - se voce clicar/digitar sem focar a aplicacao alvo antes \
(ex: Outlook, navegador, VS Code), a acao vai cair dentro do proprio Cerne Code, nao na aplicacao \
que o usuario quer controlar. Pra evitar isso, prefira passar `window_title` direto nos parametros \
de computer_use_click/type_text/press_key/scroll (foca a janela automaticamente antes da acao, \
numa chamada so); use computer_use_focus_window(titulo) separadamente so quando quiser focar sem \
agir ainda (ex: antes de um screenshot). Use computer_use_list_windows pra descobrir o titulo \
exato antes. \
\n\n## Regra de Loop\n\
- Continue chamando ferramentas ate a tarefa estar 100% completa.\n\
- NUNCA pare no meio para narrar o que falta. Execute.\n\
- Quando REALMENTE terminar tudo, inclua \"TAREFA_CONCLUIDA\" na sua ultima mensagem.\n\
- Se precisar de informacao do usuario, use a ferramenta ask.";

const COMPACTION_SYSTEM_PROMPT: &str = "Voce resume trechos antigos de uma conversa entre um \
usuario e um agente de codigo, para liberar espaco de contexto. Escreva um resumo denso e factual \
em portugues: objetivo original do usuario, decisoes tomadas, arquivos e comandos ja mexidos, \
erros encontrados e como foram resolvidos, e o que ainda estava pendente. Sem rodeios, sem \
saudacao, direto o conteudo do resumo.";

#[derive(Serialize, Clone)]
struct ToolCallEvent {
    session_id: String,
    tool: String,
    args: String,
}

#[derive(Serialize, Clone)]
struct AskEvent {
    session_id: String,
    id: String,
    question: String,
    options: Vec<String>,
}

/// Emite a pergunta pra UI e suspende ate o usuario responder — a task
/// async do `run_turn`/`subagent::run` fica literalmente parada aqui num
/// `.await`, sem precisar serializar/retomar estado em disco: o canal
/// `oneshot` guarda a "continuacao" da chamada, e `answer_ask` (comando
/// Tauri disparado pela UI) so precisa mandar a resposta por ele.
async fn ask_user(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    question: String,
    options: Vec<String>,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    state
        .pending_questions
        .lock()
        .unwrap()
        .insert(id.clone(), tx);
    let _ = app.emit(
        "agent:ask",
        AskEvent {
            session_id: session_id.to_string(),
            id: id.clone(),
            question,
            options,
        },
    );
    rx.await.map_err(|_| {
        anyhow::anyhow!("pergunta cancelada (sessao ou app encerrado antes de responder)")
    })
}

#[derive(Serialize, Clone)]
struct PermissionEvent {
    session_id: String,
    id: String,
    tool: String,
    args: String,
}

/// Modo "Manual" de execução: pausa antes de rodar QUALQUER tool call (exceto
/// a própria `ask`, que já é uma pausa esperando o usuário) e só prossegue
/// depois de aprovação explícita — mesmo padrão de canal `oneshot` do
/// `ask_user`. `answer_permission` (comando Tauri) é quem manda a resposta.
async fn request_permission(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    tool: String,
    args: String,
) -> Result<bool> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .pending_permissions
        .lock()
        .unwrap()
        .insert(id.clone(), tx);
    let _ = app.emit(
        "agent:permission_request",
        PermissionEvent {
            session_id: session_id.to_string(),
            id: id.clone(),
            tool,
            args,
        },
    );
    rx.await.map_err(|_| {
        anyhow::anyhow!(
            "pedido de permissao cancelado (sessao ou app encerrado antes de responder)"
        )
    })
}

#[derive(Serialize, Clone)]
struct StatusEvent {
    session_id: String,
    status: String,
}

#[derive(Serialize, Clone)]
struct DoneEvent {
    session_id: String,
}

#[derive(Serialize, Clone)]
struct CompactedEvent {
    session_id: String,
    summarized_messages: usize,
}

#[derive(Serialize, Clone)]
struct TurnStatsEvent {
    session_id: String,
    turn: u32,
    elapsed_ms: u64,
    prompt_tokens: u32,
    completion_tokens: u32,
}

pub async fn run_turn(
    app: AppHandle,
    state: &AppState,
    session_id: String,
    user_text: String,
    images: Vec<String>,
    display_text: Option<String>,
) -> Result<()> {
    let app_data_dir = state.app_data_dir.clone();
    let mut session = sessions::get_session(&app_data_dir, &session_id)?;
    let mut messages = sessions::load_messages(&app_data_dir, &session_id)?;

    if messages.is_empty() {
        let project_path: Option<&Path> = session.project_root.as_deref().map(Path::new);
        let catalog = skills::list_skills(&app_data_dir, project_path).unwrap_or_default();
        let mut prompt = SYSTEM_PROMPT.to_string();
        if !catalog.is_empty() {
            prompt.push_str("\n\nSkills disponiveis (chame load_skill(name) pra ler o conteudo completo de uma antes de segui-la):\n");
            for skill in &catalog {
                prompt.push_str(&format!(
                    "- {} ({}): {}\n",
                    skill.name, skill.scope, skill.description
                ));
            }
        }
        if session.fable_method {
            prompt.push_str("\n\n");
            prompt.push_str(FABLE_METHOD_PROMPT);
        }
        if let Some(ref root) = session.project_root {
            prompt.push_str(&format!(
                "\n\nPasta do projeto desta sessao: {root}\n\
                 Use caminhos relativos a essa pasta (resolvidos automaticamente) ou caminhos absolutos dentro dela."
            ));
        }
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: prompt,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        });
    }
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_text,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        images,
        display_content: display_text,
    });
    sessions::save_messages(&app_data_dir, &session_id, &messages)?;

    // Quantas mensagens do usuario existem agora (incluindo a que acabou de
    // ser adicionada) - usado pra marcar em qual "turno" cada TaskItem deste
    // run_turn nasceu, pra intercalar os passos na timeline do chat.
    let turn = messages.iter().filter(|m| m.role == "user").count() as u32;

    if session.provider == ProviderKind::LlamaCpp {
        let fork_id = session
            .llama_fork
            .clone()
            .unwrap_or_else(|| state.config.lock().unwrap().active_llama_fork.clone());
        let _ = app.emit(
            "agent:status",
            StatusEvent {
                session_id: session_id.clone(),
                status: "starting_server".to_string(),
            },
        );
        crate::ensure_llama_ready(state, &fork_id)
            .await
            .map_err(|e| {
                anyhow::anyhow!("nao foi possivel subir o llama-server ({fork_id}): {e}")
            })?;
    }

    let (cfg, api_key) = provider_config_for(
        &session.provider,
        state,
        session.custom_provider_id.as_deref(),
    )?;
    let mut tool_specs = tools::always_tool_specs();
    if session.project_root.is_some() {
        tool_specs.extend(tools::project_tool_specs());
    }
    let mcp_servers = crate::mcp::load_servers(&app_data_dir).unwrap_or_default();
    tool_specs.extend(state.mcp_clients.tool_specs(&mcp_servers).await);

    let has_vision = providers::supports_vision(&cfg, api_key.clone(), &session.model, &app_data_dir).await;
    if has_vision {
        tool_specs.extend(computer::tool_specs());
    } else {
        // Modelo sem visão: remove imagens do histórico pra não enviar
        // multimodal data que o provider rejeitaria (400 Bad Request). As
        // tools de computer_use que NÃO dependem de screenshot (list_windows,
        // focus_window, authorize, browser_execute, AX-tree) continuam
        // disponíveis — dá pra automatizar tela via árvore de acessibilidade
        // sem nunca precisar "ver" um pixel.
        tool_specs.extend(
            computer::tool_specs()
                .into_iter()
                .filter(|s| !computer::requires_vision(&s.function.name)),
        );
        for msg in &mut messages {
            msg.images.clear();
        }
    }

    let provider_ctx_override = cfg.context_length_override;
    let context_length = session.context_length.unwrap_or_else(|| {
        providers::resolve_context_length(&app_data_dir, &session.model, provider_ctx_override)
    });
    let is_estimated_length = session.context_length.is_none();
    if session.context_length.is_none() {
        providers::save_context_length(&app_data_dir, &session.model, context_length);
    }

    let mut tasks = sessions::load_tasks(&app_data_dir, &session_id)?;
    let mut recent_calls: Vec<(String, String)> = Vec::new();
    let mut nudge_count: usize = 0;
    let mut force_tool_choice: bool = false;
    let mut tool_steps: usize = 0;
    let turn_start = std::time::Instant::now();
    let mut turn_prompt_tokens: u32 = 0;
    let mut turn_completion_tokens: u32 = 0;

    'steps: loop {
        if tool_steps >= MAX_AGENTIC_STEPS {
            break;
        }
        if maybe_compact(
            &app,
            &session_id,
            &cfg,
            api_key.clone(),
            &session.model,
            &mut messages,
            context_length,
        )
        .await?
        {
            sessions::save_messages(&app_data_dir, &session_id, &messages)?;
        }
        emit_context_usage(
            &app,
            &session_id,
            &messages,
            context_length,
            is_estimated_length,
            &session,
        );

        let _ = app.emit(
            "agent:status",
            StatusEvent {
                session_id: session_id.clone(),
                status: "thinking".to_string(),
            },
        );

        let stream_result = providers::chat_stream(
            &app,
            &session_id,
            &cfg,
            api_key.clone(),
            &session.model,
            &messages,
            &tool_specs,
            session.reasoning_effort,
            if force_tool_choice { Some("required") } else { None },
        )
        .await?;

        if stream_result.usage.prompt_tokens > 0 || stream_result.usage.completion_tokens > 0 {
            turn_prompt_tokens += stream_result.usage.prompt_tokens;
            turn_completion_tokens += stream_result.usage.completion_tokens;
            if let Ok(updated) = sessions::accumulate_usage(
                &app_data_dir,
                &session_id,
                stream_result.usage.prompt_tokens,
                stream_result.usage.completion_tokens,
            ) {
                session = updated;
            }
        }

        let assistant = stream_result.message;
        let has_tool_calls = assistant
            .tool_calls
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);
        messages.push(assistant.clone());
        sessions::save_messages(&app_data_dir, &session_id, &messages)?;

        if !has_tool_calls {
            // Auto-continue (nudge): se o modelo parou sem tool call mas a
            // tarefa parece incompleta, injeta um "continue" e volta pro loop.
            let text = &assistant.content;
            if is_task_complete(text) || !looks_incomplete(text) || nudge_count >= MAX_NUDGES {
                break;
            }
            nudge_count += 1;
            force_tool_choice = true;
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: NUDGE_PROMPT.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: Some("⏳ Continuando automaticamente...".to_string()),
            });
            sessions::save_messages(&app_data_dir, &session_id, &messages)?;
            continue;
        }

        // Modelo chamou ferramentas — reseta o flag de nudge e conta o step.
        force_tool_choice = false;
        tool_steps += 1;

        let project_path: Option<&Path> = session.project_root.as_deref().map(Path::new);

        for call in assistant.tool_calls.iter().flatten() {
            let args: serde_json::Value =
                serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);

            let _ = app.emit(
                "agent:tool_call",
                ToolCallEvent {
                    session_id: session_id.clone(),
                    tool: call.function.name.clone(),
                    args: call.function.arguments.clone(),
                },
            );

            let task_id = call.id.clone();
            let task_idx = tasks.len();
            let file_path = extract_file_path(&call.function.name, &args);
            let command = extract_command_text(&call.function.name, &args);
            let task_started = std::time::Instant::now();
            tasks.push(TaskItem {
                id: task_id.clone(),
                label: format!(
                    "{}({})",
                    call.function.name,
                    truncate(&call.function.arguments, 80)
                ),
                status: "running".to_string(),
                detail: None,
                turn,
                file_path,
                additions: 0,
                deletions: 0,
                started_at_ms: chrono::Utc::now().timestamp_millis() as u64,
                duration_ms: None,
                command,
            });
            sessions::save_tasks(&app_data_dir, &session_id, &tasks)?;

            // Modo "Manual": toda tool call pausa esperando aprovacao antes
            // de rodar, exceto a propria `ask` (ja e uma pausa esperando o
            // usuario, pedir permissao pra perguntar seria so redundante).
            let approved =
                if session.execution_mode == ExecutionMode::Manual && call.function.name != "ask" {
                    request_permission(
                        &app,
                        state,
                        &session_id,
                        call.function.name.clone(),
                        call.function.arguments.clone(),
                    )
                    .await?
                } else {
                    true
                };

            let mut tool_images: Vec<String> = Vec::new();
            let result = if !approved {
                Err(anyhow::anyhow!("Ação negada pelo usuário."))
            } else if call.function.name == "load_skill" {
                match args["name"].as_str() {
                    Some(skill_name) => {
                        skills::load_skill_body(&app_data_dir, project_path, skill_name).map(
                            |body| tools::ToolOutcome {
                                observation: body,
                                pending_edit: None,
                            },
                        )
                    }
                    None => Err(anyhow::anyhow!("name obrigatorio")),
                }
            } else if call.function.name == "task" {
                // Tratado a parte, igual load_skill: precisa de app/estado/
                // provider que tools::execute_tool nao recebe (e nao devia
                // precisar receber, so essa ferramenta usa isso).
                match (project_path, args["prompt"].as_str()) {
                    (Some(project_root), Some(prompt)) => {
                        let description = args["description"].as_str().unwrap_or("sub-tarefa");
                        subagent::run(
                            &app,
                            state,
                            &session_id,
                            &cfg,
                            api_key.clone(),
                            &session.model,
                            project_root,
                            &session.extra_read_paths,
                            description,
                            prompt,
                        )
                        .await
                        .map(|report| tools::ToolOutcome {
                            observation: report,
                            pending_edit: None,
                        })
                    }
                    (None, _) => Err(anyhow::anyhow!(
                        "task precisa de uma pasta de projeto associada a sessao"
                    )),
                    (_, None) => Err(anyhow::anyhow!("prompt obrigatorio")),
                }
            } else if call.function.name == "ask" {
                // Tratado a parte igual load_skill/task: precisa suspender
                // esperando o usuario responder, o que exige app/estado que
                // tools::execute_tool nao tem (e nao devia ter, so essa
                // ferramenta usa isso).
                match args["question"].as_str() {
                    Some(question) => {
                        let options = args["options"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        ask_user(&app, state, &session_id, question.to_string(), options)
                            .await
                            .map(|answer| tools::ToolOutcome {
                                observation: answer,
                                pending_edit: None,
                            })
                    }
                    None => Err(anyhow::anyhow!("question obrigatorio")),
                }
            } else if call.function.name == "todo_list" {
                let todos_json = args["todos"].clone();
                let _ = app.emit(
                    "agent:todo_update",
                    serde_json::json!({
                        "session_id": session_id,
                        "todos": todos_json,
                    }),
                );
                Ok(tools::ToolOutcome {
                    observation: "Lista de tarefas atualizada.".to_string(),
                    pending_edit: None,
                })
            } else if call.function.name == "verify_completion" {
                // Tratado a parte igual task: dispara seu proprio loop de
                // ferramentas (com prompt/toolset diferentes, ver verifier.rs),
                // precisa de app/estado/provider que tools::execute_tool nao tem.
                match (
                    project_path,
                    args["task_summary"].as_str(),
                    args["how_to_verify"].as_str(),
                ) {
                    (Some(project_root), Some(summary), Some(how)) => verifier::run(
                        &app,
                        state,
                        &session_id,
                        &cfg,
                        api_key.clone(),
                        &session.model,
                        project_root,
                        &session.extra_read_paths,
                        summary,
                        how,
                    )
                    .await
                    .map(|verdict| tools::ToolOutcome {
                        observation: verdict,
                        pending_edit: None,
                    }),
                    (None, _, _) => Err(anyhow::anyhow!(
                        "verify_completion precisa de uma pasta de projeto associada a sessao"
                    )),
                    _ => Err(anyhow::anyhow!("task_summary e how_to_verify obrigatorios")),
                }
            } else if call.function.name.starts_with("computer_use_") {
                if computer::requires_vision(&call.function.name) && !has_vision {
                    Err(anyhow::anyhow!(
                        "computer_use requer um modelo com suporte a visao. O modelo atual nao suporta imagens."
                    ))
                } else {
                    computer::execute(&call.function.name, &args, &app_data_dir).await.map(|outcome| {
                        tool_images = outcome
                            .screenshot_base64
                            .map(|b64| vec![format!("data:image/png;base64,{b64}")])
                            .unwrap_or_default();
                        tools::ToolOutcome {
                            observation: outcome.text,
                            pending_edit: None,
                        }
                    })
                }
            } else {
                tools::execute_tool(
                    &call.function.name,
                    &args,
                    project_path,
                    &session.extra_read_paths,
                    &state.background_jobs,
                    &state.mcp_clients,
                    &state.app_data_dir,
                    &session.execution_mode,
                )
                .await
            };

            let observation = match &result {
                Ok(outcome) => outcome.observation.clone(),
                Err(e) => format!("erro executando ferramenta: {e}"),
            };

            if let Ok(outcome) = &result {
                if let Some((target_path, sandbox_path, diff, is_new_file, already_applied)) =
                    &outcome.pending_edit
                {
                    let edit = PendingEdit {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        target_path: target_path.clone(),
                        sandbox_path: sandbox_path.clone(),
                        diff: diff.clone(),
                        is_new_file: *is_new_file,
                        already_applied: *already_applied,
                    };
                    // YOLO (already_applied): ja escrito direto, nao entra na
                    // lista persistente - nao ha accept/reject pra tirar de
                    // la depois, entao ficaria "zumbi" pra sempre em
                    // list_pending_edits (accept_edit/reject_edit sao os
                    // unicos pontos que removem do mapa). So invalida o cache.
                    // Auto/Manual: fica na sandbox esperando o usuario aceitar.
                    if *already_applied {
                        walk_cache::invalidate(std::path::Path::new(target_path));
                    } else {
                        state
                            .pending_edits
                            .lock()
                            .unwrap()
                            .insert(edit.id.clone(), edit.clone());
                    }
                    let _ = app.emit("agent:pending_edit", edit);

                    // Preenche stats de diff no TaskItem pra UI mostrar +N/-N.
                    if let Some(task) = tasks.get_mut(task_idx) {
                        let (adds, dels) = count_diff_stats(diff);
                        task.additions = adds;
                        task.deletions = dels;
                    }
                }
            }

            if let Some(task) = tasks.get_mut(task_idx) {
                task.status = if result.is_ok() {
                    "done".to_string()
                } else {
                    "failed".to_string()
                };
                // Cap generoso (nao os ~200 chars antigos) pra UI poder
                // mostrar as ultimas linhas completas e permitir expandir
                // o bloco "OUT" (ver TaskStepGroup.vue).
                task.detail = Some(truncate(&observation, 6000));
                task.duration_ms = Some(task_started.elapsed().as_millis() as u64);
            }
            sessions::save_tasks(&app_data_dir, &session_id, &tasks)?;

            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: observation,
                tool_calls: None,
                tool_call_id: Some(call.id.clone()),
                name: Some(call.function.name.clone()),
                images: tool_images,
                display_content: None,
            });

            recent_calls.push((call.function.name.clone(), call.function.arguments.clone()));
            if is_doom_loop(&recent_calls) {
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: format!(
                        "⚠️ Parei a execução: chamei `{}` {DOOM_LOOP_THRESHOLD} vezes seguidas com os mesmos \
                         argumentos, sem sinal de progresso — parece um loop. Me diga como prosseguir ou \
                         reformule o pedido.",
                        call.function.name
                    ),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                    images: Vec::new(),
            display_content: None,
                });
                sessions::save_messages(&app_data_dir, &session_id, &messages)?;
                let _ = app.emit(
                    "agent:status",
                    StatusEvent {
                        session_id: session_id.clone(),
                        status: "loop_detectado".to_string(),
                    },
                );
                break 'steps;
            }
        }
        sessions::save_messages(&app_data_dir, &session_id, &messages)?;
    }

    emit_context_usage(
        &app,
        &session_id,
        &messages,
        context_length,
        is_estimated_length,
        &session,
    );

    let _ = app.emit(
        "agent:turn_stats",
        TurnStatsEvent {
            session_id: session_id.clone(),
            turn,
            elapsed_ms: turn_start.elapsed().as_millis() as u64,
            prompt_tokens: turn_prompt_tokens,
            completion_tokens: turn_completion_tokens,
        },
    );

    let _ = app.emit(
        "agent:done",
        DoneEvent {
            session_id: session_id.clone(),
        },
    );
    Ok(())
}

fn emit_context_usage(
    app: &AppHandle,
    session_id: &str,
    messages: &[ChatMessage],
    context_length: u32,
    is_estimated_length: bool,
    session: &Session,
) {
    let usage = context::usage_for(
        session_id,
        messages,
        context_length,
        is_estimated_length,
        session.total_prompt_tokens,
        session.total_completion_tokens,
        session.total_requests,
    );
    let _ = app.emit("agent:context", usage);
}

/// If the running history is past `COMPACT_TRIGGER_RATIO` of the model's
/// context window, folds everything except the system prompt and the last
/// `KEEP_LAST_MESSAGES` messages into a single summary (one extra LLM call,
/// same provider/model). Returns whether it actually compacted anything.
async fn maybe_compact(
    app: &AppHandle,
    session_id: &str,
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    messages: &mut Vec<ChatMessage>,
    context_length: u32,
) -> Result<bool> {
    let has_system = messages
        .first()
        .map(|m| m.role == "system")
        .unwrap_or(false);
    let start_idx = if has_system { 1 } else { 0 };

    if messages.len() < start_idx + KEEP_LAST_MESSAGES + 2 {
        return Ok(false); // not enough history to bother
    }

    let estimate = context::estimate_messages_tokens(messages);
    if (estimate as f32) < (context_length as f32) * COMPACT_TRIGGER_RATIO {
        return Ok(false);
    }

    let compactable = &messages[start_idx..messages.len() - KEEP_LAST_MESSAGES];
    if compactable.is_empty() {
        return Ok(false);
    }

    let transcript = compactable
        .iter()
        .map(|m| match m.role.as_str() {
            "tool" => format!(
                "[ferramenta {}] {}",
                m.name.as_deref().unwrap_or("?"),
                m.content
            ),
            other => format!("[{other}] {}", m.content),
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let summary_messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: COMPACTION_SYSTEM_PROMPT.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: transcript,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
    ];

    // Streamed via a synthetic session id so the summarization tokens never
    // leak into the visible chat (the frontend only listens on the real id).
    let compaction_channel = format!("{session_id}::compact");
    let summary = providers::chat_stream(
        app,
        &compaction_channel,
        cfg,
        api_key,
        model,
        &summary_messages,
        &[],
        // Compactação é chamada utilitária: em locais força Off (senão pensa
        // à toa); em cloud deixa Auto pra não mandar campos que um backend
        // OpenAI estrito rejeitaria.
        cfg.kind.default_reasoning_effort(),
        None,
    )
    .await?
    .message;

    let mut new_messages: Vec<ChatMessage> = messages[..start_idx].to_vec();
    new_messages.push(ChatMessage {
        role: "system".to_string(),
        content: format!(
            "[Resumo do que aconteceu antes deste ponto na sessao]\n{}",
            summary.content
        ),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        images: Vec::new(),
        display_content: None,
    });
    new_messages.extend_from_slice(&messages[messages.len() - KEEP_LAST_MESSAGES..]);

    let summarized_count = compactable.len();
    *messages = new_messages;

    let _ = app.emit(
        "agent:context_compacted",
        CompactedEvent {
            session_id: session_id.to_string(),
            summarized_messages: summarized_count,
        },
    );

    Ok(true)
}

/// Fina camada sobre `crate::build_provider_config` — mesma lógica (incluindo
/// o caminho de provider customizado, ver `providers::custom`), só adaptada
/// pra receber `&AppState` em vez de um `State<AppState>` do Tauri.
pub(crate) fn provider_config_for(
    kind: &ProviderKind,
    state: &AppState,
    custom_provider_id: Option<&str>,
) -> Result<(ProviderConfig, Option<String>)> {
    let config = state.config.lock().unwrap().clone();
    crate::build_provider_config(*kind, &config, &state.app_data_dir, custom_provider_id)
        .map_err(|e| anyhow::anyhow!(e))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}...", s.chars().take(max).collect::<String>())
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &str) -> (String, String) {
        (name.to_string(), args.to_string())
    }

    #[test]
    fn no_loop_below_threshold() {
        let calls = vec![
            call("grep", "{\"pattern\":\"foo\"}"),
            call("grep", "{\"pattern\":\"foo\"}"),
        ];
        assert!(!is_doom_loop(&calls), "so 2 repeticoes, threshold e 3");
    }

    #[test]
    fn detects_same_tool_same_args_three_times_in_a_row() {
        let calls = vec![
            call("edit_file", "{\"path\":\"a.rs\",\"old_str\":\"x\"}"),
            call("edit_file", "{\"path\":\"a.rs\",\"old_str\":\"x\"}"),
            call("edit_file", "{\"path\":\"a.rs\",\"old_str\":\"x\"}"),
        ];
        assert!(is_doom_loop(&calls));
    }

    #[test]
    fn does_not_flag_same_tool_with_different_args() {
        let calls = vec![
            call("read_file", "{\"path\":\"a.rs\"}"),
            call("read_file", "{\"path\":\"b.rs\"}"),
            call("read_file", "{\"path\":\"c.rs\"}"),
        ];
        assert!(
            !is_doom_loop(&calls),
            "argumentos diferentes nao sao um loop, sao progresso real"
        );
    }

    #[test]
    fn a_different_call_in_between_resets_the_window() {
        // repete 2x, faz outra coisa, repete so 1x de novo - nao bate o
        // threshold de 3 seguidas iguais no final.
        let calls = vec![
            call("grep", "{\"pattern\":\"foo\"}"),
            call("grep", "{\"pattern\":\"foo\"}"),
            call("read_file", "{\"path\":\"a.rs\"}"),
            call("grep", "{\"pattern\":\"foo\"}"),
        ];
        assert!(
            !is_doom_loop(&calls),
            "so olha a JANELA final, uma chamada diferente no meio deveria resetar"
        );
    }

    #[test]
    fn only_checks_the_trailing_window_not_the_whole_history() {
        // As 3 primeiras sao iguais (bateria loop se estivessem no final),
        // mas a ULTIMA e diferente - a janela final (as ultimas 3) tem uma
        // diferente, entao nao deveria ser loop.
        let calls = vec![
            call("grep", "{\"pattern\":\"foo\"}"),
            call("grep", "{\"pattern\":\"foo\"}"),
            call("grep", "{\"pattern\":\"foo\"}"),
            call("grep", "{\"pattern\":\"bar\"}"),
        ];
        assert!(!is_doom_loop(&calls));
    }
}
