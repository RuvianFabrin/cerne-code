//! Comandos de shell em segundo plano — `run_command` normal e sincrono
//! trava o loop do agente ate o processo terminar, o que nunca funciona pra
//! `npm run dev`, `cargo watch`, um build/teste longo, etc. (o processo so
//! "termina" quando alguem mata ele, entao a chamada sincrona nunca retorna).
//! Padrao inspirado no `task`/`monitor`/`get_task_output` do grok-build e no
//! `hub` (`start`/`ps`/`logs`/`stop`) do oh-my-pi, reduzido ao que o Cerne
//! precisa: sem guarda de profundidade de subagente (o Cerne ainda nao tem
//! subagentes), sem categorias de resumo de output por ferramenta (git/
//! docker/cargo etc. do oh-my-pi) — so acumula as ultimas linhas e devolve
//! cru.
//!
//! Escopo global ao app (nao por sessao): os jobs vivem em `AppState`, entao
//! `list_background`/`check_background_output` enxergam processos iniciados
//! por qualquer sessao. Pra um app local de um usuario so isso e uma
//! simplificacao razoavel — o id (UUID) devolvido na hora de iniciar e o que
//! de fato aponta pro processo certo, entao nao ha ambiguidade real de qual
//! job pertence a qual sessao.

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Child;

/// Intervalo mínimo entre eventos `agent:background_output` do MESMO job —
/// sem isso, um processo bem tagarela (build verboso, por exemplo) inundaria
/// o frontend com um evento por linha. O buffer completo continua sendo
/// acumulado a cada linha (`read_output`/`check_background_output` sempre
/// veem tudo); só o PUSH em tempo real é limitado.
const BACKGROUND_OUTPUT_EMIT_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Serialize, Clone)]
struct BackgroundOutputEvent {
    id: String,
    output: String,
}

/// Emitido quando um job em segundo plano termina de vez (ambos stdout e
/// stderr fecharam) — sinal pro frontend mostrar uma notificação, sem
/// precisar que o usuário/LLM fique conferindo (T14).
#[derive(Serialize, Clone)]
struct BackgroundDoneEvent {
    id: String,
    session_id: String,
    command: String,
    output: String,
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackgroundJobStatus {
    Running,
    Exited { code: Option<i32> },
    Unknown,
}

#[derive(Serialize, Clone)]
pub struct BackgroundJobInfo {
    pub id: String,
    pub command: String,
    pub status: BackgroundJobStatus,
    pub output: String,
    pub started_at_ms: i64,
    pub session_id: String,
}

/// Quantas linhas de output (stdout+stderr combinados) manter por job — mais
/// que isso e descartado do inicio, tipo um `tail -f` com buffer limitado,
/// pra um dev server rodando por horas nao crescer sem limite na memoria.
const MAX_OUTPUT_LINES: usize = 2_000;

struct JobHandle {
    child: Child,
    command: String,
    output: Arc<Mutex<VecDeque<String>>>,
    started_at_ms: i64,
    session_id: String,
}

/// Registro dos processos em segundo plano ainda vivos (ou encerrados mas
/// ainda nao conferidos/removidos). Uma instancia vive em `AppState`.
///
/// `app` fica `None` nos testes (que constroem via `Default`, sem app Tauri
/// de verdade) e `Some` em produção (`BackgroundJobs::new`, chamado no
/// `setup` do app) — quando `None`, o push de `agent:background_output`
/// simplesmente não dispara, sem quebrar nada (mesmo espírito de todo outro
/// `let _ = app.emit(...)` no código: melhor esforço, nunca crítico).
/// Teto de jobs rastreados ao mesmo tempo (rodando + já terminados, ainda
/// não "esquecidos") — sem isso, todo `run_command(background=true)` cujo
/// processo termina SOZINHO (a maioria: build, teste, git, etc. — só o
/// `stop()` explícito removia do mapa) ficava pra sempre em memória, com o
/// `Child` handle e o buffer de output inteiro (até `MAX_OUTPUT_LINES`
/// cada) nunca liberados. **Vazamento de memória real, confirmado**: Log de
/// Eventos do Windows mostrou `RADAR_PRE_LEAK_64` (heurística de vazamento
/// do próprio Windows) pouco mais de uma hora antes de um crash por falha
/// fatal de alocação (`0xc0000409`) numa sessão de uso prolongado —
/// achado investigando o relato do usuário, 2026-08-17. Só remove jobs JÁ
/// TERMINADOS (nunca um rodando) quando o total passa do teto, começando
/// pelo mais antigo.
const MAX_TRACKED_JOBS: usize = 50;

fn prune_finished_jobs(jobs: &mut HashMap<String, JobHandle>) {
    if jobs.len() < MAX_TRACKED_JOBS {
        return;
    }
    let mut finished: Vec<(String, i64)> = jobs
        .iter_mut()
        .filter_map(|(id, job)| match job.child.try_wait() {
            Ok(Some(_)) => Some((id.clone(), job.started_at_ms)),
            _ => None,
        })
        .collect();
    finished.sort_by_key(|(_, started_at_ms)| *started_at_ms);
    let excess = jobs.len() + 1 - MAX_TRACKED_JOBS;
    for (id, _) in finished.into_iter().take(excess) {
        jobs.remove(&id);
    }
}

#[derive(Default)]
pub struct BackgroundJobs {
    jobs: Mutex<HashMap<String, JobHandle>>,
    app: Option<AppHandle>,
}

impl BackgroundJobs {
    pub fn new(app: AppHandle) -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            app: Some(app),
        }
    }

    /// Inicia `command` em segundo plano dentro de `project_root` e devolve
    /// o id (UUID) pra consultar/parar depois. Nao espera o processo
    /// terminar — retorna assim que o SO confirma que o processo nasceu.
    ///
    /// `app_data_dir`/`session_id` sao pra T14 (callback automatico): quando
    /// o job termina de vez, o resultado e injetado no historico dessa sessao
    /// sem o LLM precisar ficar chamando `check_background_output` de novo.
    pub fn start(
        &self,
        project_root: &Path,
        command: &str,
        app_data_dir: &Path,
        session_id: &str,
    ) -> Result<String> {
        prune_finished_jobs(&mut self.jobs.lock().unwrap());
        let mut cmd = super::shell::build_shell_command(command);
        // Unix: grupo de processo proprio — pre-condicao pro kill de arvore
        // via `kill -9 -<pgid>` no stop() (ver shell::apply_process_group).
        super::shell::apply_process_group(&mut cmd);
        cmd.current_dir(project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("nao foi possivel iniciar o comando em segundo plano: {e}"))?;

        let id = uuid::Uuid::new_v4().to_string();
        let output = Arc::new(Mutex::new(VecDeque::new()));
        let last_emit: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        // Contador de streams (stdout+stderr) ainda abertos pra esse job —
        // quando os dois fecham (chegou a 0), o processo terminou de vez;
        // qualquer um dos dois readers pode ser o ultimo a fechar.
        let remaining_streams = Arc::new(Mutex::new(2u8));
        let completion = JobCompletionCtx {
            app: self.app.clone(),
            job_id: id.clone(),
            session_id: session_id.to_string(),
            command: command.to_string(),
            app_data_dir: app_data_dir.to_path_buf(),
            remaining_streams,
        };
        if let Some(stdout) = child.stdout.take() {
            spawn_reader(
                stdout,
                output.clone(),
                None,
                last_emit.clone(),
                completion.clone(),
            );
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_reader(
                stderr,
                output.clone(),
                Some("[stderr] "),
                last_emit.clone(),
                completion.clone(),
            );
        }

        self.jobs.lock().unwrap().insert(
            id.clone(),
            JobHandle {
                child,
                command: command.to_string(),
                output,
                started_at_ms: chrono::Utc::now().timestamp_millis(),
                session_id: session_id.to_string(),
            },
        );
        Ok(id)
    }

    /// Le o output acumulado ate agora e o status atual (ainda rodando, ou
    /// encerrado com que codigo de saida) sem parar o processo.
    pub fn read_output(&self, id: &str) -> Result<String> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs.get_mut(id).ok_or_else(|| {
            anyhow!("job em segundo plano '{id}' nao encontrado (id errado, ou ja foi parado)")
        })?;
        let status = match job.child.try_wait() {
            Ok(Some(exit_status)) => {
                format!(
                    "encerrado (codigo {})",
                    exit_status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "desconhecido".to_string())
                )
            }
            Ok(None) => "rodando".to_string(),
            Err(e) => format!("erro ao verificar status: {e}"),
        };
        let output = job
            .output
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!(
            "comando: {}\nstatus: {status}\noutput (ultimas {MAX_OUTPUT_LINES} linhas):\n{output}",
            job.command
        ))
    }

    /// PID do `cmd.exe` que o tokio rastreia pra esse job — so exposto pra
    /// teste confirmar que o processo real (filho do cmd.exe) morre junto
    /// no `stop`, sem depender de checar por nome de imagem (que colide com
    /// outros testes rodando `ping` em paralelo no mesmo processo de teste).
    #[cfg(test)]
    fn cmd_pid(&self, id: &str) -> Option<u32> {
        self.jobs.lock().unwrap().get(id).and_then(|j| j.child.id())
    }

    /// Mata o processo (arvore inteira) e remove o job do registro.
    ///
    /// **Achado ao testar ao vivo**: `child.start_kill()` sozinho mata so o
    /// `cmd.exe` que o tokio rastreia — no Windows, matar o processo pai nao
    /// mata os filhos dele automaticamente (diferente do que se poderia
    /// supor). Como todo comando aqui roda via `cmd /C <command>`, o
    /// processo de verdade (`python`, `node`, etc.) e FILHO do `cmd.exe`, e
    /// ficava orfao rodando depois do "stop" — confirmado ao vivo: um
    /// `python server.py` continuou respondendo por HTTP depois do
    /// `stop_background` reportar sucesso. Corrigido com `taskkill /PID <pid>
    /// /T /F`, que mata a arvore de processo inteira pelo PID, nao so o
    /// processo direto.
    pub async fn stop(&self, id: &str) -> Result<String> {
        // `job` precisa continuar vivo (nao dropado) ate o taskkill
        // terminar: o `Child` foi criado com `kill_on_drop(true)`, entao
        // dropar `job` cedo demais mata o cmd.exe sozinho ANTES do taskkill
        // rodar — e o `/T` (arvore) do taskkill precisa que o processo pai
        // ainda exista na hora da chamada pra conseguir montar a arvore de
        // filhos; se o pai ja morreu, o taskkill falha com "processo nao
        // encontrado" e os filhos (o processo de verdade) ficam orfaos.
        // Achado depurando um teste que falhava so as vezes.
        let job = {
            let mut jobs = self.jobs.lock().unwrap();
            jobs.remove(id).ok_or_else(|| {
                anyhow!("job em segundo plano '{id}' nao encontrado (id errado, ou ja foi parado)")
            })?
        };
        let command = job.command.clone();
        if let Some(pid) = job.child.id() {
            // Ignora falha do kill de proposito: o caso mais comum e o
            // processo ja ter morrido sozinho entre o ultimo check e o stop,
            // que nao e erro real (o objetivo do usuario ja estava satisfeito).
            //
            // Windows: `taskkill /T /F` mata a arvore inteira (fix original
            // do bug do processo orfao — NAO simplificar, ver README).
            // Unix: mesmo problema existia com `kill -9 <pid>` (so matava o
            // /bin/sh -c); agora usa process group (ver shell.rs) — o spawn
            // em `start()` coloca o processo num grupo proprio, entao um
            // `kill -9 -<pgid>` alcanca shell + filhos de uma vez.
            #[cfg(windows)]
            {
                let mut taskkill = tokio::process::Command::new("taskkill");
                taskkill.args(["/PID", &pid.to_string(), "/T", "/F"]);
                super::shell::apply_creation_flags(&mut taskkill);
                let _ = taskkill.output().await;
            }
            #[cfg(not(windows))]
            {
                let _ = tokio::task::spawn_blocking(move || {
                    super::shell::kill_pid_tree_blocking(pid);
                })
                .await;
            }
        }
        drop(job); // kill_on_drop dispara aqui como rede de seguranca redundante, sem efeito (ja morto)
        Ok(format!("comando '{command}' (id {id}) encerrado"))
    }

    /// Mata TODOS os jobs ainda vivos de uma vez, via `taskkill /T /F` —
    /// usado no encerramento do app (ver `lib.rs::run`) pra nao deixar dev
    /// servers/processos filhos orfaos rodando depois que o Cerne fecha.
    /// Sincrono de proposito: o handler de saida do Tauri nao e async, e
    /// `kill_on_drop`/`start_kill()` sozinhos nao sao confiaveis nesse
    /// momento (o runtime pode nao ter chance de rodar o kill assincrono
    /// antes do processo do app sumir - mesmo achado documentado em `stop`
    /// acima, so que ali o contexto e async e da pra fazer `.await`).
    pub fn kill_all_blocking(&self) {
        let pids: Vec<u32> = {
            let jobs = self.jobs.lock().unwrap();
            jobs.values().filter_map(|j| j.child.id()).collect()
        };
        for pid in pids {
            super::shell::kill_pid_tree_blocking(pid);
        }
        self.jobs.lock().unwrap().clear();
    }

    /// Versão estruturada de `list()` — pro painel da UI (Fase C1), que
    /// precisa dos campos separados pra renderizar (não pro LLM, que usa o
    /// texto formatado de `list()`).
    pub fn list_structured(&self) -> Vec<BackgroundJobInfo> {
        let mut jobs = self.jobs.lock().unwrap();
        let mut result: Vec<BackgroundJobInfo> = jobs
            .iter_mut()
            .map(|(id, job)| {
                let status = match job.child.try_wait() {
                    Ok(Some(exit_status)) => BackgroundJobStatus::Exited {
                        code: exit_status.code(),
                    },
                    Ok(None) => BackgroundJobStatus::Running,
                    Err(_) => BackgroundJobStatus::Unknown,
                };
                let output = job
                    .output
                    .lock()
                    .unwrap()
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                BackgroundJobInfo {
                    id: id.clone(),
                    command: job.command.clone(),
                    status,
                    output,
                    started_at_ms: job.started_at_ms,
                    session_id: job.session_id.clone(),
                }
            })
            .collect();
        // Mais recente primeiro — `jobs` é um HashMap (ordem de iteração
        // arbitrária), sem isso a lista pulava de ordem a cada refresh.
        result.sort_by_key(|j| std::cmp::Reverse(j.started_at_ms));
        result
    }

    /// Lista todo job conhecido (rodando ou encerrado, ainda nao limpo) —
    /// util pro modelo checar "ja tem um dev server rodando de antes?" antes
    /// de subir outro.
    pub fn list(&self) -> String {
        let mut jobs = self.jobs.lock().unwrap();
        if jobs.is_empty() {
            return "nenhum comando em segundo plano".to_string();
        }
        jobs.iter_mut()
            .map(|(id, job)| {
                let status = match job.child.try_wait() {
                    Ok(Some(exit_status)) => {
                        format!(
                            "encerrado (codigo {})",
                            exit_status
                                .code()
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "desconhecido".to_string())
                        )
                    }
                    Ok(None) => "rodando".to_string(),
                    Err(_) => "status desconhecido".to_string(),
                };
                format!("{id}\t{status}\t{}", job.command)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Tudo que o callback de conclusao (T14) precisa, agrupado pra nao
/// carregar 6 parametros soltos entre os dois readers (stdout/stderr) de um
/// mesmo job.
#[derive(Clone)]
struct JobCompletionCtx {
    app: Option<AppHandle>,
    job_id: String,
    session_id: String,
    command: String,
    app_data_dir: PathBuf,
    remaining_streams: Arc<Mutex<u8>>,
}

fn spawn_reader<R>(
    reader: R,
    output: Arc<Mutex<VecDeque<String>>>,
    prefix: Option<&'static str>,
    last_emit: Arc<Mutex<Option<Instant>>>,
    completion: JobCompletionCtx,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        // `read_until` em vez de `lines()`: `lines()` exige UTF-8 válido e
        // devolve `Err` no primeiro byte inválido — o que **encerrava o loop**
        // (`while let Ok(Some(..))` simplesmente saía). Ou seja: um comando em
        // segundo plano que imprimisse um acento no codepage do console parava
        // de ser capturado no meio, em silêncio. Agora lê os bytes crus e
        // decodifica com `decode_output`, que nunca aborta a leitura.
        let mut reader = BufReader::new(reader);
        let mut raw: Vec<u8> = Vec::new();
        loop {
            raw.clear();
            match reader.read_until(b'\n', &mut raw).await {
                Ok(0) => break,   // EOF
                Ok(_) => {}
                Err(_) => break,
            }
            // `strip_ansi` pelo mesmo motivo do `run_command`: build/teste
            // colorido despejava código de escape no painel de background.
            let line = super::shell::strip_ansi(&super::shell::decode_output(&raw));
            let line = line.trim_end_matches(['\n', '\r']).to_string();
            let joined = {
                let mut buf = output.lock().unwrap();
                buf.push_back(match prefix {
                    Some(p) => format!("{p}{line}"),
                    None => line,
                });
                if buf.len() > MAX_OUTPUT_LINES {
                    buf.pop_front();
                }
                buf.iter().cloned().collect::<Vec<_>>().join("\n")
            };

            if let Some(app) = &completion.app {
                let should_emit = {
                    let mut last = last_emit.lock().unwrap();
                    let now = Instant::now();
                    let ready = last
                        .map(|t| now.duration_since(t) >= BACKGROUND_OUTPUT_EMIT_INTERVAL)
                        .unwrap_or(true);
                    if ready {
                        *last = Some(now);
                    }
                    ready
                };
                if should_emit {
                    let _ = app.emit(
                        "agent:background_output",
                        BackgroundOutputEvent {
                            id: completion.job_id.clone(),
                            output: joined,
                        },
                    );
                }
            }
        }

        // Esse stream (stdout ou stderr) fechou — quando os dois fecharem
        // (contador chega a 0), o processo terminou de vez.
        let is_last = {
            let mut remaining = completion.remaining_streams.lock().unwrap();
            *remaining = remaining.saturating_sub(1);
            *remaining == 0
        };
        if is_last {
            on_job_finished(completion, output).await;
        }
    });
}

/// Callback do T14: quando um job em segundo plano termina de vez, avisa a
/// UI (`agent:background_done`) e injeta uma mensagem no historico da sessao
/// que o iniciou, pra o LLM ver o resultado no proximo turno sem precisar
/// ficar chamando `check_background_output` repetidamente. Melhor esforco —
/// falhas aqui (sessao ja apagada, etc.) nao devem derrubar nada.
async fn on_job_finished(completion: JobCompletionCtx, output: Arc<Mutex<VecDeque<String>>>) {
    let final_output = output
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    if let Some(app) = &completion.app {
        let _ = app.emit(
            "agent:background_done",
            BackgroundDoneEvent {
                id: completion.job_id.clone(),
                session_id: completion.session_id.clone(),
                command: completion.command.clone(),
                output: final_output.clone(),
            },
        );
    }

    if completion.session_id.is_empty() {
        return;
    }
    // Preview curto pro chat (`display_content`) — o output completo (ate
    // MAX_OUTPUT_LINES) so vai pro `content`, que e o que o LLM realmente le
    // no proximo turno. Sem essa distincao, um build/teste verboso vira uma
    // bolha de chat gigante; com ela, a tela mostra so as ultimas linhas e o
    // agente ainda tem o output inteiro disponivel se precisar investigar.
    const PREVIEW_LINES: usize = 12;
    let preview: String = final_output
        .lines()
        .rev()
        .take(PREVIEW_LINES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let note = crate::models::ChatMessage {
        role: "system".to_string(),
        content: format!(
            "[Comando em segundo plano concluido]\ncomando: {}\nid: {}\noutput (ultimas {MAX_OUTPUT_LINES} linhas):\n{}",
            completion.command, completion.job_id, final_output
        ),
        tool_calls: None,
        tool_call_id: None,
        // Marca essa mensagem como a nota de conclusao do T14 pro frontend
        // saber renderiza-la de um jeito distinto (nao e o system prompt
        // real, que tambem usa role "system" mas nunca deveria aparecer no
        // chat visivel).
        name: Some("background_job_done".to_string()),
        images: Vec::new(),
        display_content: Some(format!(
            "✅ Comando em segundo plano concluído: `{}`\n\n```\n{}\n```",
            completion.command, preview
        )),
    };
    if let Ok(mut messages) =
        crate::sessions::load_messages(&completion.app_data_dir, &completion.session_id)
    {
        messages.push(note);
        // Achado ao vivo (2026-09-12, sessão travada com 400 "An assistant
        // message with 'tool_calls' must be followed by tool messages"): essa
        // nota é injetada a qualquer momento, inclusive quando o turno que
        // iniciou o job ainda está no meio da execução — entre o `tool_calls`
        // já salvo em disco e o resultado da ferramenta ainda não salvo. Sem o
        // reparo, a nota entra no meio do grupo e o histórico inteiro passa a
        // ser rejeitado pelo provider em toda mensagem seguinte.
        crate::history::repair(&mut messages);
        let _ = crate::sessions::save_messages(
            &completion.app_data_dir,
            &completion.session_id,
            &messages,
        );
    }

    // Sem isso, a nota acima so ficava salva em disco, inerte, ate o
    // usuario mandar outra mensagem por conta propria - o LLM nunca reagia
    // ao comando ter terminado (achado reportado ao vivo). Guardado contra
    // sessao ja ocupada e loop infinito dentro de `spawn_auto_continue_turn`.
    if let Some(app) = &completion.app {
        super::spawn_auto_continue_turn(
            app.clone(),
            completion.session_id.clone(),
            "Um comando em segundo plano que voce iniciou acabou de terminar - o resultado ja \
             esta no historico acima. Confira o output e continue a tarefa original a partir \
             daqui."
                .to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Poll até o output conter `needle` (ou estourar ~6s). Sleep fixo era
    /// flaky quando a suíte inteira roda em paralelo e o cmd demora a subir.
    async fn wait_for(jobs: &BackgroundJobs, id: &str, needle: &str) -> String {
        for _ in 0..60 {
            if let Ok(out) = jobs.read_output(id) {
                if out.contains(needle) {
                    return out;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        jobs.read_output(id).unwrap_or_else(|e| format!("<job sumiu: {e}>"))
    }

    #[tokio::test]
    async fn start_read_and_stop_a_background_command() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        let id = jobs.start(&dir, "echo hello-from-background", &dir, "test-session").unwrap();

        let output = wait_for(&jobs, &id, "hello-from-background").await;
        assert!(
            output.contains("hello-from-background"),
            "esperava ver o output, recebeu: {output}"
        );
        let output = wait_for(&jobs, &id, "encerrado").await;
        assert!(
            output.contains("encerrado"),
            "echo termina rapido, deveria ja estar encerrado: {output}"
        );

        jobs.stop(&id).await.unwrap();
        assert!(
            jobs.read_output(&id).is_err(),
            "apos stop o job deveria sumir do registro"
        );
    }

    #[tokio::test]
    async fn stop_kills_a_still_running_command() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        // ping localhost e um jeito portavel de ter um processo Windows que
        // fica rodando por alguns segundos, pra testar "parar enquanto ainda roda".
        let id = jobs.start(&dir, "ping -n 20 127.0.0.1", &dir, "test-session").unwrap();

        let output = wait_for(&jobs, &id, "status: rodando").await;
        assert!(
            output.contains("status: rodando"),
            "deveria ainda estar rodando: {output}"
        );

        let stop_msg = jobs.stop(&id).await.unwrap();
        assert!(stop_msg.contains("encerrado"));
        assert!(jobs.read_output(&id).is_err());
    }

    /// Regressao do bug encontrado testando ao vivo: todo comando roda via
    /// `cmd /C <command>`, entao o processo de verdade e FILHO do cmd.exe que
    /// o tokio rastreia. Um `stop_background` que so mata o cmd.exe
    /// (`child.start_kill()`) deixa esse filho orfao rodando — confirmado ao
    /// vivo com um `python server.py` que continuou respondendo por HTTP
    /// depois do stop reportar sucesso. `ping` sempre spawna `PING.EXE` como
    /// processo filho real e separado, o suficiente pra reproduzir o mesmo
    /// formato do bug sem depender de python/node instalado. Checa pelo PID
    /// especifico do filho (via PowerShell/CIM), nao por nome de imagem —
    /// nome de imagem colidiria com o `ping` de outro teste rodando em
    /// paralelo no mesmo processo de teste.
    ///
    /// Versao Unix (mesmo proposito, mesma classe de bug): o comando roda via
    /// `/bin/sh -c "sleep 30"`, o sleep e FILHO do sh. Antes do fix do
    /// process group, o stop so matava o sh (`kill -9 <pid>`) e o sleep
    /// ficava orfao. Agora o kill de grupo (`kill -9 -<pgid>`) deve alcancar
    /// o filho tambem — checado via `/proc/<pid>` (Linux) ou `ps -p`
    /// (macOS/compativel), sem depender de inspecionar arvores por fora.
    #[cfg(windows)]
    #[tokio::test]
    async fn stop_kills_the_whole_process_tree_not_just_cmd_exe() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        let id = jobs.start(&dir, "ping -n 30 127.0.0.1", &dir, "test-session").unwrap();

        let cmd_pid = jobs
            .cmd_pid(&id)
            .expect("job deveria ter pid enquanto roda");
        let mut ping_pid: Option<u32> = None;
        for _ in 0..60 {
            if let Some(pid) = child_pid_of(cmd_pid).await {
                ping_pid = Some(pid);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let ping_pid =
            ping_pid.expect("cmd.exe deveria ter spawnado PING.EXE como processo filho com PID proprio");
        assert!(
            pid_exists(ping_pid).await,
            "PING.EXE (pid {ping_pid}) deveria estar rodando antes do stop"
        );

        jobs.stop(&id).await.unwrap();
        let mut gone = false;
        for _ in 0..60 {
            if !pid_exists(ping_pid).await {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            gone,
            "PING.EXE (pid {ping_pid}) deveria ter morrido junto com o cmd.exe - bug do processo orfao voltou se isso falhar"
        );
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn stop_kills_the_whole_process_tree_not_just_sh() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        // IMPORTANTE (achado depurando no WSL): shells POSIX fazem EXEC()
        // direto quando `sh -c <comando-simples>` — o shell E substituido
        // pelo comando e o PID rastreado pelo tokio JA E o processo final
        // (sem arvore). Nesses casos kill -9 <pid> puro bastaria. O bug de
        // arvore so existe quando o shell PERMANECE vivo com filhos: comando
        // composto (`;`) + subshell explicito `( ... )` garante isso. Aqui:
        // `(sleep 30 & wait)` mantem o bash vivo com o sleep como FILHO real.
        // Verificar via /proc quem e o filho do shell rastreado e exigir que
        // ele morra junto no stop — se o kill so alcancar o shell, o sleep
        // fica orfao e este teste falha (mesma semantica do teste Windows).
        let id = jobs.start(&dir, "(sleep 30 & wait)", &dir, "test-session").unwrap();

        let sh_pid = jobs
            .cmd_pid(&id)
            .expect("job deveria ter pid enquanto roda");
        let mut sleep_pid: Option<u32> = None;
        for _ in 0..60 {
            if let Some(pid) = child_pid_of(sh_pid) {
                sleep_pid = Some(pid);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let sleep_pid =
            sleep_pid.expect("/bin/sh deveria ter spawnado o sleep como processo filho com PID proprio");
        assert!(
            pid_exists(sleep_pid),
            "sleep (pid {sleep_pid}) deveria estar rodando antes do stop"
        );

        jobs.stop(&id).await.unwrap();
        let mut gone = false;
        for _ in 0..60 {
            if !pid_exists(sleep_pid) {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            gone,
            "sleep (pid {sleep_pid}) deveria ter morrido junto com o sh - bug do processo orfao voltou se isso falhar"
        );
    }

    #[cfg(windows)]
    async fn child_pid_of(parent_pid: u32) -> Option<u32> {
        let output = tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-CimInstance Win32_Process -Filter \"ParentProcessId={parent_pid}\").ProcessId"),
            ])
            .output()
            .await
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .lines()
            .next()?
            .trim()
            .parse::<u32>()
            .ok()
    }

    /// Filho direto de um PID no Unix — le `/proc/<pid>/task/<tid>/children`
    /// (Linux; primeira linha tem PIDs separados por espaco). Em macOS nao ha
    /// /proc: usa `pgrep -P`. Retorna o primeiro filho encontrado.
    #[cfg(not(windows))]
    fn child_pid_of(parent_pid: u32) -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            let children_path = format!("/proc/{parent_pid}/task/{parent_pid}/children");
            let content = std::fs::read_to_string(children_path).ok()?;
            return content
                .split_whitespace()
                .next()?
                .parse::<u32>()
                .ok();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let output = std::process::Command::new("pgrep")
                .args(["-P", &parent_pid.to_string()])
                .output()
                .ok()?;
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()?
                .parse::<u32>()
                .ok()
        }
    }

    #[cfg(not(windows))]
    fn pid_exists(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }

    #[cfg(windows)]
    async fn pid_exists(pid: u32) -> bool {
        let output = tokio::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output()
            .await
            .unwrap();
        String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
    }

    #[tokio::test]
    async fn list_shows_known_jobs() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        assert_eq!(jobs.list(), "nenhum comando em segundo plano");

        let id = jobs.start(&dir, "echo listed", &dir, "test-session").unwrap();
        let listing = jobs.list();
        assert!(listing.contains(&id));
        assert!(listing.contains("echo listed"));

        jobs.stop(&id).await.unwrap();
    }

    #[tokio::test]
    async fn read_output_errors_for_unknown_id() {
        let jobs = BackgroundJobs::default();
        assert!(jobs.read_output("not-a-real-id").is_err());
    }

    /// T14: quando o job termina de vez (stdout e stderr fecham), o
    /// resultado deve aparecer sozinho no historico da sessao que o
    /// iniciou — sem o LLM precisar chamar `check_background_output` de
    /// novo pra descobrir que terminou.
    #[tokio::test]
    async fn finished_job_injects_message_into_session_history() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        let app_data_dir = dir.join(format!("cerne-t14-test-{}", uuid::Uuid::new_v4()));
        let session_id = "orquestrador-t14";

        let id = jobs
            .start(&dir, "echo t14-done", &app_data_dir, session_id)
            .unwrap();

        // Espera o job terminar de verdade (nao so aparecer no output).
        let _ = wait_for(&jobs, &id, "encerrado").await;

        let mut injected = Vec::new();
        for _ in 0..60 {
            injected = crate::sessions::load_messages(&app_data_dir, session_id).unwrap_or_default();
            if injected.iter().any(|m| m.content.contains("t14-done")) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            injected.iter().any(|m| m.role == "system" && m.content.contains("t14-done")),
            "esperava uma mensagem 'system' com o output do job injetada na sessao, recebeu: {injected:?}"
        );
        // T14 (achado testando ao vivo, 2026-08-16): a nota precisa do
        // marcador `name` pro frontend distinguir do system prompt real, e
        // de um `display_content` curto pra nao virar uma bolha gigante no
        // chat visivel quando o output for grande.
        let note = injected
            .iter()
            .find(|m| m.role == "system" && m.name.as_deref() == Some("background_job_done"))
            .expect("nota deveria ter name = background_job_done");
        assert!(
            note.display_content.as_deref().unwrap_or("").contains("t14-done"),
            "display_content deveria trazer um preview do output: {note:?}"
        );

        jobs.stop(&id).await.ok();
        fs_remove_dir_all_ignore(&app_data_dir);
    }

    fn fs_remove_dir_all_ignore(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Regressão do 400 que travou a sessão do usuário em 2026-09-12 ("An
    /// assistant message with 'tool_calls' must be followed by tool messages
    /// responding to each 'tool_call_id'"): o job em segundo plano terminou
    /// enquanto o histórico tinha um `tool_calls` recém-salvo e ainda sem
    /// resposta, e a nota injetada entrou NO MEIO do grupo. Depois do reparo,
    /// a nota (e qualquer mensagem seguinte) fica DEPOIS de uma resposta —
    /// sintética, já que a real nunca chegou a ser salva.
    #[tokio::test]
    async fn finished_job_never_breaks_a_tool_call_group_in_history() {
        use crate::models::{ChatMessage, ToolCall, ToolCallFunction};

        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        let app_data_dir = dir.join(format!("cerne-t14-orfao-test-{}", uuid::Uuid::new_v4()));
        let session_id = "sessao-com-tool-call-orfao";

        let assistant = ChatMessage {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "call_orfa_1".to_string(),
                kind: "function".to_string(),
                function: ToolCallFunction {
                    name: "run_command".to_string(),
                    arguments: "{\"background\":true}".to_string(),
                },
            }]),
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        };
        let historico = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "prompt".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: "roda o teste em background".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: Vec::new(),
                display_content: None,
            },
            assistant,
        ];
        crate::sessions::save_messages(&app_data_dir, session_id, &historico).unwrap();

        let id = jobs
            .start(&dir, "echo t14-orfao", &app_data_dir, session_id)
            .unwrap();
        let _ = wait_for(&jobs, &id, "encerrado").await;

        let mut messages = Vec::new();
        for _ in 0..60 {
            messages = crate::sessions::load_messages(&app_data_dir, session_id).unwrap_or_default();
            if messages.iter().any(|m| m.role == "tool") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        assert!(
            messages
                .iter()
                .any(|m| m.name.as_deref() == Some("background_job_done")),
            "a nota de conclusao deveria ter sido injetada: {messages:?}"
        );
        // A invariante que o provider exige: todo `tool_calls` seguido
        // imediatamente pelas respostas dos seus ids.
        for (i, m) in messages.iter().enumerate() {
            for call in m.tool_calls.iter().flatten() {
                let next = messages.get(i + 1).expect("resposta depois do pedido");
                assert_eq!(
                    next.role, "tool",
                    "mensagem seguinte ao tool_calls deveria ser a resposta, veio {}",
                    next.role
                );
                assert_eq!(next.tool_call_id.as_deref(), Some(call.id.as_str()));
            }
        }

        jobs.stop(&id).await.ok();
        fs_remove_dir_all_ignore(&app_data_dir);
    }
}
