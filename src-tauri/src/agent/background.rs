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
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Child;

/// Quantas linhas de output (stdout+stderr combinados) manter por job — mais
/// que isso e descartado do inicio, tipo um `tail -f` com buffer limitado,
/// pra um dev server rodando por horas nao crescer sem limite na memoria.
const MAX_OUTPUT_LINES: usize = 2_000;

struct JobHandle {
    child: Child,
    command: String,
    output: Arc<Mutex<VecDeque<String>>>,
}

/// Registro dos processos em segundo plano ainda vivos (ou encerrados mas
/// ainda nao conferidos/removidos). Uma instancia vive em `AppState`.
#[derive(Default)]
pub struct BackgroundJobs(Mutex<HashMap<String, JobHandle>>);

impl BackgroundJobs {
    /// Inicia `command` em segundo plano dentro de `project_root` e devolve
    /// o id (UUID) pra consultar/parar depois. Nao espera o processo
    /// terminar — retorna assim que o SO confirma que o processo nasceu.
    pub fn start(&self, project_root: &Path, command: &str) -> Result<String> {
        let mut cmd = super::shell::build_shell_command(command);
        cmd.current_dir(project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow!("nao foi possivel iniciar o comando em segundo plano: {e}"))?;

        let output = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stdout) = child.stdout.take() {
            spawn_reader(stdout, output.clone(), None);
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_reader(stderr, output.clone(), Some("[stderr] "));
        }

        let id = uuid::Uuid::new_v4().to_string();
        self.0.lock().unwrap().insert(
            id.clone(),
            JobHandle {
                child,
                command: command.to_string(),
                output,
            },
        );
        Ok(id)
    }

    /// Le o output acumulado ate agora e o status atual (ainda rodando, ou
    /// encerrado com que codigo de saida) sem parar o processo.
    pub fn read_output(&self, id: &str) -> Result<String> {
        let mut jobs = self.0.lock().unwrap();
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
        self.0.lock().unwrap().get(id).and_then(|j| j.child.id())
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
            let mut jobs = self.0.lock().unwrap();
            jobs.remove(id).ok_or_else(|| {
                anyhow!("job em segundo plano '{id}' nao encontrado (id errado, ou ja foi parado)")
            })?
        };
        let command = job.command.clone();
        if let Some(pid) = job.child.id() {
            // Ignora falha do taskkill de proposito: o caso mais comum e o
            // processo ja ter morrido sozinho entre o ultimo check e o stop,
            // que nao e erro real (o objetivo do usuario ja estava satisfeito).
            let _ = tokio::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .output()
                .await;
        }
        drop(job); // kill_on_drop dispara aqui como rede de seguranca redundante, sem efeito (ja morto)
        Ok(format!("comando '{command}' (id {id}) encerrado"))
    }

    /// Lista todo job conhecido (rodando ou encerrado, ainda nao limpo) —
    /// util pro modelo checar "ja tem um dev server rodando de antes?" antes
    /// de subir outro.
    pub fn list(&self) -> String {
        let mut jobs = self.0.lock().unwrap();
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

fn spawn_reader<R>(reader: R, output: Arc<Mutex<VecDeque<String>>>, prefix: Option<&'static str>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut buf = output.lock().unwrap();
            buf.push_back(match prefix {
                Some(p) => format!("{p}{line}"),
                None => line,
            });
            if buf.len() > MAX_OUTPUT_LINES {
                buf.pop_front();
            }
        }
    });
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
        let id = jobs.start(&dir, "echo hello-from-background").unwrap();

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
        let id = jobs.start(&dir, "ping -n 20 127.0.0.1").unwrap();

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
    #[tokio::test]
    async fn stop_kills_the_whole_process_tree_not_just_cmd_exe() {
        let jobs = BackgroundJobs::default();
        let dir = std::env::temp_dir();
        let id = jobs.start(&dir, "ping -n 30 127.0.0.1").unwrap();

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

        let id = jobs.start(&dir, "echo listed").unwrap();
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
}
