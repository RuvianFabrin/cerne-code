//! Diff de repositório de verdade (Fase D1 revisitada, 2026-08-17): antes o
//! `RepoDiffViewer.vue` reconstruía os diffs a partir do histórico de tool
//! calls da sessão (`TaskItem`/`AgentExecution.steps`) — funcionava, mas só
//! via IN/OUT do que o agente escreveu por ela, não o estado real do
//! repositório (perdia edições manuais do usuário, não distinguia arquivo
//! novo/deletado/renomeado direito). Pedido explícito do usuário testando ao
//! vivo: pegar o diff do `git` de verdade, igual o painel "Controle do
//! Código-Fonte" do VS Code — este módulo faz exatamente isso, chamando o
//! binário `git` via subprocesso (mesmo espírito de `shell.rs`, sem
//! dependência de crate externa pra git).

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct GitFileChange {
    /// Caminho relativo à raiz passada (`project_root`) — resolvido com
    /// `--relative` nos comandos que suportam, pra não depender de
    /// `project_root` ser exatamente a raiz do repositório git (pode ser
    /// uma subpasta dele).
    pub path: String,
    pub status: String, // "modified" | "added" | "deleted" | "renamed" | "untracked"
    pub additions: u32,
    pub deletions: u32,
}

/// Teto de tempo pra qualquer chamada `git` — comandos locais (status/diff)
/// terminam em milissegundos, então isso só entra em ação nos que tocam rede
/// (`pull`/`push` do backup via git). Achado testando ao vivo (2026-08-18):
/// usuário clicou em "Sincronizar" sem querer e o modal de Configurações
/// travou — sem remoto configurado corretamente (ou pedindo credencial que o
/// git não tem como pedir num processo sem terminal), `git pull`/`push`
/// ficava esperando pra sempre, e como `run_git` não tinha timeout nenhum
/// (diferente de `run_command`/teste de conexão MCP, que já tinham), a
/// Promise no frontend nunca resolvia.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

fn run_git(project_root: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(project_root);
    // GIT_TERMINAL_PROMPT=0 faz o git falhar na hora com um erro claro
    // quando precisaria pedir usuário/senha (sem terminal pra pedir de
    // verdade aqui) em vez de travar esperando uma entrada que nunca vem —
    // mesma causa raiz do travamento do modal de Configurações descrito
    // acima, resolvida na origem (o timeout abaixo é a rede de segurança
    // pra qualquer outro jeito de travar que passe por cima disso).
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // Sem isso, no Windows cada chamada (git status/diff rodando toda vez
    // que o usuario troca de sessao ou abre o visualizador de diff) faz uma
    // janela de console preta piscar na tela — achado testando ao vivo,
    // 2026-08-17 (mesmo cuidado que shell.rs ja tem pros comandos do
    // agente, so que faltava aqui pro `git` por ser std::process::Command
    // em vez de tokio::process::Command).
    crate::agent::shell::apply_std_creation_flags(&mut cmd);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let child = cmd
        .spawn()
        .map_err(|e| anyhow!("nao foi possivel executar o git (esta instalado e no PATH?): {e}"))?;

    // Watchdog numa thread separada em vez de só dar poll com `try_wait` —
    // ler stdout/stderr só DEPOIS do processo terminar arrisca um deadlock
    // de verdade se a saída passar do buffer do pipe do SO (64KB tipico no
    // Windows) antes de alguem drenar: o processo trava esperando escrever,
    // ninguem le, ninguem espera. `wait_with_output()` já drena os pipes em
    // paralelo enquanto espera, então é seguro pra saída grande — só precisa
    // de alguem matando o processo se passar do timeout, daí a thread.
    let pid_for_kill = child.id();
    let killed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let killed_clone = killed.clone();
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(GIT_TIMEOUT);
        killed_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        #[cfg(windows)]
        {
            let mut kill_cmd = std::process::Command::new("taskkill");
            kill_cmd.args(["/PID", &pid_for_kill.to_string(), "/T", "/F"]);
            crate::agent::shell::apply_std_creation_flags(&mut kill_cmd);
            let _ = kill_cmd.output();
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid_for_kill.to_string()])
                .output();
        }
    });

    let output = child
        .wait_with_output()
        .map_err(|e| anyhow!("erro esperando o git terminar: {e}"))?;
    // Processo terminou (sozinho ou morto pelo watchdog) — não precisa mais
    // do watchdog rodando; se ele já disparou o kill, isso é só um no-op.
    drop(watchdog);

    if killed.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(anyhow!(
            "git {} nao respondeu em {}s (processo encerrado) — pode estar pedindo \
             credencial que nao tem como digitar aqui, ou a rede esta travada",
            args.join(" "),
            GIT_TIMEOUT.as_secs()
        ));
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git {} falhou: {}", args.join(" "), stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---------------------------------------------------------------------
// Backup/sincronização de SESSÕES via git (alternativa ao `.zip`).
// Repo próprio,
// raiz em `<app_data_dir>/sessions` (NÃO o app_data_dir inteiro — arquivos
// como `mcp_servers.json` podem ter segredo em texto puro no campo `env`,
// nunca deveriam ir pra um remoto git por engano). Conflito de merge não é
// resolvido automaticamente (JSON não tem merge seguro) — o erro do próprio
// git sobe pro chamador, que mostra pro usuário resolver.
//
// Autenticação: pedido explícito do usuário (2026-08-18) depois de travar o
// modal de Configurações tentando sincronizar sem credencial nenhuma — em
// vez de depender só do credential helper do SO (que pode simplesmente não
// existir configurado, daí o travamento), o Cerne agora permite configurar
// nome/email locais (só pra essa pasta, nunca `--global`) e um token, salvo
// no cofre de credenciais do sistema via `keyring` (mesmo mecanismo da
// chave do OpenRouter, `config.rs`) — nunca em texto puro no `.git/config`.
// Quando o remoto é `https://` e há token salvo, ele é injetado só no
// ARGUMENTO da URL de cada `pull`/`push` (nunca gravado no remoto
// permanente) — SSH continua 100% por conta do agente SSH do SO, token não
// se aplica.
// ---------------------------------------------------------------------

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").is_dir()
}

pub fn init_repo(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    run_git(dir, &["init"])?;
    Ok(())
}

pub fn set_remote(dir: &Path, url: &str) -> Result<()> {
    // Remove o remoto antigo antes (se existir) — permite trocar de URL sem
    // falhar com "remote origin already exists".
    let _ = run_git(dir, &["remote", "remove", "origin"]);
    run_git(dir, &["remote", "add", "origin", url])?;
    Ok(())
}

/// Identidade LOCAL (só essa pasta — `--local`, nunca `--global`) usada nos
/// commits de backup. Pode ser fake (o usuário só precisa de algo consistente
/// pra aparecer no histórico, não uma identidade real).
pub fn set_local_identity(dir: &Path, name: &str, email: &str) -> Result<()> {
    run_git(dir, &["config", "--local", "user.name", name])?;
    run_git(dir, &["config", "--local", "user.email", email])?;
    Ok(())
}

pub fn get_local_identity(dir: &Path) -> (Option<String>, Option<String>) {
    let name = run_git(dir, &["config", "--local", "user.name"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = run_git(dir, &["config", "--local", "user.email"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    (name, email)
}

/// Injeta `token@` na URL só pra ESSA chamada (nunca grava no remoto
/// permanente) — só se aplica a URLs `https://`; SSH (`git@host:...`) usa o
/// agente SSH do SO normalmente, token não tem onde entrar aí.
fn authenticated_url(url: &str, token: Option<&str>) -> String {
    match token {
        Some(t) if !t.is_empty() && url.starts_with("https://") => {
            format!("https://{t}@{}", &url["https://".len()..])
        }
        _ => url.to_string(),
    }
}

pub fn get_remote(dir: &Path) -> Option<String> {
    run_git(dir, &["remote", "get-url", "origin"])
        .ok()
        .map(|s| s.trim().to_string())
}

/// Commita qualquer mudança local, faz pull (merge, não rebase — mais
/// tolerante a rodar de várias máquinas) e push. Devolve um resumo em texto
/// do que aconteceu em cada etapa. `token` (opcional) só é usado se o
/// remoto for `https://` — injetado na URL de cada chamada, nunca gravado
/// no remoto permanente.
pub fn sync(dir: &Path, token: Option<&str>) -> Result<String> {
    if !is_repo(dir) {
        return Err(anyhow!(
            "backup ainda nao foi iniciado nesta pasta (sem repositorio git)"
        ));
    }
    let (name, email) = get_local_identity(dir);
    if name.is_none() || email.is_none() {
        return Err(anyhow!(
            "configure nome e email antes de sincronizar (Configuracoes -> Backup de sessoes) - \
             o git precisa disso pra criar o commit, mesmo que seja um valor fake"
        ));
    }

    let mut log = Vec::new();

    run_git(dir, &["add", "-A"])?;
    let porcelain = run_git(dir, &["status", "--porcelain"]).unwrap_or_default();
    if porcelain.trim().is_empty() {
        log.push("nada novo pra commitar localmente.".to_string());
    } else {
        let msg = format!(
            "Backup Cerne Code — {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        run_git(dir, &["commit", "-m", &msg])?;
        log.push("commit local criado.".to_string());
    }

    if let Some(remote_url) = get_remote(dir) {
        let auth_url = authenticated_url(&remote_url, token);
        run_git(
            dir,
            &[
                "pull",
                "--no-rebase",
                "--allow-unrelated-histories",
                &auth_url,
                "HEAD",
            ],
        )
        .map_err(|e| anyhow!("pull falhou (pode ser conflito - resolva manualmente): {e}"))?;
        log.push("pull ok.".to_string());
        run_git(dir, &["push", &auth_url, "HEAD"]).map_err(|e| anyhow!("push falhou: {e}"))?;
        log.push("push ok.".to_string());
    } else {
        log.push("sem remoto configurado - so commit local.".to_string());
    }

    Ok(log.join(" "))
}

/// Lista os arquivos com mudança em relação ao HEAD (staged + unstaged
/// combinados, já que o Cerne escreve direto na working tree, nunca faz
/// `git add`) mais os arquivos novos ainda não rastreados.
pub fn status(project_root: &Path) -> Result<Vec<GitFileChange>> {
    run_git(project_root, &["rev-parse", "--is-inside-work-tree"])
        .map_err(|_| anyhow!("esta pasta nao e um repositorio git (ou o git nao esta instalado)"))?;

    let mut changes: HashMap<String, GitFileChange> = HashMap::new();

    let numstat = run_git(project_root, &["diff", "HEAD", "--numstat", "--relative"]).unwrap_or_default();
    for line in numstat.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(add), Some(del), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        // Arquivo binario: git mostra "-" em vez de um numero.
        let additions = add.parse().unwrap_or(0);
        let deletions = del.parse().unwrap_or(0);
        changes.insert(
            path.to_string(),
            GitFileChange {
                path: path.to_string(),
                status: "modified".to_string(),
                additions,
                deletions,
            },
        );
    }

    let name_status = run_git(project_root, &["diff", "HEAD", "--name-status", "--relative"]).unwrap_or_default();
    for line in name_status.lines() {
        let mut parts = line.splitn(2, '\t');
        let (Some(code), Some(path)) = (parts.next(), parts.next()) else {
            continue;
        };
        let status = match code.chars().next() {
            Some('A') => "added",
            Some('D') => "deleted",
            Some('R') => "renamed",
            _ => "modified",
        };
        if let Some(c) = changes.get_mut(path) {
            c.status = status.to_string();
        }
    }

    let untracked = run_git(project_root, &["status", "--porcelain", "--untracked-files=all"]).unwrap_or_default();
    for line in untracked.lines() {
        if let Some(path) = line.strip_prefix("?? ") {
            let full = project_root.join(path);
            let additions = std::fs::read_to_string(&full)
                .map(|s| s.lines().count() as u32)
                .unwrap_or(0);
            changes.insert(
                path.to_string(),
                GitFileChange {
                    path: path.to_string(),
                    status: "untracked".to_string(),
                    additions,
                    deletions: 0,
                },
            );
        }
    }

    let mut result: Vec<GitFileChange> = changes.into_values().collect();
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

/// Devolve o diff unificado de um arquivo especifico (relativo a
/// `project_root`). Arquivo ainda nao rastreado: `git diff` nao mostra nada
/// pra ele (fora do index), entao sintetiza um diff "tudo adicionado" na
/// mao, no mesmo formato unificado que o resto do app ja sabe colorir
/// (`diffUtils.ts::diffLines`).
pub fn diff_file(project_root: &Path, rel_path: &str) -> Result<String> {
    let is_tracked = run_git(project_root, &["ls-files", "--error-unmatch", "--", rel_path]).is_ok();
    if !is_tracked {
        let full = project_root.join(rel_path);
        let content = std::fs::read_to_string(&full)
            .unwrap_or_else(|_| "(arquivo binario ou ilegivel)".to_string());
        let mut text = format!("--- /dev/null\n+++ b/{rel_path}\n");
        for line in content.lines() {
            text.push('+');
            text.push_str(line);
            text.push('\n');
        }
        return Ok(text);
    }
    run_git(project_root, &["diff", "HEAD", "--relative", "--", rel_path])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    fn scratch_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-git-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        StdCommand::new("git").arg("init").arg("-q").current_dir(&dir).output().unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@cerne.local"])
            .current_dir(&dir)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "cerne-test"])
            .current_dir(&dir)
            .output()
            .unwrap();
        std::fs::write(dir.join("existing.txt"), "linha 1\nlinha 2\n").unwrap();
        StdCommand::new("git").args(["add", "."]).current_dir(&dir).output().unwrap();
        StdCommand::new("git")
            .args(["commit", "-q", "-m", "inicial"])
            .current_dir(&dir)
            .output()
            .unwrap();
        dir
    }

    #[test]
    fn status_errors_outside_a_git_repo() {
        let dir = std::env::temp_dir().join(format!("cerne-not-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(status(&dir).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn status_reports_modified_added_and_untracked_files() {
        let dir = scratch_repo();
        std::fs::write(dir.join("existing.txt"), "linha 1 mudou\nlinha 2\n").unwrap();
        std::fs::write(dir.join("novo.txt"), "arquivo novo\nsegunda linha\n").unwrap();

        let changes = status(&dir).unwrap();
        let modified = changes.iter().find(|c| c.path == "existing.txt").unwrap();
        assert_eq!(modified.status, "modified");
        assert_eq!(modified.additions, 1);
        assert_eq!(modified.deletions, 1);

        let untracked = changes.iter().find(|c| c.path == "novo.txt").unwrap();
        assert_eq!(untracked.status, "untracked");
        assert_eq!(untracked.additions, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn diff_file_shows_unified_diff_for_tracked_file() {
        let dir = scratch_repo();
        std::fs::write(dir.join("existing.txt"), "linha 1 mudou\nlinha 2\n").unwrap();
        let diff = diff_file(&dir, "existing.txt").unwrap();
        assert!(diff.contains("-linha 1"));
        assert!(diff.contains("+linha 1 mudou"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn diff_file_synthesizes_all_additions_for_untracked_file() {
        let dir = scratch_repo();
        std::fs::write(dir.join("novo.txt"), "linha a\nlinha b\n").unwrap();
        let diff = diff_file(&dir, "novo.txt").unwrap();
        assert!(diff.contains("+linha a"));
        assert!(diff.contains("+linha b"));
        assert!(diff.contains("/dev/null"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn init_repo_makes_is_repo_true() {
        let dir = std::env::temp_dir().join(format!("cerne-backup-git-{}", uuid::Uuid::new_v4()));
        assert!(!is_repo(&dir));
        init_repo(&dir).unwrap();
        assert!(is_repo(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_remote_then_get_remote_roundtrips() {
        let dir = std::env::temp_dir().join(format!("cerne-backup-git-{}", uuid::Uuid::new_v4()));
        init_repo(&dir).unwrap();
        assert_eq!(get_remote(&dir), None);
        set_remote(&dir, "https://example.com/repo.git").unwrap();
        assert_eq!(get_remote(&dir), Some("https://example.com/repo.git".to_string()));
        // Troca de URL nao deve falhar com "remote origin already exists".
        set_remote(&dir, "https://example.com/outro.git").unwrap();
        assert_eq!(get_remote(&dir), Some("https://example.com/outro.git".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sync_errors_when_dir_is_not_a_repo_yet() {
        let dir = std::env::temp_dir().join(format!("cerne-backup-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(sync(&dir, None).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sync_errors_without_local_identity_configured() {
        let dir = std::env::temp_dir().join(format!("cerne-backup-git-{}", uuid::Uuid::new_v4()));
        init_repo(&dir).unwrap();
        let err = sync(&dir, None).unwrap_err().to_string();
        assert!(err.contains("nome e email"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_local_identity_roundtrips_and_stays_local_not_global() {
        let dir = std::env::temp_dir().join(format!("cerne-backup-git-{}", uuid::Uuid::new_v4()));
        init_repo(&dir).unwrap();
        assert_eq!(get_local_identity(&dir), (None, None));
        set_local_identity(&dir, "Cerne Code", "backup@cernecode.local").unwrap();
        assert_eq!(
            get_local_identity(&dir),
            (
                Some("Cerne Code".to_string()),
                Some("backup@cernecode.local".to_string())
            )
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn authenticated_url_injects_token_only_for_https() {
        assert_eq!(
            authenticated_url("https://github.com/user/repo.git", Some("tok123")),
            "https://tok123@github.com/user/repo.git"
        );
        assert_eq!(
            authenticated_url("https://github.com/user/repo.git", None),
            "https://github.com/user/repo.git"
        );
        assert_eq!(
            authenticated_url("git@github.com:user/repo.git", Some("tok123")),
            "git@github.com:user/repo.git"
        );
    }

    #[test]
    fn sync_commits_local_changes_without_remote() {
        let dir = std::env::temp_dir().join(format!("cerne-backup-git-{}", uuid::Uuid::new_v4()));
        init_repo(&dir).unwrap();
        StdCommand::new("git").args(["config", "user.email", "test@cerne.local"]).current_dir(&dir).output().unwrap();
        StdCommand::new("git").args(["config", "user.name", "cerne-test"]).current_dir(&dir).output().unwrap();
        std::fs::write(dir.join("session-a.json"), "{}").unwrap();

        let summary = sync(&dir, None).unwrap();
        assert!(summary.contains("commit local criado"));
        assert!(summary.contains("sem remoto"));

        // Rodar de novo sem mudanca nenhuma nao deve falhar, so nao ter nada
        // pra commitar.
        let summary2 = sync(&dir, None).unwrap();
        assert!(summary2.contains("nada novo"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
