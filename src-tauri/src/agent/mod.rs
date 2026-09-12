mod analyst;
mod ast_tools;
pub mod background;
pub mod computer;
#[cfg(target_os = "linux")]
mod computer_atspi;
#[cfg(target_os = "linux")]
mod computer_wayland;
mod pipeline;
pub mod shell;
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
use tauri::{AppHandle, Emitter, Manager};

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

/// Ferramentas de LEITURA de status que esperam ser chamadas repetidas
/// vezes com os MESMOS argumentos enquanto o modelo espera algo terminar
/// (polling) — isso e uso normal, nao um loop travado. T39 (2026-08-15):
/// `check_background_output({"id": "..."})` chamado 3x seguidas enquanto um
/// job em background ainda rodava disparava o doom loop e abortava o turno
/// à toa. Ficam de fora da janela de deteccao (`recent_calls`) - ferramentas
/// que MUDAM estado continuam sujeitas ao threshold normal.
const DOOM_LOOP_EXEMPT_TOOLS: &[&str] = &[
    "check_background_output",
    "list_background",
    "check_agent_session",
    "list_agent_sessions",
];

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

/// Fase G: timing de uma sessão orquestrada (`start_agent_session`), pra
/// `check_agent_session` sugerir quanto esperar antes de checar de novo
/// (G2 do roteiro — pedido explícito do usuário: basear a espera em quanto
/// a primeira resposta real levou, não num número fixo arbitrário).
/// `first_response_ms` é a duração do primeiro turno inteiro (não o
/// instante exato da primeira mensagem assistant dentro dele — pegar isso
/// exigiria instrumentar `run_turn` por dentro; essa aproximação já cobre o
/// pedido, já que só existe UM turno rodando logo após a criação).
pub struct OrchestratedSessionInfo {
    pub started_at_ms: u64,
    pub first_response_ms: Option<u64>,
    /// Quando `check_agent_session` foi chamado por ultima vez pra essa
    /// sessao - usado pra impor uma espera minima de verdade (nao so uma
    /// sugestao em texto) quando o modelo ignora o hint e fica chamando em
    /// loop apertado (achado testando ao vivo: modelo local pequeno
    /// (qwen3.5-9b) simplesmente ignorou a sugestao textual e chamou a
    /// ferramenta centenas de vezes seguidas sem pausa, sobrecarregando o
    /// router do llama.cpp).
    pub last_checked_ms: Option<u64>,
    /// Quantas vezes `check_agent_session` ja foi chamado pra essa sessao
    /// enquanto ainda rodando - depois de `MAX_ORCHESTRATED_POLLS` paramos
    /// de so esperar e mandamos uma instrucao explicita pro modelo desistir
    /// de checar e avisar o usuario, pra nao ficar preso num loop pra sempre
    /// se a sessao filha travar de verdade.
    pub poll_count: u32,
}

const MAX_ORCHESTRATED_POLLS: u32 = 8;

/// Fase G: dispara o turno de uma sessão orquestrada em segundo plano —
/// extraído pra função separada (não-async) em vez de inline dentro de
/// `run_turn`, porque `run_turn` chamando a si mesma diretamente dentro do
/// próprio corpo (`async move { ... run_turn(...).await ... }` inline) cria
/// um ciclo que o rustc não consegue provar `Send` (a análise de auto-trait
/// de um `impl Future` auto-referente trava — erro real encontrado
/// implementando isso: "future cannot be sent between threads safely",
/// mesmo todo dado capturado sendo `Send`). Isolando numa função comum
/// (não-`async fn`, só devolve o `JoinHandle` depois de registrar o spawn),
/// a dependência vira de mão única — mesmo formato que `send_message`
/// (`lib.rs`) já usa com sucesso pra disparar `run_turn` sem bloquear.
fn spawn_orchestrated_turn(
    app: AppHandle,
    child_id: String,
    prompt: String,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let turn_started = std::time::Instant::now();
        let state = app.state::<AppState>();
        let result = run_turn(app.clone(), &state, child_id.clone(), prompt, Vec::new(), None).await;
        if let Some(info) = state.orchestrated_sessions.lock().unwrap().get_mut(&child_id) {
            info.first_response_ms = Some(turn_started.elapsed().as_millis() as u64);
        }
        if let Err(e) = result {
            let _ = app.emit(
                "agent:error",
                serde_json::json!({ "session_id": child_id, "message": e.to_string() }),
            );
        }
        state.running_turns.lock().unwrap().remove(&child_id);

        // A sessao filha terminou o turno dela — se o pai parou de pollar
        // via check_agent_session ANTES disso (seu proprio turno ja tinha
        // encerrado), ele nunca ficaria sabendo (achado reportado ao vivo:
        // sessao parada pra sempre esperando algo que ja tinha terminado).
        // Reativa o pai sozinho, mesmo guard anti-loop de
        // `spawn_auto_continue_turn` (nao reativa se o pai ja estiver
        // ocupado com outra coisa nesse meio tempo).
        if let Ok(child_session) = sessions::get_session(&state.app_data_dir, &child_id) {
            if let Some(parent_id) = child_session.parent_session_id {
                spawn_auto_continue_turn(
                    app.clone(),
                    parent_id,
                    format!(
                        "A sessao filha orquestrada \"{}\" (session_id: {child_id}) terminou o \
                         turno dela. Use check_agent_session(\"{child_id}\") pra conferir o \
                         resultado e continue a partir dai.",
                        child_session.title
                    ),
                );
            }
        }
    })
}

/// Trava anti-loop do auto-continue: quantas vezes seguidas uma sessao pode
/// se retomar sozinha (job em segundo plano ou sessao filha orquestrada
/// terminando) sem uma mensagem de verdade do usuario no meio. Zerado em
/// `send_message` (lib.rs), toda vez que o usuario manda algo de verdade —
/// sem isso, um job em background que sempre dispara outro job em
/// background poderia encadear auto-continuacoes pra sempre. 20 (nao um
/// numero baixo tipo 3) porque o guard existe so pra pegar loop de verdade,
/// nao pra limitar uma tarefa complexa legitima com varias etapas em
/// sequencia (cada `task`/job em segundo plano contando como uma).
const MAX_AUTO_CONTINUES: u32 = 20;

/// Dispara um novo turno pra uma sessao como reacao a algo assincrono ter
/// terminado (comando em segundo plano, sessao filha orquestrada) — sem
/// isso a nota de conclusao so ficava salva no historico, inerte, ate o
/// usuario mandar outra mensagem por conta propria (achado reportado ao
/// vivo: "o LLM simplesmente nao faz mais nada" mesmo o comando demorado ja
/// tendo terminado). So dispara se a sessao NAO tiver turno rodando agora
/// (ex: usuario ja mandou mensagem nova enquanto o job rodava — nesse caso
/// nao interfere) e se o guard `MAX_AUTO_CONTINUES` ainda tiver credito.
///
/// O guard de `running_turns` fica dentro do MESMO lock do check ate o
/// insert do handle (sem soltar no meio), pra fechar a janela de corrida de
/// dois jobs terminando quase juntos pra sessao mesma sessao.
///
/// Quando o teto do guard e atingido, NAO para em silencio (isso reproduziria
/// o bug original que motivou essa funcao existir) — deixa uma nota visivel
/// no chat explicando que parou de continuar sozinho e precisa de um "vai"
/// do usuario, pra nao parecer que o agente simplesmente travou/ficou burro.
pub fn spawn_auto_continue_turn(app: AppHandle, session_id: String, prompt: String) {
    let state = app.state::<AppState>();
    {
        let mut counts = state.auto_continue_counts.lock().unwrap();
        let count = counts.entry(session_id.clone()).or_insert(0);
        if *count >= MAX_AUTO_CONTINUES {
            notify_auto_continue_stopped(&app, &state, &session_id);
            return;
        }
        *count += 1;
    }

    let mut running = state.running_turns.lock().unwrap();
    if running.contains_key(&session_id) {
        return; // sessao ja ocupada (ex: usuario mandou mensagem manual) - nao interfere
    }

    let app_task = app.clone();
    let session_id_task = session_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let state = app_task.state::<AppState>();
        if let Err(e) = run_turn(
            app_task.clone(),
            &state,
            session_id_task.clone(),
            prompt,
            Vec::new(),
            None,
        )
        .await
        {
            let _ = app_task.emit(
                "agent:error",
                serde_json::json!({ "session_id": session_id_task, "message": e.to_string() }),
            );
        }
        state.running_turns.lock().unwrap().remove(&session_id_task);
    });
    running.insert(session_id, handle);
}

#[derive(Serialize, Clone)]
struct AutoContinueStoppedEvent {
    session_id: String,
}

/// Injeta uma nota visivel no chat + avisa a UI quando o guard anti-loop do
/// auto-continue (`MAX_AUTO_CONTINUES`) e atingido — sem isso a sessao
/// simplesmente para de reagir sozinha sem nenhuma explicacao, reproduzindo
/// o mesmo bug ("parece que o LLM travou/e burro") que `spawn_auto_continue_turn`
/// existe pra evitar.
fn notify_auto_continue_stopped(app: &AppHandle, state: &AppState, session_id: &str) {
    let note = ChatMessage {
        role: "system".to_string(),
        content: format!(
            "[Auto-continue parou apos {MAX_AUTO_CONTINUES} tentativas seguidas sem uma \
             mensagem sua no meio - pode ter mais coisa pendente. Mande uma mensagem (ex: \
             \"continua\") se quiser que eu siga.]"
        ),
        tool_calls: None,
        tool_call_id: None,
        name: Some("auto_continue_stopped".to_string()),
        images: Vec::new(),
        display_content: Some(
            "⏸️ Parei de continuar sozinho depois de várias tentativas em sequência sem você \
             mandar nada no meio — pode ter mais coisa pendente. Mande uma mensagem (ex: \
             \"continua\") se quiser que eu siga."
                .to_string(),
        ),
    };
    if let Ok(mut messages) = sessions::load_messages(&state.app_data_dir, session_id) {
        messages.push(note);
        let _ = sessions::save_messages(&state.app_data_dir, session_id, &messages);
    }
    let _ = app.emit(
        "agent:auto_continue_stopped",
        AutoContinueStoppedEvent {
            session_id: session_id.to_string(),
        },
    );
}

/// Registra o início de uma execução de agente/skill (`task`/
/// `verify_completion`) no registro em memória, pra UI poder consultar "o
/// que está rodando agora" (Fase A1). Devolve o `execution_id` (UUID) gerado.
///
/// `parent_id` era sempre `None` na prática até a Fase 3 (pipeline
/// Dev→QA→Analista) existir — a guarda de profundidade estrutural impedia
/// qualquer execução de disparar outra. O pipeline é a primeira exceção
/// genuína: cada etapa (dev/qa/analista, por round) é filha da execução do
/// pipeline inteiro, então passa `Some(pipeline_execution_id)` aqui.
/// Teto de execuções rastreadas — sem isso, `state.agent_executions` cresce
/// pra sempre numa sessão de uso prolongado (cada `task`/`verify_completion`/
/// pipeline fica registrado ali junto de `.steps` com o detalhe de cada
/// tool call, nunca liberado). Mesma categoria de vazamento de memória já
/// corrigida em `background.rs::prune_finished_jobs` (achado investigando
/// um crash real do usuário, RADAR_PRE_LEAK no Log de Eventos do Windows
/// antes de uma falha de alocação) — aplicado aqui também por precaução,
/// mesmo sem confirmar que essa estrutura especificamente já existia na
/// versão que crashou. Só remove execuções JÁ TERMINADAS, mais antigas
/// primeiro.
const MAX_TRACKED_EXECUTIONS: usize = 100;

fn prune_finished_executions(
    executions: &mut std::collections::HashMap<String, crate::models::AgentExecution>,
) {
    if executions.len() < MAX_TRACKED_EXECUTIONS {
        return;
    }
    let mut finished: Vec<(String, u64)> = executions
        .iter()
        .filter(|(_, e)| e.status != "running")
        .map(|(id, e)| (id.clone(), e.started_at_ms))
        .collect();
    finished.sort_by_key(|(_, started_at_ms)| *started_at_ms);
    let excess = executions.len() + 1 - MAX_TRACKED_EXECUTIONS;
    for (id, _) in finished.into_iter().take(excess) {
        executions.remove(&id);
    }
}

fn start_agent_execution(
    state: &AppState,
    session_id: &str,
    kind: &str,
    name: &str,
    parent_id: Option<&str>,
) -> String {
    prune_finished_executions(&mut state.agent_executions.lock().unwrap());
    let id = uuid::Uuid::new_v4().to_string();
    let execution = crate::models::AgentExecution {
        id: id.clone(),
        parent_id: parent_id.map(|p| p.to_string()),
        session_id: session_id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        status: "running".to_string(),
        started_at_ms: chrono::Utc::now().timestamp_millis() as u64,
        finished_at_ms: None,
        steps: Vec::new(),
    };
    state
        .agent_executions
        .lock()
        .unwrap()
        .insert(id.clone(), execution);
    id
}

/// Marca uma execução como terminada (sucesso ou falha) no registro em
/// memória. Best-effort: se o id não existir mais (não deveria acontecer),
/// não faz nada.
fn finish_agent_execution(state: &AppState, execution_id: &str, ok: bool) {
    if let Some(execution) = state
        .agent_executions
        .lock()
        .unwrap()
        .get_mut(execution_id)
    {
        execution.status = if ok { "done" } else { "failed" }.to_string();
        execution.finished_at_ms = Some(chrono::Utc::now().timestamp_millis() as u64);
    }
}

/// Registra um novo passo (chamada de ferramenta) dentro de uma execução —
/// usado por `subagent.rs`/`verifier.rs`/`analyst.rs` pra persistir seu
/// próprio histórico de tool calls dentro do `AgentExecution.steps`, em vez
/// de só emitir o evento efêmero de UI (`agent:tool_call`) que sumia assim
/// que o turno terminava. Best-effort: se a execução já não existir mais no
/// registro (não deveria acontecer), não faz nada.
pub(crate) fn record_execution_step(state: &AppState, execution_id: &str, step: crate::models::TaskItem) {
    if let Some(execution) = state.agent_executions.lock().unwrap().get_mut(execution_id) {
        execution.steps.push(step);
    }
}

/// Atualiza o passo registrado por `record_execution_step` quando a
/// ferramenta termina (status/resultado/duração) — par de
/// `agent:tool_result`, mesma ideia de `record_execution_step` pro
/// `agent:tool_call`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_execution_step(
    state: &AppState,
    execution_id: &str,
    step_id: &str,
    status: &str,
    detail: Option<String>,
    additions: u32,
    deletions: u32,
    duration_ms: u64,
) {
    if let Some(execution) = state.agent_executions.lock().unwrap().get_mut(execution_id) {
        if let Some(step) = execution.steps.iter_mut().find(|s| s.id == step_id) {
            step.status = status.to_string();
            step.detail = detail;
            step.additions = additions;
            step.deletions = deletions;
            step.duration_ms = Some(duration_ms);
        }
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
// Fase A6 do roteiro de Agentes/Skills: teto de tamanho pra descricao de
// skill no catalogo injetado no system prompt, pra nao deixar uma descricao
// longa demais inflar o prompt a toa (picoClaw usa 1024 chars pro campo
// inteiro; aqui e mais conservador, ~200 caracteres reais de custo por
// skill listada, ja que o catalogo inteiro entra em TODO turno). Quando
// corta, o LLM pode chamar read_skill_details(name) pra ler a descricao
// inteira antes de decidir se a skill e relevante.
const SKILL_CATALOG_DESC_MAX_CHARS: usize = 200;

// Margem de reserva antes de compactar: proporcional à janela do modelo, mas
// com piso e teto em tokens absolutos — uma razão fixa (ex: sempre 50%) é
// ruim nos dois extremos. Numa janela de 1M (ex: deepseek via API), 50% joga
// fora 500k de sobra útil à toa; numa janela de 48k (modelo local pequeno),
// 50% corta bem cedo demais. Com piso/teto, a reserva vira ~30-40k pra
// janelas grandes (compacta só perto de 96-97% de uso) e ~6-8k pra janelas
// pequenas (compacta perto de 83-87%, ainda com folga pro próximo turno).
const COMPACT_RESERVE_RATIO: f32 = 0.15;
const COMPACT_RESERVE_MIN_TOKENS: u32 = 6_000;
const COMPACT_RESERVE_MAX_TOKENS: u32 = 40_000;
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
sessao anterior. Quando um comando em segundo plano termina de vez (nao um dev server que fica \
rodando, e sim algo que roda ate acabar, tipo um build/teste longo), o resultado ja aparece \
sozinho no historico como uma nota do sistema - nao fique chamando check_background_output em \
loop so pra descobrir se ja terminou, va fazendo outra coisa e confira de novo depois. Use a \
ferramenta task pra delegar uma sub-tarefa que precisa de varias chamadas \
de ferramenta (ler/buscar/editar varios arquivos) mas cujo processo intermediario nao importa pro \
usuario, so o resultado final - por exemplo 'ache todos os usos de X e resuma onde estao' ou \
'implemente a funcao Y seguindo o padrao existente'. Nao use task pra algo que uma unica chamada \
de ferramenta ja resolve, nem pra decisoes que dependem do contexto desta conversa (o sub-agente \
so ve o prompt que voce escrever, nao o historico daqui) - escreva o prompt da task de forma \
autocontida. Pra trabalho genuinamente grande e paralelo que nao precisa do resultado na hora (ex: \
'monte o frontend' enquanto trata de outra coisa, ou varias frentes ao mesmo tempo), use \
start_agent_session em vez de task - ela cria uma sessao de verdade que roda desacoplada, sem \
bloquear seu turno; confira o progresso depois com check_agent_session, que ja devolve uma sugestao \
de quanto esperar antes de checar de novo - siga essa sugestao, nao fique chamando em loop \
apertado. Use a ferramenta ask quando precisar de uma decisao que so o usuario pode tomar antes \
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
Quando mostrar comandos de shell pro usuario rodar (fora de run_command, direto no texto da \
resposta), prefira UM bloco de codigo por comando — facilita copiar/colar cada um isoladamente. \
So agrupe varios comandos num unico bloco quando eles precisarem rodar juntos, em sequencia, de um \
unico paste (nesse caso comente cada linha se ajudar a entender o que faz). \
\n\n## Formatacao de respostas\n\
- Em respostas longas ou com mais de um assunto (varias noticias, varias secoes de um resumo, \
etapas de um plano), separe cada bloco com um heading markdown (## ou ###) — um heading por topico \
novo, nunca um heading pra cada frase.\n\
- Logo abaixo do heading vem o texto do topico, sem heading extra no meio.\n\
- Use lista (- ou 1.) pra itens paralelos (etapas, opcoes, pontos de uma mesma categoria), nao pra \
paragrafos longos. Quando o item tiver um rotulo/categoria antes da explicacao, use \"**Rotulo:** \
descricao\" (rotulo em negrito seguido de dois-pontos).\n\
- Nao aninhe listas em mais de 2 niveis.\n\
- Paragrafos curtos: no maximo 3-4 linhas, quebre em varios paragrafos ao trocar de ideia dentro \
do mesmo topico.\n\
- Link de fonte/documentacao fica em linha propria logo depois do paragrafo ou item a que pertence, \
nunca no meio do texto corrido.\n\n\
## Regra de Loop\n\
- Continue chamando ferramentas ate a tarefa estar 100% completa.\n\
- NUNCA pare no meio para narrar o que falta. Execute.\n\
- Se precisar de informacao do usuario, use a ferramenta ask.";

const COMPACTION_SYSTEM_PROMPT: &str = "Voce resume trechos antigos de uma conversa entre um \
usuario e um agente de codigo, para liberar espaco de contexto. Escreva um resumo denso e factual \
em portugues: objetivo original do usuario, decisoes tomadas, arquivos e comandos ja mexidos, \
erros encontrados e como foram resolvidos, e o que ainda estava pendente. Sem rodeios, sem \
saudacao, direto o conteudo do resumo.";

#[derive(Serialize, Clone)]
struct ToolCallEvent {
    session_id: String,
    id: String,
    tool: String,
    args: String,
    /// Comando de shell (`run_command`) já extraído dos argumentos, pra UI
    /// mostrar o bloco "IN" completo assim que a chamada começa, sem
    /// precisar esperar a sessão recarregar do disco.
    command: Option<String>,
    /// Caminho de arquivo (`read_file`/`write_file`/`edit_file`/etc.) já
    /// extraído, mesmo motivo do `command` acima.
    file_path: Option<String>,
    /// UUID da execução de agente/skill (`task`/`verify_completion`) dona
    /// deste passo, se este passo aconteceu DENTRO de uma — `None` pra
    /// passos do loop principal da sessão. Ver `models::AgentExecution`.
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_id: Option<String>,
}

#[derive(Serialize, Clone)]
struct ToolResultEvent {
    session_id: String,
    id: String,
    status: String,
    /// Output/observação da ferramenta, pra UI atualizar o bloco "OUT" (ou
    /// o detalhe expandido) assim que a chamada termina, sem esperar
    /// `agent:done`/recarregar a sessão inteira.
    detail: Option<String>,
    additions: u32,
    deletions: u32,
    duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_id: Option<String>,
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

/// Um agente/skill que o modelo pretende usar neste turno (`task`/
/// `load_skill`/`verify_completion`) — item da lista mostrada no modal
/// batelado da Fase A5, pra não pedir aprovação de novo por chamada
/// individual quando o usuário já aprovou o plano inteiro de uma vez.
#[derive(Serialize, Clone)]
struct AgentSkillPlanItem {
    id: String,
    tool: String,
    name: String,
}

#[derive(Serialize, Clone)]
struct AgentsSkillsPlanEvent {
    session_id: String,
    id: String,
    items: Vec<AgentSkillPlanItem>,
    /// Fase A4: true quando 2+ chamadas de `task` vão rodar em PARALELO
    /// (provider de API, não local) neste turno — a UI usa isso pra avisar
    /// que execução paralela gasta mais chamadas simultâneas de API e pode
    /// esbarrar em rate limit, além do aviso normal de "vou usar X/Y/Z".
    parallel: bool,
}

/// Modo "Manual": em vez de um popup por chamada de `task`/`load_skill`/
/// `verify_completion` (que já pausariam individualmente via
/// `request_permission`, gerando fadiga de clique quando o modelo planeja
/// usar várias no mesmo turno), pergunta uma vez só, batelado, ANTES de
/// começar a executar as tool calls do turno. Devolve aprovado/negado pra
/// TODAS as chamadas listadas de uma vez (aprovar/recusar uma a uma fica
/// pra uma iteração futura, ver roteiro Fase A5).
async fn request_agents_skills_plan(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    items: Vec<AgentSkillPlanItem>,
    parallel: bool,
) -> Result<bool> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    state
        .pending_agent_plans
        .lock()
        .unwrap()
        .insert(id.clone(), tx);
    let _ = app.emit(
        "agent:agents_skills_plan",
        AgentsSkillsPlanEvent {
            session_id: session_id.to_string(),
            id: id.clone(),
            items,
            parallel,
        },
    );
    rx.await.map_err(|_| {
        anyhow::anyhow!(
            "plano de agentes/skills cancelado (sessao ou app encerrado antes de responder)"
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

    // Monta o system prompt com as informações atuais (pastas, skills, etc.)
    // — refeito a cada turno, não só na primeira mensagem, pra refletir
    // mudanças como adição de pastas extras durante a sessão.
    {
        let project_path: Option<&Path> = session.project_root.as_deref().map(Path::new)
            .or_else(|| {
                let first = session.extra_read_paths.first()?;
                let p = Path::new(&first.path);
                if p.is_dir() { Some(p) } else { None }
            });
        // Persona ativa carregada aqui (antes de montar o catalogo de skills)
        // pra poder filtrar o catalogo pela allowlist dela (B3/Fase A3) — o
        // uso pra injetar `## Perfil ativo` no prompt mais abaixo reusa essa
        // mesma variavel, so ler o disco uma vez.
        let active_persona = session.persona_id.as_ref().and_then(|persona_id| {
            crate::personas::list_personas(&app_data_dir)
                .ok()?
                .into_iter()
                .find(|p| &p.id == persona_id)
        });

        let mut catalog = skills::list_skills(&app_data_dir, project_path).unwrap_or_default();
        if let Some(ref persona) = active_persona {
            if !persona.skills.is_empty() {
                catalog.retain(|s| persona.skills.contains(&s.name));
            }
        }
        let mut prompt = SYSTEM_PROMPT.to_string();
        // Memoria entre sessoes (inspirado no Hermes Agent, 14_backlog_pendente.md):
        // fatos duraveis gravados pela tool `remember` em MEMORY.md, lidos no
        // inicio de TODA sessao (nao so a que gravou) - carrega contexto que
        // ja foi estabelecido antes sem precisar reexplicar.
        if let Ok(memory) = crate::memory::load_memory(&app_data_dir) {
            if !memory.trim().is_empty() {
                prompt.push_str("\n\n## Memoria entre sessoes\n");
                prompt.push_str(&memory);
            }
        }
        if !catalog.is_empty() {
            let mut any_truncated = false;
            prompt.push_str("\n\nSkills disponiveis (chame load_skill(name) pra ler o conteudo completo de uma antes de segui-la):\n");
            for skill in &catalog {
                any_truncated |= skill.description.chars().count() > SKILL_CATALOG_DESC_MAX_CHARS;
                let desc = truncate(&skill.description, SKILL_CATALOG_DESC_MAX_CHARS);
                prompt.push_str(&format!("- {} ({}): {}\n", skill.name, skill.scope, desc));
            }
            if any_truncated {
                prompt.push_str(
                    "Descricoes cortadas em '...' acima: chame read_skill_details(name) pra ler \
                     a descricao inteira antes de decidir se a skill e relevante.\n",
                );
            }
        }
        if session.fable_method {
            prompt.push_str("\n\n");
            prompt.push_str(FABLE_METHOD_PROMPT);
        }
        if let Some(ref persona) = active_persona {
            prompt.push_str("\n\n## Perfil ativo\n");
            prompt.push_str(&persona.content);
        }
        if let Some(ref root) = session.project_root {
            // Separador do SO real (Tarefa 4.2 do port): em Windows o exemplo
            // continua com `\`; em Unix vira `/` — o prompt e contrato de
            // comportamento do LLM, ensinar separador errado degrada tudo
            // silenciosamente.
            let sep = std::path::MAIN_SEPARATOR;
            prompt.push_str(&format!(
                "\n\nPasta do projeto desta sessao: {root}\n\
                 Use SEMPRE caminhos absolutos nas ferramentas de arquivo e diretorio \
                 (ex: {root}{sep}arquivo.txt) — nunca caminhos relativos. Assim o usuario ve \
                 exatamente em qual pasta cada arquivo sera lido ou criado."
            ));
        }
        if !session.extra_read_paths.is_empty() {
            prompt.push_str("\n\nPastas extras disponiveis nesta sessao:\n");
            for entry in &session.extra_read_paths {
                let mode = match entry.mode {
                    crate::models::FolderMode::Read => "🔍 só leitura",
                    crate::models::FolderMode::ReadWrite => "✏️ leitura e escrita",
                };
                prompt.push_str(&format!("  - {} ({})\n", entry.path, mode));
            }
            prompt.push_str("Use SEMPRE caminhos absolutos nas ferramentas de arquivo e diretorio.");
        }
        let has_project_tools =
            session.project_root.is_some() || !session.extra_read_paths.is_empty();
        if has_project_tools {
            let shell_info = shell::detect_shell();
            prompt.push_str(&format!(
                "\n\nShell disponivel para run_command: {} — use a sintaxe desse shell nos comandos.",
                shell_info.description
            ));
        } else {
            // Achado testando ao vivo (2026-08-17): sessoes orquestradas
            // (start_agent_session) criadas sem project_root herdado nem
            // passado explicitamente ficam SEM nenhuma ferramenta de
            // arquivo/shell (read_file, write_file, run_command etc — todas
            // vem de project_tool_specs, so incluido quando ha pasta). Sem
            // avisar isso no prompt, o modelo nao sabia que faltava a
            // ferramenta e ficou tentando contornar via busca na web em vez
            // de simplesmente dizer que precisa de uma pasta.
            prompt.push_str(
                "\n\nEsta sessao NAO tem pasta de projeto nem pasta extra anexada, entao \
                 ferramentas de arquivo e comando (read_file, write_file, run_command, etc.) \
                 NAO estao disponiveis. Se a tarefa pedida exigir criar/editar arquivos ou \
                 rodar comandos, nao tente contornar isso buscando na web — responda \
                 explicando que precisa de uma pasta de projeto pra continuar.",
            );
        }
        if messages.is_empty() {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: None,
            });
        } else {
            // Atualiza a primeira mensagem (system) com o prompt mais recente
            messages[0].content = prompt;
        }
    }
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_text.clone(),
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
        session.llama_fork.as_deref(),
    )?;

    // Auto-nomeação: se é a primeira mensagem e o título ainda é o default,
    // pede ao LLM um nome curto baseado no texto do usuário. Roda em paralelo
    // (não bloqueia o loop principal) — o nome aparece na sidebar assim que
    // o LLM responder, sem atrasar a resposta real.
    let is_first_message = messages.iter().filter(|m| m.role == "user").count() == 1;
    let is_default_title = session.title == "Nova sessão"
        || session.title == "New session"
        || session.title == "Nueva sesión"
        || session.title == "新建会话"
        || session.title == "新会话";
    if is_first_message && is_default_title {
        let app_clone = app.clone();
        let session_id_clone = session_id.clone();
        let cfg_clone = cfg.clone();
        let api_key_clone = api_key.clone();
        let model_clone = session.model.clone();
        let user_text_clone = user_text.clone();
        let app_data_dir_clone = app_data_dir.clone();
        tokio::spawn(async move {
            // take(500) conta CARACTERES, nao bytes — slicing por byte
            // entraria em panico no meio de um acento/ideograma multibyte.
            let snippet: String = user_text_clone.chars().take(500).collect();
            let naming_prompt = format!(
                "Dê um nome MUITO curto (máximo 5 palavras) para uma conversa que começa com esta mensagem do usuário. \
                 Responda APENAS com o nome, sem aspas, sem explicação, sem pontuação final.\n\n\
                 Mensagem: {snippet}"
            );
            let naming_messages = vec![ChatMessage {
                role: "user".to_string(),
                content: naming_prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: None,
            }];
            // Canal sintetico, mesmo padrao da compactacao (`::compact`) —
            // antes usava o session_id de verdade, entao os tokens dessa
            // chamada utilitaria (so pra descobrir o nome) vazavam no
            // `chat:token` da conversa de verdade, se misturando com a
            // resposta real que estava sendo transmitida ao mesmo tempo.
            // O frontend so escuta o canal real, entao o usuario nunca
            // precisa ver o LLM "pensando" no nome — so o resultado final
            // (evento `agent:session_renamed`) importa.
            let naming_channel = format!("{session_id_clone}::naming");
            if let Ok(result) = providers::chat_stream(
                &app_clone,
                &naming_channel,
                &cfg_clone,
                api_key_clone,
                &model_clone,
                &naming_messages,
                &[],
                Some(crate::models::ReasoningEffort::Off),
                None,
            )
            .await
            {
                let name = result.message.content.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() && name.len() < 80 {
                    if let Ok(updated) = sessions::update_title(&app_data_dir_clone, &session_id_clone, name.to_string()) {
                        let _ = app_clone.emit(
                            "agent:session_renamed",
                            serde_json::json!({
                                "session_id": session_id_clone,
                                "title": updated.title,
                            }),
                        );
                    }
                }
            }
        });
    }

    let mut tool_specs = tools::always_tool_specs();
    if session.project_root.is_some() || !session.extra_read_paths.is_empty() {
        tool_specs.extend(tools::project_tool_specs());
    }
    // Fase G: guarda de profundidade de nivel unico — uma sessao ORQUESTRADA
    // (criada via start_agent_session, tem parent_session_id) nao ganha as
    // ferramentas de orquestracao, mesmo espirito do guard que `task` ja tem
    // pra sub-agente nao poder recursar.
    if session.parent_session_id.is_none() {
        tool_specs.extend(tools::orchestration_tool_specs());
    }
    let mut mcp_servers = crate::mcp::load_servers(&app_data_dir).unwrap_or_default();
    if let Some(ref enabled) = session.enabled_mcp_servers {
        mcp_servers.retain(|s| enabled.contains(&s.name));
    }
    tool_specs.extend(state.mcp_clients.tool_specs(&mcp_servers).await);

    // Fase A3: se a persona ativa desta sessao definiu uma allowlist de
    // ferramentas, filtra o toolset pra so essas (+ `ask`, sempre mantido —
    // sem isso o modelo pode ficar travado sem como pedir esclarecimento).
    // Vazio (default, e todas as personas de antes desse campo existir) =
    // sem filtro nenhum, comportamento identico a antes.
    if let Some(ref persona_id) = session.persona_id {
        if let Ok(personas) = crate::personas::list_personas(&app_data_dir) {
            if let Some(persona) = personas.iter().find(|p| &p.id == persona_id) {
                if !persona.tools.is_empty() {
                    tool_specs
                        .retain(|t| t.function.name == "ask" || persona.tools.contains(&t.function.name));
                }
            }
        }
    }

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

    // Compacta (se precisar) UMA vez, aqui, antes de mandar a mensagem do
    // usuario pro LLM — nao mais a cada passo do loop de tool calls la
    // embaixo. Rodar no meio de uma sequencia de tool calls pausava o turno
    // de forma invisivel pro usuario (achado ao vivo: parecia que o stream
    // tinha travado). O trade-off aceito: um turno com MUITAS tool calls em
    // sequencia pode crescer o contexto alem do limite antes do PROXIMO
    // turno recompactar — aceitavel porque e raro e o alternativa (checar a
    // cada passo) e o que causava a pausa.
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

    let mut tasks = sessions::load_tasks(&app_data_dir, &session_id)?;
    let mut recent_calls: Vec<(String, String)> = Vec::new();
    let mut tool_steps: usize = 0;
    let turn_start = std::time::Instant::now();
    let mut turn_prompt_tokens: u32 = 0;
    let mut turn_completion_tokens: u32 = 0;

    'steps: loop {
        if tool_steps >= MAX_AGENTIC_STEPS {
            break;
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
            None,
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
            break;
        }

        tool_steps += 1;

        let project_path: Option<&Path> = session.project_root.as_deref().map(Path::new)
            .or_else(|| {
                let first = session.extra_read_paths.first()?;
                let p = Path::new(&first.path);
                if p.is_dir() { Some(p) } else { None }
            });

        // Fase A4: quando ha 2+ chamadas de `task` no MESMO turno e o
        // provider NAO e local (GPU local so aguenta uma coisa por vez - a
        // fila "de 1" ja existe por construcao, o turno inteiro roda numa
        // cadeia sequencial de awaits), roda os sub-agentes em paralelo de
        // verdade mais abaixo, em vez de esperar um terminar pra comecar o
        // proximo.
        let task_call_ids: Vec<String> = assistant
            .tool_calls
            .iter()
            .flatten()
            .filter(|c| c.function.name == "task")
            .map(|c| c.id.clone())
            .collect();
        let parallel_eligible = !cfg.kind.is_local() && task_call_ids.len() >= 2;

        // Modo "Manual", Fase A5: se este turno vai usar agente(s)/skill(s)
        // (task/load_skill/verify_completion), pergunta uma vez so, batelado,
        // em vez de deixar cada chamada pausar individualmente (que ja
        // aconteceria mais abaixo via request_permission) - evita fadiga de
        // clique quando o modelo planeja usar varias no mesmo turno.
        let mut agents_skills_decision: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        if session.execution_mode == ExecutionMode::Manual {
            let plan_items: Vec<AgentSkillPlanItem> = assistant
                .tool_calls
                .iter()
                .flatten()
                .filter(|call| {
                    matches!(
                        call.function.name.as_str(),
                        "task" | "load_skill" | "verify_completion" | "run_pipeline"
                    )
                })
                .map(|call| {
                    let args: serde_json::Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);
                    let name = match call.function.name.as_str() {
                        "task" => args["description"].as_str().unwrap_or("sub-tarefa").to_string(),
                        "load_skill" => args["name"].as_str().unwrap_or("skill").to_string(),
                        "run_pipeline" => args["requirement"]
                            .as_str()
                            .unwrap_or("pipeline dev/qa/analista")
                            .to_string(),
                        _ => "verificador".to_string(),
                    };
                    AgentSkillPlanItem {
                        id: call.id.clone(),
                        tool: call.function.name.clone(),
                        name,
                    }
                })
                .collect();
            if !plan_items.is_empty() {
                let ids: Vec<String> = plan_items.iter().map(|i| i.id.clone()).collect();
                let approved_all =
                    request_agents_skills_plan(&app, state, &session_id, plan_items, parallel_eligible)
                        .await?;
                for id in ids {
                    agents_skills_decision.insert(id, approved_all);
                }
            }
        } else if parallel_eligible {
            // Auto/YOLO nao tem modal de aprovacao pra agentes/skills - ainda
            // assim avisa a UI (sem bloquear) que isso vai rodar em paralelo
            // via API, o que gasta mais chamadas simultaneas e pode esbarrar
            // em rate limit.
            let _ = app.emit(
                "agent:parallel_execution_info",
                serde_json::json!({
                    "session_id": session_id,
                    "count": task_call_ids.len(),
                }),
            );
        }

        // Roda os `task` elegiveis (ver parallel_eligible acima) em paralelo
        // via join_all, ANTES do loop sequencial abaixo - que so consome o
        // resultado ja pronto (ver `precomputed_task_results.remove` la
        // embaixo) em vez de rodar de novo. Continuam na mesma tokio task
        // (sem tokio::spawn), so as chamadas HTTP ficam concorrentes.
        // Carrega o execution_id junto do resultado pra o loop sequencial
        // abaixo poder linkar o TaskItem dessa chamada com a execucao (pra
        // UI conseguir mostrar os passos internos gravados nela).
        let mut precomputed_task_results: std::collections::HashMap<String, (String, Result<String>)> =
            std::collections::HashMap::new();
        if parallel_eligible {
            let extra_paths = crate::models::FolderEntry::paths(&session.extra_read_paths);
            let mut futures = Vec::new();
            for call in assistant.tool_calls.iter().flatten() {
                if call.function.name != "task" {
                    continue;
                }
                if let Some(&approved) = agents_skills_decision.get(&call.id) {
                    if !approved {
                        // Negado no plano batelado - o loop sequencial abaixo
                        // reporta o erro (nao esta em precomputed_task_results).
                        continue;
                    }
                }
                let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                    .unwrap_or(serde_json::Value::Null);
                let Some(project_root) = project_path else {
                    continue; // sem pasta - o loop sequencial abaixo reporta o erro de validacao
                };
                let Some(prompt) = args["prompt"].as_str() else {
                    continue; // sem prompt - idem
                };
                let description = args["description"].as_str().unwrap_or("sub-tarefa").to_string();
                let execution_id = start_agent_execution(state, &session_id, "task", &description, None);
                let execution_id_clone = execution_id.clone();
                let call_id = call.id.clone();
                let prompt = prompt.to_string();
                let app_clone = app.clone();
                let session_id_clone = session_id.clone();
                let cfg_clone = cfg.clone();
                let api_key_clone = api_key.clone();
                let model_clone = session.model.clone();
                let extra_paths_clone = extra_paths.clone();
                let enabled_mcp_clone = session.enabled_mcp_servers.clone();
                let execution_mode_clone = session.execution_mode.clone();
                futures.push(async move {
                    let result = subagent::run(
                        &app_clone,
                        state,
                        &session_id_clone,
                        &execution_id,
                        &cfg_clone,
                        api_key_clone,
                        &model_clone,
                        project_root,
                        &extra_paths_clone,
                        &description,
                        &prompt,
                        enabled_mcp_clone.as_deref(),
                        &execution_mode_clone,
                    )
                    .await;
                    finish_agent_execution(state, &execution_id, result.is_ok());
                    (call_id, execution_id_clone, result)
                });
            }
            if !futures.is_empty() {
                for (call_id, execution_id, result) in futures_util::future::join_all(futures).await {
                    precomputed_task_results.insert(call_id, (execution_id, result));
                }
            }
        }

        for call in assistant.tool_calls.iter().flatten() {
            let args: serde_json::Value =
                serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);
            let file_path = extract_file_path(&call.function.name, &args);
            let command = extract_command_text(&call.function.name, &args);

            let _ = app.emit(
                "agent:tool_call",
                ToolCallEvent {
                    session_id: session_id.clone(),
                    id: call.id.clone(),
                    tool: call.function.name.clone(),
                    args: call.function.arguments.clone(),
                    command: command.clone(),
                    file_path: file_path.clone(),
                    execution_id: None,
                },
            );

            let task_id = call.id.clone();
            let task_idx = tasks.len();
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
                // Preenchido mais abaixo, quando a chamada for task/
                // verify_completion/run_pipeline e o id da execucao ja
                // existir (essa struct e montada antes do despacho saber
                // qual ferramenta e).
                execution_id: None,
            });
            sessions::save_tasks(&app_data_dir, &session_id, &tasks)?;

            // Modo "Manual": toda tool call pausa esperando aprovacao antes
            // de rodar, exceto a propria `ask` (ja e uma pausa esperando o
            // usuario, pedir permissao pra perguntar seria so redundante) e
            // exceto as que ja foram decididas em lote pelo modal de
            // agentes/skills acima (Fase A5) - senao o usuario seria
            // perguntado duas vezes pela mesma chamada.
            let approved = if let Some(&decided) = agents_skills_decision.get(&call.id) {
                decided
            } else if session.execution_mode == ExecutionMode::Manual && call.function.name != "ask"
            {
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
                    Some(skill_name) if !skill_allowed_for_persona(&app_data_dir, &session, skill_name) => {
                        Err(anyhow::anyhow!(
                            "skill '{skill_name}' nao esta na allowlist da persona ativa desta sessao - \
                             ela so pode carregar as skills listadas no catalogo do system prompt"
                        ))
                    }
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
            } else if call.function.name == "read_skill_details" {
                // Fase A6 do roteiro de Agentes/Skills: descricao no catalogo
                // do system prompt vem cortada em SKILL_CATALOG_DESC_MAX_CHARS
                // pra nao inflar o prompt a toa - essa tool devolve a
                // descricao INTEIRA sob demanda, sem carregar o corpo da
                // skill (isso continua sendo so o load_skill).
                match args["name"].as_str() {
                    Some(skill_name) => {
                        let catalog =
                            skills::list_skills(&app_data_dir, project_path).unwrap_or_default();
                        match catalog.into_iter().find(|s| s.name == skill_name) {
                            Some(skill) => Ok(tools::ToolOutcome {
                                observation: format!(
                                    "{} ({}): {}",
                                    skill.name, skill.scope, skill.description
                                ),
                                pending_edit: None,
                            }),
                            None => Err(anyhow::anyhow!("skill '{skill_name}' nao encontrada no catalogo")),
                        }
                    }
                    None => Err(anyhow::anyhow!("name obrigatorio")),
                }
            } else if call.function.name == "improve_skill" {
                match (args["name"].as_str(), args["new_content"].as_str()) {
                    (Some(skill_name), _) if !skill_allowed_for_persona(&app_data_dir, &session, skill_name) => {
                        Err(anyhow::anyhow!(
                            "skill '{skill_name}' nao esta na allowlist da persona ativa desta sessao"
                        ))
                    }
                    (Some(skill_name), Some(new_content)) => {
                        let catalog =
                            skills::list_skills(&app_data_dir, project_path).unwrap_or_default();
                        match catalog.into_iter().find(|s| s.name == skill_name) {
                            Some(skill) => skills::write_skill_file(&skill.dir, new_content).map(|_| {
                                tools::ToolOutcome {
                                    observation: format!(
                                        "skill '{skill_name}' atualizada com sucesso."
                                    ),
                                    pending_edit: None,
                                }
                            }),
                            None => Err(anyhow::anyhow!(
                                "skill '{skill_name}' nao encontrada no catalogo"
                            )),
                        }
                    }
                    _ => Err(anyhow::anyhow!("name e new_content sao obrigatorios")),
                }
            } else if call.function.name == "remember" {
                match args["fact"].as_str() {
                    Some(fact) => crate::memory::append_memory(&app_data_dir, fact).map(|_| {
                        tools::ToolOutcome {
                            observation: "fato registrado em MEMORY.md - vai aparecer no prompt \
                                          de toda sessao futura."
                                .to_string(),
                            pending_edit: None,
                        }
                    }),
                    None => Err(anyhow::anyhow!("fact obrigatorio")),
                }
            } else if let Some((execution_id, precomputed)) = precomputed_task_results.remove(&call.id) {
                // Fase A4: essa chamada de `task` ja rodou em paralelo com
                // outras do mesmo turno (ver bloco antes deste loop) - so
                // usa o resultado que ja veio pronto, sem rodar de novo.
                if let Some(t) = tasks.get_mut(task_idx) {
                    t.execution_id = Some(execution_id);
                }
                precomputed.map(|report| tools::ToolOutcome {
                    observation: report,
                    pending_edit: None,
                })
            } else if call.function.name == "task" {
                // Tratado a parte, igual load_skill: precisa de app/estado/
                // provider que tools::execute_tool nao recebe (e nao devia
                // precisar receber, so essa ferramenta usa isso).
                match (project_path, args["prompt"].as_str()) {
                    (Some(project_root), Some(prompt)) => {
                        let description = args["description"].as_str().unwrap_or("sub-tarefa");
                        let execution_id =
                            start_agent_execution(state, &session_id, "task", description, None);
                        if let Some(t) = tasks.get_mut(task_idx) {
                            t.execution_id = Some(execution_id.clone());
                        }
                        let result = subagent::run(
                            &app,
                            state,
                            &session_id,
                            &execution_id,
                            &cfg,
                            api_key.clone(),
                            &session.model,
                            project_root,
                            &crate::models::FolderEntry::paths(&session.extra_read_paths),
                            description,
                            prompt,
                            session.enabled_mcp_servers.as_deref(),
                            &session.execution_mode,
                        )
                        .await;
                        finish_agent_execution(state, &execution_id, result.is_ok());
                        result.map(|report| tools::ToolOutcome {
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
                    (Some(project_root), Some(summary), Some(how)) => {
                        let execution_id =
                            start_agent_execution(state, &session_id, "verify_completion", "verificador", None);
                        if let Some(t) = tasks.get_mut(task_idx) {
                            t.execution_id = Some(execution_id.clone());
                        }
                        let result = verifier::run(
                            &app,
                            state,
                            &session_id,
                            &execution_id,
                            &cfg,
                            api_key.clone(),
                            &session.model,
                            project_root,
                            &crate::models::FolderEntry::paths(&session.extra_read_paths),
                            summary,
                            how,
                        )
                        .await;
                        finish_agent_execution(state, &execution_id, result.is_ok());
                        result.map(|verdict| tools::ToolOutcome {
                            observation: verdict,
                            pending_edit: None,
                        })
                    }
                    (None, _, _) => Err(anyhow::anyhow!(
                        "verify_completion precisa de uma pasta de projeto associada a sessao"
                    )),
                    _ => Err(anyhow::anyhow!("task_summary e how_to_verify obrigatorios")),
                }
            } else if call.function.name == "run_pipeline" {
                // Fase 3: pipeline determinístico Dev→QA→Analista (ver
                // agent/pipeline.rs). Tratado a parte igual task/
                // verify_completion: precisa de app/estado/provider que
                // tools::execute_tool não tem, e orquestra internamente
                // subagent::run + verifier::run + analyst::run em sequência.
                match (project_path, args["requirement"].as_str()) {
                    (Some(project_root), Some(requirement)) => {
                        let max_rounds = args["max_rounds"]
                            .as_u64()
                            .map(|n| n as u32)
                            .unwrap_or(pipeline::DEFAULT_MAX_ROUNDS);
                        let pipeline_execution_id = start_agent_execution(
                            state,
                            &session_id,
                            "pipeline",
                            "Pipeline Dev → QA → Analista",
                            None,
                        );
                        if let Some(t) = tasks.get_mut(task_idx) {
                            t.execution_id = Some(pipeline_execution_id.clone());
                        }
                        let result = pipeline::run(
                            &app,
                            state,
                            &session_id,
                            &cfg,
                            api_key.clone(),
                            &session.model,
                            project_root,
                            &crate::models::FolderEntry::paths(&session.extra_read_paths),
                            session.enabled_mcp_servers.as_deref(),
                            &session.execution_mode,
                            &pipeline_execution_id,
                            requirement,
                            max_rounds,
                        )
                        .await;
                        result.map(|report| tools::ToolOutcome {
                            observation: report,
                            pending_edit: None,
                        })
                    }
                    (None, _) => Err(anyhow::anyhow!(
                        "run_pipeline precisa de uma pasta de projeto associada a sessao"
                    )),
                    (_, None) => Err(anyhow::anyhow!("requirement obrigatorio")),
                }
            } else if call.function.name == "start_agent_session" {
                // Fase G: cria uma Session de verdade e dispara run_turn
                // desacoplado (tauri::async_runtime::spawn), mesmo padrao
                // exato de send_message (lib.rs) - nao espera terminar,
                // devolve o session_id na hora.
                match (args["description"].as_str(), args["prompt"].as_str()) {
                    (Some(description), Some(prompt)) => {
                        let child_project_root = args["project_root"]
                            .as_str()
                            .map(|s| s.to_string())
                            .or_else(|| session.project_root.clone());
                        match sessions::create_session(
                            &app_data_dir,
                            description.to_string(),
                            session.provider,
                            session.model.clone(),
                            child_project_root,
                            None,
                            session.llama_fork.clone(),
                            session.custom_provider_id.clone(),
                        ) {
                            Ok(child) => {
                                let updated_child = sessions::update_parent_session_id(
                                    &app_data_dir,
                                    &child.id,
                                    Some(session_id.clone()),
                                )
                                .unwrap_or(child);
                                // UI: sidebar so carrega a lista de sessoes no
                                // mount/acoes explicitas do usuario - sem esse
                                // evento a sessao orquestrada ficaria invisivel
                                // ate o usuario recarregar o app na mao.
                                let _ = app.emit("agent:session_created", &updated_child);
                                // Herda o modo de execucao do pai - Manual
                                // exigiria alguem clicando aceitar na sessao
                                // filha (possivel, ja que ela e uma sessao de
                                // verdade na sidebar, mas o padrao mais util
                                // pra um fluxo desacoplado e o mesmo modo do
                                // orquestrador).
                                let _ = sessions::update_execution_mode(
                                    &app_data_dir,
                                    &updated_child.id,
                                    session.execution_mode,
                                );
                                // Achado testando ao vivo (2026-08-17): so herdar
                                // provider/modelo/project_root nao bastava - uma
                                // sessao orquestrada sem as pastas extras, MCPs
                                // habilitados e persona do pai perdia capacidades
                                // que o usuario esperava que ela tivesse por
                                // padrao. Pedido explicito do usuario: a sessao
                                // filha deve herdar TUDO do pai, nao so o minimo.
                                let _ = sessions::update_extra_read_paths(
                                    &app_data_dir,
                                    &updated_child.id,
                                    session.extra_read_paths.clone(),
                                );
                                let _ = sessions::update_enabled_mcp_servers(
                                    &app_data_dir,
                                    &updated_child.id,
                                    session.enabled_mcp_servers.clone(),
                                );
                                let _ = sessions::update_persona(
                                    &app_data_dir,
                                    &updated_child.id,
                                    session.persona_id.clone(),
                                );
                                if session.fable_method {
                                    let _ = sessions::update_fable_method(
                                        &app_data_dir,
                                        &updated_child.id,
                                        true,
                                    );
                                }
                                let child_id = updated_child.id.clone();
                                let started_at_ms = chrono::Utc::now().timestamp_millis() as u64;
                                state.orchestrated_sessions.lock().unwrap().insert(
                                    child_id.clone(),
                                    OrchestratedSessionInfo {
                                        started_at_ms,
                                        first_response_ms: None,
                                        last_checked_ms: None,
                                        poll_count: 0,
                                    },
                                );
                                let handle = spawn_orchestrated_turn(
                                    app.clone(),
                                    child_id.clone(),
                                    prompt.to_string(),
                                );
                                state
                                    .running_turns
                                    .lock()
                                    .unwrap()
                                    .insert(child_id.clone(), handle);
                                Ok(tools::ToolOutcome {
                                    observation: format!(
                                        "Sessao orquestrada criada, rodando em segundo plano. \
                                         session_id: \"{child_id}\". Use check_agent_session \
                                         mais tarde pra conferir o progresso - nao fique \
                                         checando em loop apertado."
                                    ),
                                    pending_edit: None,
                                })
                            }
                            Err(e) => Err(anyhow::anyhow!(
                                "nao foi possivel criar a sessao orquestrada: {e}"
                            )),
                        }
                    }
                    _ => Err(anyhow::anyhow!("description e prompt sao obrigatorios")),
                }
            } else if call.function.name == "check_agent_session" {
                match args["session_id"].as_str() {
                    Some(child_id) => {
                        let running = state.running_turns.lock().unwrap().contains_key(child_id);
                        if running {
                            // O hint em texto sozinho nao bastava: um modelo local
                            // pequeno (qwen3.5-9b) ignorou a sugestao e chamou essa
                            // ferramenta centenas de vezes em loop apertado, achado
                            // testando ao vivo. Agora a propria chamada espera de
                            // verdade (min. 4s, no maximo o intervalo sugerido) antes
                            // de responder "running" de novo - forca uma pausa real
                            // independente do modelo respeitar o texto ou nao.
                            let (suggested_ms, elapsed) = {
                                let map = state.orchestrated_sessions.lock().unwrap();
                                match map.get(child_id) {
                                    Some(info) => {
                                        let elapsed = (chrono::Utc::now().timestamp_millis() as u64)
                                            .saturating_sub(info.started_at_ms);
                                        let suggested = match info.first_response_ms {
                                            Some(ms) => ((ms as f64) * 1.2) as u64,
                                            None => 12000,
                                        };
                                        (suggested, elapsed)
                                    }
                                    None => (12000, 0),
                                }
                            };
                            let now = chrono::Utc::now().timestamp_millis() as u64;
                            let since_last_check = {
                                let map = state.orchestrated_sessions.lock().unwrap();
                                map.get(child_id)
                                    .and_then(|info| info.last_checked_ms)
                                    .map(|last| now.saturating_sub(last))
                            };
                            let min_wait_ms = suggested_ms.clamp(4000, 30000);
                            if let Some(since) = since_last_check {
                                if since < min_wait_ms {
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        min_wait_ms - since,
                                    ))
                                    .await;
                                }
                            }
                            let poll_count = {
                                let mut map = state.orchestrated_sessions.lock().unwrap();
                                match map.get_mut(child_id) {
                                    Some(info) => {
                                        info.last_checked_ms =
                                            Some(chrono::Utc::now().timestamp_millis() as u64);
                                        info.poll_count += 1;
                                        info.poll_count
                                    }
                                    None => 1,
                                }
                            };
                            if poll_count >= MAX_ORCHESTRATED_POLLS {
                                Ok(tools::ToolOutcome {
                                    observation: format!(
                                        "status: running ({elapsed}ms desde o inicio, ja \
                                         verificado {poll_count} vezes). PARE de checar essa \
                                         sessao agora - ela continua rodando em segundo plano \
                                         e o usuario pode conferir depois pela sidebar. \
                                         Termine seu turno avisando o usuario que a sessao \
                                         '{child_id}' segue em andamento, sem chamar \
                                         check_agent_session de novo."
                                    ),
                                    pending_edit: None,
                                })
                            } else {
                                Ok(tools::ToolOutcome {
                                    observation: format!(
                                        "status: running ({elapsed}ms desde o inicio). \
                                         continua rodando - chame check_agent_session de \
                                         novo se precisar, essa chamada ja espera o tempo \
                                         necessario antes de responder."
                                    ),
                                    pending_edit: None,
                                })
                            }
                        } else {
                            match sessions::load_messages(&app_data_dir, child_id) {
                                Ok(messages) => {
                                    let last_assistant =
                                        messages.iter().rev().find(|m| m.role == "assistant");
                                    match last_assistant {
                                        Some(m) => Ok(tools::ToolOutcome {
                                            observation: format!(
                                                "status: done. ultima resposta:\n{}",
                                                m.content
                                            ),
                                            pending_edit: None,
                                        }),
                                        None => Ok(tools::ToolOutcome {
                                            observation: "status: done. sem resposta do \
                                                 assistente ainda (pode ter falhado antes de \
                                                 responder)."
                                                .to_string(),
                                            pending_edit: None,
                                        }),
                                    }
                                }
                                Err(e) => Err(anyhow::anyhow!(
                                    "nao foi possivel ler a sessao '{child_id}': {e}"
                                )),
                            }
                        }
                    }
                    None => Err(anyhow::anyhow!("session_id obrigatorio")),
                }
            } else if call.function.name == "list_agent_sessions" {
                match sessions::list_sessions(&app_data_dir) {
                    Ok(all) => {
                        let children: Vec<_> = all
                            .into_iter()
                            .filter(|s| s.parent_session_id.as_deref() == Some(session_id.as_str()))
                            .collect();
                        if children.is_empty() {
                            Ok(tools::ToolOutcome {
                                observation: "nenhuma sessao orquestrada criada ainda nesta \
                                     conversa."
                                    .to_string(),
                                pending_edit: None,
                            })
                        } else {
                            let running_ids: std::collections::HashSet<String> =
                                state.running_turns.lock().unwrap().keys().cloned().collect();
                            let lines: Vec<String> = children
                                .iter()
                                .map(|s| {
                                    let status =
                                        if running_ids.contains(&s.id) { "running" } else { "done" };
                                    format!("- {} ({}): {}", s.title, status, s.id)
                                })
                                .collect();
                            Ok(tools::ToolOutcome {
                                observation: lines.join("\n"),
                                pending_edit: None,
                            })
                        }
                    }
                    Err(e) => Err(anyhow::anyhow!("nao foi possivel listar sessoes: {e}")),
                }
            } else if call.function.name == "stop_agent_session" {
                match args["session_id"].as_str() {
                    Some(child_id) => {
                        let handle = state.running_turns.lock().unwrap().remove(child_id);
                        match handle {
                            Some(h) => {
                                h.abort();
                                Ok(tools::ToolOutcome {
                                    observation: format!("sessao '{child_id}' abortada."),
                                    pending_edit: None,
                                })
                            }
                            None => Ok(tools::ToolOutcome {
                                observation: format!(
                                    "sessao '{child_id}' nao estava rodando (ja tinha \
                                     terminado, ou id invalido)."
                                ),
                                pending_edit: None,
                            }),
                        }
                    }
                    None => Err(anyhow::anyhow!("session_id obrigatorio")),
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
                    &session_id,
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
                // Sinal real de conclusao por tool call — sem isso a UI so
                // descobre que uma tool terminou ao inferir pelo proximo
                // "thinking" (heuristica que quebrava com >1 tool call no
                // mesmo turno, e sempre mostrava "done" mesmo em falha).
                let _ = app.emit(
                    "agent:tool_result",
                    ToolResultEvent {
                        session_id: session_id.clone(),
                        id: task_id.clone(),
                        status: task.status.clone(),
                        detail: task.detail.clone(),
                        additions: task.additions,
                        deletions: task.deletions,
                        duration_ms: task.duration_ms,
                        execution_id: None,
                    },
                );
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

            if !DOOM_LOOP_EXEMPT_TOOLS.contains(&call.function.name.as_str()) {
                recent_calls.push((call.function.name.clone(), call.function.arguments.clone()));
            }
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
    let reserve = ((context_length as f32) * COMPACT_RESERVE_RATIO)
        .clamp(COMPACT_RESERVE_MIN_TOKENS as f32, COMPACT_RESERVE_MAX_TOKENS as f32)
        as u32;
    let remaining = context_length.saturating_sub(estimate);
    if remaining > reserve {
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

    // Avisa o frontend ANTES de gastar tempo na chamada de resumo — sem
    // isso a tela fica travada em silencio por alguns segundos (achado
    // reportado ao vivo: parecia que o stream tinha simplesmente parado).
    let _ = app.emit(
        "agent:status",
        StatusEvent {
            session_id: session_id.to_string(),
            status: "compacting".to_string(),
        },
    );

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
    fork_id: Option<&str>,
) -> Result<(ProviderConfig, Option<String>)> {
    let config = state.config.lock().unwrap().clone();
    crate::build_provider_config(*kind, &config, &state.app_data_dir, custom_provider_id, fork_id)
        .map_err(|e| anyhow::anyhow!(e))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}...", s.chars().take(max).collect::<String>())
    } else {
        s.to_string()
    }
}

/// B3/Fase A3: se a sessão tem persona ativa E essa persona tem uma
/// allowlist de skills não-vazia, só permite `load_skill` pra skills dessa
/// lista — mesma semântica do filtro de `tools` já aplicado ao toolset
/// (T48). Sem persona ativa, ou persona sem `skills` preenchido, tudo
/// continua liberado (comportamento idêntico a antes desse campo existir).
fn skill_allowed_for_persona(app_data_dir: &Path, session: &crate::models::Session, skill_name: &str) -> bool {
    let Some(ref persona_id) = session.persona_id else {
        return true;
    };
    let Ok(personas) = crate::personas::list_personas(app_data_dir) else {
        return true;
    };
    let Some(persona) = personas.iter().find(|p| &p.id == persona_id) else {
        return true;
    };
    persona.skills.is_empty() || persona.skills.contains(&skill_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &str) -> (String, String) {
        (name.to_string(), args.to_string())
    }

    fn scratch_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cerne-mod-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn skill_allowed_for_persona_without_active_persona_allows_everything() {
        let dir = scratch_dir();
        let session = crate::sessions::create_session(
            &dir,
            "t".to_string(),
            crate::models::ProviderKind::Openrouter,
            "m".to_string(),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(skill_allowed_for_persona(&dir, &session, "qualquer-skill"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skill_allowed_for_persona_respects_allowlist_when_persona_active() {
        let dir = scratch_dir();
        let session = crate::sessions::create_session(
            &dir,
            "t".to_string(),
            crate::models::ProviderKind::Openrouter,
            "m".to_string(),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let persona = crate::personas::create_persona(
            &dir,
            "Restrito",
            "so pode resumir",
            vec![],
            vec!["summarize".to_string()],
            crate::personas::PersonaKind::Persona,
        )
        .unwrap();
        let session = crate::sessions::update_persona(&dir, &session.id, Some(persona.id)).unwrap();

        assert!(skill_allowed_for_persona(&dir, &session, "summarize"));
        assert!(!skill_allowed_for_persona(&dir, &session, "weather"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skill_allowed_for_persona_with_empty_allowlist_allows_everything() {
        let dir = scratch_dir();
        let session = crate::sessions::create_session(
            &dir,
            "t".to_string(),
            crate::models::ProviderKind::Openrouter,
            "m".to_string(),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let persona = crate::personas::create_persona(
            &dir,
            "Sem restricao",
            "prompt",
            vec![],
            vec![],
            crate::personas::PersonaKind::Persona,
        )
        .unwrap();
        let session = crate::sessions::update_persona(&dir, &session.id, Some(persona.id)).unwrap();

        assert!(skill_allowed_for_persona(&dir, &session, "qualquer-skill"));
        std::fs::remove_dir_all(&dir).ok();
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
