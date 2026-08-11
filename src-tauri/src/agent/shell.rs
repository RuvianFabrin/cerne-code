//! Detecção do shell disponível no sistema operacional.
//!
//! Windows: prefere PowerShell 7 (`pwsh`), depois PowerShell 5 (`powershell`),
//! e por último `cmd` como fallback.
//! Linux/macOS: usa `/bin/sh` (ou `$SHELL` se definido).
//!
//! O resultado é cacheado na primeira chamada — o shell não muda durante a
//! execução do app.

use std::sync::OnceLock;

/// Qual shell foi detectado no sistema.
#[derive(Debug, Clone)]
pub struct ShellInfo {
    /// Nome do executável (ex: "pwsh", "powershell", "cmd", "/bin/sh").
    pub executable: String,
    /// Argumentos prefixo antes do comando do usuário.
    /// - pwsh/powershell: `["-NoProfile", "-Command"]`
    /// - cmd: `["/C"]`
    /// - sh: `["-c"]`
    pub args_prefix: Vec<String>,
    /// Descrição legível para o system prompt.
    pub description: String,
}

static CACHED_SHELL: OnceLock<ShellInfo> = OnceLock::new();

/// Retorna o shell detectado (cacheado após a primeira chamada).
pub fn detect_shell() -> &'static ShellInfo {
    CACHED_SHELL.get_or_init(|| {
        #[cfg(windows)]
        {
            detect_windows_shell()
        }
        #[cfg(not(windows))]
        {
            detect_unix_shell()
        }
    })
}

#[cfg(windows)]
fn detect_windows_shell() -> ShellInfo {
    // 1. Tenta PowerShell 7 (pwsh)
    if command_exists("pwsh") {
        return ShellInfo {
            executable: "pwsh".to_string(),
            args_prefix: vec!["-NoProfile".to_string(), "-Command".to_string()],
            description: "PowerShell 7 (pwsh)".to_string(),
        };
    }
    // 2. Tenta PowerShell 5 (powershell.exe — sempre presente no Windows 10+)
    if command_exists("powershell") {
        return ShellInfo {
            executable: "powershell".to_string(),
            args_prefix: vec!["-NoProfile".to_string(), "-Command".to_string()],
            description: "PowerShell 5 (powershell)".to_string(),
        };
    }
    // 3. Fallback: cmd
    ShellInfo {
        executable: "cmd".to_string(),
        args_prefix: vec!["/C".to_string()],
        description: "CMD (cmd.exe)".to_string(),
    }
}

#[cfg(not(windows))]
fn detect_unix_shell() -> ShellInfo {
    // Usa $SHELL se definido, senão /bin/sh
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    ShellInfo {
        executable: shell.clone(),
        args_prefix: vec!["-c".to_string()],
        description: format!("Shell ({shell})"),
    }
}

/// Verifica se um comando existe no PATH (sem executar).
#[cfg(windows)]
fn command_exists(name: &str) -> bool {
    // `where` é o equivalente Windows do `which`
    std::process::Command::new("where")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn command_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Aplica as flags de criação de processo apropriadas para o shell detectado.
/// No Windows, adiciona CREATE_NO_WINDOW para suprimir janelas de console.
#[cfg(windows)]
pub fn apply_creation_flags(cmd: &mut tokio::process::Command) {
    // creation_flags já disponível via tokio::process::Command no Windows
    // sem precisar do trait CommandExt explicitamente.
    #[allow(unused_imports)]
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
pub fn apply_creation_flags(_cmd: &mut tokio::process::Command) {
    // No-op em Unix
}

/// Configura um `tokio::process::Command` com o shell detectado e o comando
/// do usuário. Retorna o Command pronto para `.spawn()`.
pub fn build_shell_command(command: &str) -> tokio::process::Command {
    let shell = detect_shell();
    let mut cmd = tokio::process::Command::new(&shell.executable);
    for arg in &shell.args_prefix {
        cmd.arg(arg);
    }
    cmd.arg(command);
    apply_creation_flags(&mut cmd);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_returns_valid_info() {
        let shell = detect_shell();
        assert!(!shell.executable.is_empty());
        assert!(!shell.args_prefix.is_empty());
        assert!(!shell.description.is_empty());
    }

    #[test]
    fn build_shell_command_includes_user_command() {
        let cmd = build_shell_command("echo hello");
        // Não podemos inspecionar os args facilmente, mas pelo menos
        // verifica que não panica.
        let _ = format!("{:?}", cmd);
    }
}
