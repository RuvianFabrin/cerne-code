//! Modo Anything, fase 04: o modo numa sessão. Cada sessão com o modo ligado tem o próprio servidor
//! do motor, apontado para a pasta de trabalho do projeto dela, e um bloco curto no prompt.

use super::cli;
use crate::mcp::McpServerConfig;
use crate::models::{AnythingSettings, AnythingState};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Nome do servidor desta sessão. Precisa ser por sessão: o cliente MCP é guardado pelo nome, e duas
/// sessões com projetos diferentes não podem dividir o mesmo processo.
pub fn server_name(session_id: &str) -> String {
    let curto: String = session_id.chars().filter(|c| c.is_ascii_alphanumeric()).take(12).collect();
    format!("anything-{curto}")
}

/// Pasta onde o modo guarda `.anything`: a primeira pasta da sessão com permissão de escrita. Todas as
/// pastas da sessão têm o mesmo peso (não há pasta "principal"); o modo precisa gravar fichas, então
/// escolhe a primeira que aceita escrita. `project_root` só entra como reserva, para sessões antigas
/// que ainda não têm pastas anexadas.
pub fn raiz_de_trabalho(session: &crate::models::Session) -> Option<String> {
    escolher_raiz(&session.extra_read_paths, session.project_root.as_deref())
}

fn escolher_raiz(pastas: &[crate::models::FolderEntry], project_root: Option<&str>) -> Option<String> {
    pastas
        .iter()
        .find(|f| f.mode == crate::models::FolderMode::ReadWrite)
        .map(|f| f.path.clone())
        .or_else(|| if pastas.is_empty() { project_root.map(String::from) } else { None })
}

/// Mensagem de quando não há onde gravar.
pub const SEM_PASTA: &str = "adicione à sessão uma pasta com permissão de escrita antes de usar o modo Anything";

/// Pasta de trabalho do modo dentro do projeto (`.anything`). A fase 06 completa o layout.
pub fn workspace_dir(project_root: &Path) -> PathBuf {
    project_root.join(".anything")
}

/// Projeto `.cany` do motor, dentro da pasta de trabalho. O motor cria se faltar.
pub fn project_dir(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("investigacao.cany")
}

/// Configuração do servidor desta sessão, com as opções efetivas: Deep ou clean room, `--allow-run`
/// só se ligado, `--all-tools` só se ligado. Cada opção vira um flag do próprio motor.
pub fn preset(exe: &Path, project_root: &Path, session_id: &str, opts: &AnythingSettings) -> McpServerConfig {
    let mut args: Vec<String> = vec![if opts.deep { "--deep".into() } else { "--clean-room".into() }];
    if opts.allow_run {
        args.push("--allow-run".into());
    }
    args.extend(["--project".into(), project_dir(project_root).display().to_string(), "mcp".into()]);
    if opts.full_tools {
        args.push("--all-tools".into());
    }
    McpServerConfig {
        name: server_name(session_id),
        command: exe.display().to_string(),
        args,
        env: HashMap::new(),
        url: None,
        bearer_token: None,
        enabled: true,
    }
}

/// Servidor desta sessão, se o modo está ligado, a sessão tem pasta de projeto e o motor está instalado.
/// `None` em qualquer outro caso: o turno segue sem as ferramentas do motor.
pub fn server_for(
    app_data_dir: &Path,
    global: &AnythingSettings,
    state: &AnythingState,
    project_root: Option<&str>,
    session_id: &str,
) -> Option<McpServerConfig> {
    if !state.enabled {
        return None;
    }
    let root = PathBuf::from(project_root?);
    let (exe, _) = cli::find_cli(&cli::store_dir(app_data_dir))?;
    Some(preset(&exe, &root, session_id, &state.effective(global)))
}

/// Bloco do prompt de sistema quando o modo está ligado. Texto provisório: a fase 09 põe o definitivo.
pub fn prompt_block(state: &AnythingState, session_id: &str) -> String {
    let alvo = state.target.as_deref().unwrap_or("(nenhum alvo escolhido)");
    let objetivo = state.goal.as_deref().unwrap_or("(nenhum objetivo escrito)");
    format!(
        "## Modo Anything\n\n\
         Está ligado. O alvo a estudar é: {alvo}. O objetivo da pessoa: {objetivo}.\n\
         As ferramentas do motor vêm do servidor `{}` (nomes `mcp__{}__…`). Anote o que você achar \
         na pasta `.anything` do projeto. Não execute o programa analisado sem a pessoa pedir.",
        server_name(session_id),
        server_name(session_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_session_gets_its_own_server_name() {
        assert_ne!(server_name("aaa-111"), server_name("bbb-222"));
        assert_eq!(server_name("a-b-c"), "anything-abc");
    }

    #[test]
    fn default_options_are_deep_essentials_and_no_run() {
        let p = preset(Path::new("C:/store/cli.exe"), Path::new("C:/projetos/x"), "s1", &AnythingSettings::default());
        assert_eq!(p.name, "anything-s1");
        assert!(p.args.contains(&"--deep".to_string()));
        assert!(!p.args.contains(&"--all-tools".to_string()), "padrão: só o essencial");
        assert!(!p.args.iter().any(|a| a == "--allow-run"));
        let i = p.args.iter().position(|a| a == "--project").unwrap();
        assert!(p.args[i + 1].ends_with("investigacao.cany"));
    }

    #[test]
    fn each_option_becomes_its_engine_flag() {
        let tudo = AnythingSettings { full_tools: true, deep: false, allow_run: true };
        let p = preset(Path::new("C:/cli.exe"), Path::new("C:/p"), "s", &tudo);
        assert!(p.args.contains(&"--clean-room".to_string()) && !p.args.contains(&"--deep".to_string()));
        assert!(p.args.contains(&"--allow-run".to_string()));
        assert!(p.args.contains(&"--all-tools".to_string()));
        let mcp = p.args.iter().position(|a| a == "mcp").unwrap();
        assert!(p.args.iter().position(|a| a == "--all-tools").unwrap() > mcp, "--all-tools é do subcomando mcp");
    }

    #[test]
    fn session_overrides_win_over_the_global_default() {
        let global = AnythingSettings { full_tools: false, deep: true, allow_run: false };
        let sem = AnythingState { enabled: true, ..Default::default() };
        assert_eq!(sem.effective(&global), global, "sem sobrescrita, vale o global");
        let com = AnythingState { full_tools: Some(true), deep: Some(false), ..sem.clone() };
        let e = com.effective(&global);
        assert!(e.full_tools && !e.deep && !e.allow_run);
    }

    #[test]
    fn no_server_when_the_mode_is_off_or_there_is_no_project() {
        let g = AnythingSettings::default();
        let off = AnythingState { enabled: false, ..Default::default() };
        let on = AnythingState { enabled: true, ..Default::default() };
        assert!(server_for(Path::new("C:/nao-existe"), &g, &off, Some("C:/p"), "s").is_none());
        assert!(server_for(Path::new("C:/nao-existe"), &g, &on, None, "s").is_none(), "sem pasta de projeto não liga");
    }

    #[test]
    fn prompt_names_the_target_goal_and_server() {
        let st = AnythingState { enabled: true, target: Some("app.exe".into()), goal: Some("como salva".into()), ..Default::default() };
        let b = prompt_block(&st, "sess1");
        assert!(b.contains("app.exe") && b.contains("como salva") && b.contains("mcp__anything-sess1__"));
    }
}

#[cfg(test)]
mod raiz_tests {
    use super::*;
    use crate::models::{FolderEntry, FolderMode};

    fn pasta(p: &str, mode: FolderMode) -> FolderEntry {
        FolderEntry { path: p.into(), mode }
    }

    #[test]
    fn the_first_writable_folder_is_the_workspace_whatever_its_position() {
        let pastas = vec![pasta("F:/so-leitura", FolderMode::Read), pasta("F:/aaaTeste", FolderMode::ReadWrite), pasta("F:/outra", FolderMode::ReadWrite)];
        assert_eq!(escolher_raiz(&pastas, Some("C:/antigo")).as_deref(), Some("F:/aaaTeste"));
    }

    #[test]
    fn only_read_folders_means_nowhere_to_write_and_the_old_root_is_not_a_fallback() {
        let pastas = vec![pasta("F:/so-leitura", FolderMode::Read)];
        assert_eq!(escolher_raiz(&pastas, Some("C:/antigo")), None);
    }

    #[test]
    fn old_sessions_without_attached_folders_still_use_project_root() {
        assert_eq!(escolher_raiz(&[], Some("C:/antigo")).as_deref(), Some("C:/antigo"));
        assert_eq!(escolher_raiz(&[], None), None);
    }
}
