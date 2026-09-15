//! Detecção do shell disponível no sistema operacional.
//!
//! Windows: prefere PowerShell 7 (`pwsh`), depois PowerShell 5 (`powershell`),
//! e por último `cmd` como fallback.
//! Linux/macOS: usa `/bin/sh` (ou `$SHELL` se definido).
//!
//! O resultado é cacheado na primeira chamada — o shell não muda durante a
//! execução do app.

use std::sync::OnceLock;

use regex::Regex;

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

/// Verifica se um comando existe no PATH (sem executar). `pub(crate)` desde
/// 2026-08-17 pra dar mensagem clara ao usuário quando falta uma
/// dependência externa opcional (`git`, `uv`, `node`/`npx` de um MCP) em vez
/// de deixar a ferramenta falhar com um erro de shell confuso tipo "comando
/// não encontrado" — achado testando ao vivo, T17: usuário só com o Cerne
/// Code instalado, sem `uv`, teria batido nisso sem entender o motivo.
#[cfg(windows)]
pub(crate) fn command_exists(name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    // `where` é o equivalente Windows do `which`
    std::process::Command::new("where")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub(crate) fn command_exists(name: &str) -> bool {
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

/// Mesma ideia de `apply_creation_flags`, só que pra `std::process::Command`
/// (usado por chamadas síncronas curtas, ex: `git.rs`) em vez de
/// `tokio::process::Command`.
#[cfg(windows)]
pub fn apply_std_creation_flags(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
pub fn apply_std_creation_flags(_cmd: &mut std::process::Command) {
    // No-op em Unix
}

/// Mata um processo e toda a árvore de filhos dele, de forma síncrona e
/// bloqueante — usado em contextos que não são async (ex: handler de saída
/// do Tauri). No Windows, `taskkill /PID <pid> /T /F` mata a árvore inteira;
/// `Child::start_kill()`/`kill_on_drop` sozinhos só matam o processo direto
/// (achado documentado em `agent/background.rs::stop`). Ignora falha de
/// propósito — o caso mais comum é o processo já ter morrido sozinho.
///
/// No Unix, o correto é matar o **process group**: os comandos são spawnados
/// com `.process_group(0)` (ver `apply_process_group`), então o shell E todos
/// os filhos dele compartilham o pgid = pid original, e um único `kill -9 -<pgid>`
/// alcança a árvore inteira. Antes disso era `kill -9 <pid>` puro, que só
/// matava o `/bin/sh -c` e deixava o processo de verdade órfão (mesmo bug do
/// taskkill no Windows). Se o grupo já não existir (processo morreu sozinho),
/// cai pro `kill -9 <pid>` individual por segurança.
pub fn kill_pid_tree_blocking(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
    }
    #[cfg(not(windows))]
    {
        // `kill -- -PGID` mata todo o process group de uma vez. O sinal
        // negativo na frente do PID é o que indica "grupo, não processo".
        let group_kill = std::process::Command::new("kill")
            .args(["-9", &format!("-{pid}")])
            .output();
        if group_kill.is_err() || !group_kill.unwrap().status.success() {
            // Grupo pode nem existir mais (processo morreu sozinho e filhos
            // reparentados pra init). Tenta o processo direto mesmo assim.
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
    }
}

/// Coloca o futuro processo em seu PRÓPRIO process group (`pgid = pid`),
/// pré-condição pro kill de árvore Unix funcionar (ver
/// `kill_pid_tree_blocking`). Sem isso, o shell herda o pgid do Cerne e um
/// hipotético `kill -- -<pgid>` mataria o próprio Cerne junto.
#[cfg(not(windows))]
pub fn apply_process_group(cmd: &mut tokio::process::Command) {
    cmd.process_group(0);
}

#[cfg(windows)]
pub fn apply_process_group(_cmd: &mut tokio::process::Command) {
    // No-op no Windows: lá a árvore é morta via `taskkill /T`, que não
    // depende de process groups.
}

/// Setup que faz o PowerShell escrever a saida em UTF-8.
///
/// **Por que e necessario.** O PowerShell escreve na saida padrao usando o
/// codepage do console (OEM) — no Windows pt-BR isso e **CP850**, nao UTF-8.
/// Medido em 2026-09-15 capturando os bytes crus de `[Console]::Out.Write`:
/// a palavra "rótulos" saia como `72 A2 74 75 6C 6F 73` (o `A2` e o `ó` em
/// CP850), que e UTF-8 invalido — o `String::from_utf8_lossy` do lado Rust
/// entao trocava por U+FFFD e o usuario via `r�tulos` no chat.
///
/// Detalhe que engana: o codepage **OEM** (CP850) nao e o **ANSI** (CP1252),
/// entao nem sequer decodificar como Windows-1252 resolveria — daria `r¢tulos`.
/// Forcar UTF-8 na origem e o caminho correto.
///
/// So entra quando o shell e PowerShell (ver `with_encoding_prologue`). O
/// prefixo fica no comando que roda de verdade, **nao** no texto que a UI
/// mostra em "IN" — esse vem do argumento original (`extract_command_text`).
///
/// ⚠️ A propriedade e `[Console]::OutputEncoding` — **nao**
/// `[Text.Encoding]::OutputEncoding` (essa nao existe; a atribuicao falha e o
/// comando segue com o encoding errado, sem quebrar nada visivelmente). Esse
/// erro passou despercebido ate o teste de ponta a ponta comparar os bytes.
const PS_UTF8_PROLOGUE: &str = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; ";

/// Mesma ideia do `PS_UTF8_PROLOGUE`, pro `cmd.exe` (que so tem `chcp`). O
/// `>nul` esconde a linha "Active code page: 65001" que o `chcp` imprime.
const CMD_UTF8_PROLOGUE: &str = "chcp 65001 >nul && ";

/// Regex das sequencias de escape ANSI/VT100 que os programas coloridos
/// (vite, cargo, npm, git) escrevem na saida quando ela e redirecionada.
///
/// Sem isso o bloco "OUT" do chat mostra o codigo cru — o usuario via
/// `[31m[7merror[0m during build:` em vez de "error during build:" — e o LLM
/// ainda paga token por cada sequencia.
fn ansi_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Ordem importa: o mais especifico primeiro, senao o escape de 1 char
        // casaria o comeco de uma sequencia CSI e deixaria o resto pra tras.
        Regex::new(concat!(
            r"\x1b\[[\x30-\x3f]*[\x20-\x2f]*[\x40-\x7e]", // CSI: cores, mover cursor, limpar
            r"|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)",          // OSC: titulo da janela, hyperlink
            r"|\x1b[()*+#][\x20-\x7e]",                     // selecao de charset (ESC ( B)
            r"|\x1b[\x20-\x2f][\x30-\x7e]",                 // escape de 2 chars (ESC # 8)
            r"|\x1b[\x30-\x7e]",                            // escape de 1 char (ESC =, ESC c)
        ))
        .expect("regex de ANSI e constante e valida")
    })
}

/// Remove sequencias de escape ANSI de um texto.
///
/// Aplicado em **toda** saida de comando antes de guardar (ver `tools.rs`,
/// `background.rs` e `git.rs`) — o chat nao renderiza cor, entao os codigos so
/// atrapalham a leitura e gastam token.
pub fn strip_ansi(text: &str) -> String {
    ansi_regex().replace_all(text, "").into_owned()
}

/// Decodifica bytes de saida de processo pra texto.
///
/// UTF-8 estrito quando der (o caso normal, depois do prologue de encoding);
/// cai pra `from_utf8_lossy` só quando os bytes realmente nao sao UTF-8.
///
/// O fallback **nao** tenta adivinhar o codepage: `encoding_rs` segue o WHATWG
/// Encoding Standard e nao inclui codepages OEM do Windows (CP850/CP437), que
/// sao justamente os que o console usa aqui — chutar Windows-1252 daria
/// `r¢tulos`, que parece texto plausivel e esconde o problema. Um caractere de
/// substituicao visivel e mais honesto: denuncia que algo veio torto.
pub fn decode_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Prepende o setup de codificacao adequado ao shell detectado, pra a saida
/// dele chegar em UTF-8 no lado Rust (ver `PS_UTF8_PROLOGUE`).
fn with_encoding_prologue(command: &str, shell: &ShellInfo) -> String {
    match shell.executable.as_str() {
        "pwsh" | "powershell" => format!("{PS_UTF8_PROLOGUE}{command}"),
        "cmd" => format!("{CMD_UTF8_PROLOGUE}{command}"),
        // Unix ja usa UTF-8 por padrao na pratica.
        _ => command.to_string(),
    }
}

/// Configura um `tokio::process::Command` com o shell detectado e o comando
/// do usuário. Retorna o Command pronto para `.spawn()`.
pub fn build_shell_command(command: &str) -> tokio::process::Command {
    let shell = detect_shell();
    let mut cmd = tokio::process::Command::new(&shell.executable);
    for arg in &shell.args_prefix {
        cmd.arg(arg);
    }
    cmd.arg(with_encoding_prologue(command, shell));
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

    #[test]
    fn strip_ansi_remove_cores_e_controle() {
        // Os casos que o usuário viu no chat (vite/cargo colorindo a saída).
        assert_eq!(
            strip_ansi("\u{1b}[31m\u{1b}[7merror\u{1b}[0m during build:"),
            "error during build:"
        );
        assert_eq!(strip_ansi("normal sem escape"), "normal sem escape");
        assert_eq!(strip_ansi("\u{1b}[0m"), "");
        // Mover cursor / limpar linha (barras de progresso).
        assert_eq!(strip_ansi("50%\u{1b}[2K\r100%"), "50%\r100%");
    }

    #[test]
    fn strip_ansi_remove_osc_e_hyperlink() {
        // OSC com terminador BEL e com ST (ESC \).
        assert_eq!(strip_ansi("\u{1b}]0;titulo\u{07}texto"), "texto");
        assert_eq!(strip_ansi("\u{1b}]0;titulo\u{1b}\\texto"), "texto");
        // Hyperlink OSC 8 (formato que o ls moderno usa).
        assert_eq!(
            strip_ansi("\u{1b}]8;;https://exemplo.com\u{07}clique\u{1b}]8;;\u{07}"),
            "clique"
        );
    }

    #[test]
    fn strip_ansi_nao_come_acento_nem_texto_normal() {
        // Regressão importante: a limpeza não pode tocar no conteúdo real.
        let original = "rótulos: configuração — ação, órgão, três";
        assert_eq!(strip_ansi(original), original);
        // Colchetes soltos (sem ESC) são texto legítimo, não escape.
        assert_eq!(strip_ansi("[INFO] build ok"), "[INFO] build ok");
        assert_eq!(strip_ansi("array[0] = 1"), "array[0] = 1");
    }

    #[test]
    fn decode_output_le_utf8_valido() {
        assert_eq!(decode_output("rótulos".as_bytes()), "rótulos");
        assert_eq!(decode_output(b"plain ascii"), "plain ascii");
    }

    #[test]
    fn decode_output_nao_panica_com_bytes_invalidos() {
        // CP850 não é UTF-8: `A2` é o `ó` no codepage OEM do console. Não tem
        // como recuperar sem saber o codepage, mas não pode panicar nem sumir
        // com o resto do texto.
        let bytes = [b'r', 0xA2, b't', b'u', b'l', b'o', b's'];
        let saida = decode_output(&bytes);
        assert!(saida.starts_with('r'));
        assert!(saida.ends_with("tulos"));
        assert!(saida.contains('\u{FFFD}'), "deveria marcar o byte invalido");
    }

    #[test]
    fn prologue_e_prefixado_so_nos_shells_que_precisam() {
        let pwsh = ShellInfo {
            executable: "pwsh".to_string(),
            args_prefix: vec!["-NoProfile".to_string(), "-Command".to_string()],
            description: "pwsh".to_string(),
        };
        assert!(with_encoding_prologue("echo oi", &pwsh).starts_with(PS_UTF8_PROLOGUE));
        assert!(with_encoding_prologue("echo oi", &pwsh).ends_with("echo oi"));

        let cmd = ShellInfo {
            executable: "cmd".to_string(),
            args_prefix: vec!["/C".to_string()],
            description: "cmd".to_string(),
        };
        assert!(with_encoding_prologue("dir", &cmd).starts_with("chcp 65001"));

        // Unix: comando passa intacto.
        let sh = ShellInfo {
            executable: "/bin/sh".to_string(),
            args_prefix: vec!["-c".to_string()],
            description: "sh".to_string(),
        };
        assert_eq!(with_encoding_prologue("ls -la", &sh), "ls -la");
    }
}
