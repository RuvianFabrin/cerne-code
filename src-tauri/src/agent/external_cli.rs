//! Ponte com CLIs de agente externos (Claude Code, Codex, Gemini CLI, Qwen
//! Code) tratados como um "provider" a mais — porta o padrão validado pelo
//! projeto `maestro` (catálogo declarativo + spawn de subprocesso), adaptado
//! pro Rust do Cerne.
//!
//! **Diferença de arquitetura importante**: um provider HTTP normal
//! (llama.cpp/OpenRouter/Custom) fala com o loop de ferramentas do Cerne —
//! o modelo chama `read_file`/`write_file`/etc e o Cerne executa. Um CLI
//! externo tem o **próprio** loop de ferramentas (não vê as ferramentas do
//! Cerne, não recebe o system prompt do Cerne) — é uma caixa-preta: manda um
//! prompt, espera terminar, pega a resposta final. Por isso `run_turn`
//! (`agent/mod.rs`) trata `ProviderKind::Cli` num caminho totalmente
//! separado do loop agentico normal, sem `tool_specs` nem `system_prompt`.
//!
//! **Modo de acesso**: espelha o `ExecutionMode` da sessão — `Yolo` libera o
//! CLI pra editar arquivos e rodar comandos sozinho (`--dangerously-skip-permissions`
//! no Claude, `--sandbox danger-full-access` no Codex, `--yolo` no Gemini/Qwen);
//! qualquer outro modo roda em plano/leitura (o CLI só analisa e responde,
//! nunca muda nada sozinho) — pedido explícito do usuário (2026-09-21).

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use super::shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliBackendId {
    Claude,
    Codex,
    Gemini,
    Qwen,
    Pi,
}

impl CliBackendId {
    pub fn all() -> [CliBackendId; 5] {
        [
            CliBackendId::Claude,
            CliBackendId::Codex,
            CliBackendId::Gemini,
            CliBackendId::Qwen,
            CliBackendId::Pi,
        ]
    }

    pub fn id(self) -> &'static str {
        match self {
            CliBackendId::Claude => "claude",
            CliBackendId::Codex => "codex",
            CliBackendId::Gemini => "gemini",
            CliBackendId::Qwen => "qwen",
            CliBackendId::Pi => "pi",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CliBackendId::Claude => "Claude Code CLI",
            CliBackendId::Codex => "Codex CLI",
            CliBackendId::Gemini => "Gemini CLI",
            CliBackendId::Qwen => "Qwen Code CLI",
            CliBackendId::Pi => "Pi CLI",
        }
    }

    pub fn from_id(id: &str) -> Option<CliBackendId> {
        CliBackendId::all().into_iter().find(|b| b.id() == id)
    }

    /// Nome do binário procurado no PATH (sobrescrevível via
    /// `ExternalCliConfig.bin_overrides`).
    pub fn bin_default(self) -> &'static str {
        match self {
            CliBackendId::Claude => "claude",
            CliBackendId::Codex => "codex",
            CliBackendId::Gemini => "gemini",
            CliBackendId::Qwen => "qwen",
            CliBackendId::Pi => "pi",
        }
    }

    /// Modelos conhecidos. Lista curada (igual ao `maestro/frontier/catalog.cjs`)
    /// em vez de perguntar dinamicamente pro CLI — os CLIs interativos não têm
    /// um jeito estável e scriptável de listar modelos, e `/model` é um comando
    /// de TUI, não de uma execução não-interativa. `bin_overrides`/args extras
    /// em Configurações cobrem o caso de um flag ter mudado numa versão nova.
    pub fn known_models(self) -> &'static [&'static str] {
        match self {
            CliBackendId::Claude => &[
                "claude-opus-5",
                "claude-sonnet-5",
                "claude-fable-5-1",
                "claude-haiku-4-5",
            ],
            CliBackendId::Codex => &["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"],
            CliBackendId::Gemini => &["gemini-3.1-pro-preview", "gemini-3-flash"],
            // Qwen Code CLI é um fork do Gemini CLI — chute educado a partir
            // dos flags dele; o usuário pode corrigir via override se o nome
            // do modelo mudar numa versão nova.
            CliBackendId::Qwen => &["qwen3-coder-plus", "qwen3-coder-flash"],
            CliBackendId::Pi => &[],
        }
    }

    fn prompt_via(self) -> PromptVia {
        match self {
            CliBackendId::Claude | CliBackendId::Codex | CliBackendId::Pi => PromptVia::Stdin,
            CliBackendId::Gemini | CliBackendId::Qwen => PromptVia::Arg,
        }
    }

    fn output_parse(self) -> OutputParse {
        match self {
            CliBackendId::Claude => OutputParse::ClaudeJson,
            CliBackendId::Codex => OutputParse::LastMessageFile,
            CliBackendId::Pi => OutputParse::PlainText,
            CliBackendId::Gemini | CliBackendId::Qwen => OutputParse::GeminiJson,
        }
    }

    /// Monta os argumentos de linha de comando (sem o binário) pra uma
    /// chamada não-interativa. `full_access` vem de `session.execution_mode
    /// == Yolo`; qualquer outro modo roda em plano/leitura.
    fn build_args(self, model: Option<&str>, full_access: bool) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        match self {
            CliBackendId::Claude => {
                args.push("-p".into());
                args.push("--output-format".into());
                args.push("json".into());
                if full_access {
                    args.push("--dangerously-skip-permissions".into());
                } else {
                    args.push("--permission-mode".into());
                    args.push("plan".into());
                }
                if let Some(m) = model {
                    args.push("--model".into());
                    args.push(m.into());
                }
            }
            CliBackendId::Codex => {
                args.push("--ask-for-approval".into());
                args.push("never".into());
                args.push("exec".into());
                args.push("--skip-git-repo-check".into());
                args.push("--sandbox".into());
                args.push(if full_access { "danger-full-access" } else { "read-only" }.into());
                if let Some(m) = model {
                    args.push("-m".into());
                    args.push(m.into());
                }
                args.push("--color".into());
                args.push("never".into());
                args.push("--json".into());
            }
            CliBackendId::Gemini => {
                args.push("--output-format".into());
                args.push("json".into());
                args.push("--approval-mode".into());
                args.push(if full_access { "yolo" } else { "plan" }.into());
                if let Some(m) = model {
                    args.push("--model".into());
                    args.push(m.into());
                }
            }
            CliBackendId::Pi => {
                // Agente do CLI: a sessão é guardada pelo próprio Pi (`--session-id`).
                args.push("--print".into());
                if !full_access { args.extend(["--tools".into(), "read,grep,find,ls".into()]); }
                if let Some(m) = model { args.extend(["--model".into(), m.into()]); }
            }
            CliBackendId::Qwen => {
                args.push("--output-format".into());
                args.push("json".into());
                if full_access {
                    args.push("--yolo".into());
                }
                if let Some(m) = model {
                    args.push("--model".into());
                    args.push(m.into());
                }
            }
        }
        args
    }
}

#[derive(Debug, Clone, Copy)]
enum PromptVia {
    Stdin,
    Arg,
}

#[derive(Debug, Clone, Copy)]
enum OutputParse {
    PlainText,
    /// `claude -p --output-format json`: `{"result": "...", "is_error": bool}`.
    ClaudeJson,
    /// `gemini/qwen --output-format json`: `{"response": "..."}`.
    GeminiJson,
    /// Codex não garante JSON limpo no stdout em modo `exec`; lê o arquivo de
    /// `--output-last-message <tmp>`, com fallback pro stdout cru.
    LastMessageFile,
}

/// Overrides por backend, persistidos em `AppConfig.external_cli` — cobre o
/// caso de o binário não estar no PATH com o nome default, ou de um flag ter
/// mudado numa versão nova do CLI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliBackendOverride {
    #[serde(default)]
    pub bin_path: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalCliConfig {
    #[serde(default)]
    pub claude: CliBackendOverride,
    #[serde(default)]
    pub codex: CliBackendOverride,
    #[serde(default)]
    pub gemini: CliBackendOverride,
    #[serde(default)]
    pub qwen: CliBackendOverride,
    #[serde(default)]
    pub pi: CliBackendOverride,
}

impl ExternalCliConfig {
    fn override_for(&self, backend: CliBackendId) -> &CliBackendOverride {
        match backend {
            CliBackendId::Claude => &self.claude,
            CliBackendId::Codex => &self.codex,
            CliBackendId::Gemini => &self.gemini,
            CliBackendId::Qwen => &self.qwen,
            CliBackendId::Pi => &self.pi,
        }
    }
}

/// Explicit paths take precedence. Discovery also checks standard installation
/// directories because a GUI process can have an older PATH than the terminal.
pub(super) fn resolve_bin(backend: CliBackendId, cfg: &ExternalCliConfig) -> String {
    cfg.override_for(backend).bin_path.clone().filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| discover_bin(backend).unwrap_or_else(|| backend.bin_default().into()))
}

fn find_in_dirs(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    #[cfg(windows)]
    let names = [format!("{name}.exe"), format!("{name}.ps1"), format!("{name}.cmd"), format!("{name}.bat")];
    #[cfg(not(windows))]
    let names = [name.to_owned()];
    dirs.iter().flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|path| {
            if !path.is_file() { return false; }
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                return path.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
            }
            #[cfg(not(unix))] { true }
        })
}

// Only enumerate one installation directory level; never search project files.
fn append_install_versions(dirs: &mut Vec<PathBuf>, root: PathBuf) {
    let mut entries: Vec<_> = std::fs::read_dir(root).into_iter().flatten()
        .filter_map(Result::ok).filter(|e| e.path().is_dir()).collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
    dirs.extend(entries.into_iter().map(|e| e.path()));
}

fn discover_bin(backend: CliBackendId) -> Option<String> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).filter(|p| !p.as_os_str().is_empty()).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        candidates.extend([home.join(".local/bin"), home.join(".cargo/bin")]);
    }
    #[cfg(windows)] {
        if let Some(appdata) = std::env::var_os("APPDATA") { candidates.push(PathBuf::from(appdata).join("npm")); }
        if let Some(nvm) = std::env::var_os("NVM_SYMLINK") { candidates.push(PathBuf::from(nvm)); }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            if backend == CliBackendId::Codex { append_install_versions(&mut candidates, local.join("OpenAI/Codex/bin")); }
            append_install_versions(&mut candidates, local.join("nvm"));
        }
        if let Some(nvm) = std::env::var_os("NVM_HOME") { append_install_versions(&mut candidates, PathBuf::from(nvm)); }
    }
    #[cfg(not(windows))] candidates.extend([PathBuf::from("/usr/local/bin"), PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/bin")]);
    find_in_dirs(backend.bin_default(), &candidates).map(|p| p.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Serialize)]
pub struct CliReadiness {
    pub backend: &'static str,
    pub label: &'static str,
    pub installed: bool,
    pub bin: String,
    pub models: Vec<String>,
}

/// Checagem "está instalado?" pra Configurações — reusa `shell::command_exists`
/// (já lida com `where`/`which` cross-platform), exceto quando há override de
/// caminho absoluto, aí confere o arquivo direto.
pub fn check_readiness(cfg: &ExternalCliConfig) -> Vec<CliReadiness> {
    CliBackendId::all()
        .into_iter()
        .map(|backend| {
            let bin = resolve_bin(backend, cfg);
            let installed = if PathBuf::from(&bin).is_absolute() {
                PathBuf::from(&bin).is_file()
            } else {
                shell::command_exists(&bin)
            };
            CliReadiness {
                backend: backend.id(),
                label: backend.label(),
                installed,
                bin,
                models: models_for(backend),
            }
        })
        .collect()
}

/// Use the installed CLI catalog when available, instead of stale model names.
fn models_for(backend: CliBackendId) -> Vec<String> {
    if backend == CliBackendId::Codex {
        let home = std::env::var_os("CODEX_HOME").map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|p| p.join(".codex")));
        if let Some(cache) = home.and_then(|p| std::fs::read_to_string(p.join("models_cache.json")).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) {
            let models: Vec<String> = cache["models"].as_array().into_iter().flatten()
                .filter_map(|m| m["slug"].as_str().map(str::to_owned)).collect();
            if !models.is_empty() { return models; }
        }
    }
    backend.known_models().iter().map(|s| s.to_string()).collect()
}

pub async fn list_models(backend: CliBackendId, cfg: &ExternalCliConfig) -> Result<Vec<String>> {
    if backend != CliBackendId::Pi { return Ok(models_for(backend)); }
    let bin=resolve_bin(backend,cfg);
    let mut command=command_for_bin(&bin)?;
    command.args(["--offline","--no-extensions","--no-skills","--no-context-files","--no-session","--list-models"])
        .env("NO_COLOR","1").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    shell::apply_creation_flags(&mut command);
    shell::apply_process_group(&mut command);
    let child=command.spawn()?;
    let mut tree=super::cli_inference::ProcessTree(child.id());
    let output=tokio::time::timeout(Duration::from_secs(20),child.wait_with_output()).await
        .map_err(|_|anyhow!("Pi demorou para listar os modelos; voce pode informar o identificador manualmente"))??;
    tree.0=None;
    if !output.status.success() { return Err(anyhow!("Pi nao conseguiu listar modelos: {}",String::from_utf8_lossy(&output.stderr).chars().take(500).collect::<String>())); }
    Ok(parse_pi_models(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_pi_models(text: &str) -> Vec<String> {
    text.lines().filter_map(|line| {
        let cols:Vec<_>=line.split_whitespace().collect();
        if cols.len()==6 && matches!(cols[4],"yes"|"no") && matches!(cols[5],"yes"|"no") {
            Some(format!("{}/{}",cols[0],cols[1]))
        } else { None }
    }).collect()
}

pub fn codex_context_length(model: &str) -> Option<u32> {
    let home = std::env::var_os("CODEX_HOME").map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".codex")))?;
    let cache: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(home.join("models_cache.json")).ok()?).ok()?;
    cache["models"].as_array()?.iter().find(|m| m["slug"].as_str() == Some(model))?
        ["context_window"].as_u64().and_then(|n| u32::try_from(n).ok())
}

/// npm Windows launchers need their script host; never build a shell command.
pub(super) fn command_for_bin(bin: &str) -> Result<tokio::process::Command> {
    #[cfg(windows)]
    {
        let path = PathBuf::from(bin);
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ps1")) {
            let mut cmd = tokio::process::Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", bin]);
            return Ok(cmd);
        }
        if bin == "pi" {
            if let Ok(output) = std::process::Command::new("where.exe").arg("pi.ps1").output() {
                if let Some(path) = String::from_utf8_lossy(&output.stdout).lines().find(|p| PathBuf::from(p).is_file()) {
                    return command_for_bin(path);
                }
            }
        }
    }
    Ok(tokio::process::Command::new(bin))
}

const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Dispara uma chamada não-interativa pro CLI externo e devolve o texto da
/// resposta final. Nunca faz streaming de verdade (os CLIs não garantem um
/// formato incremental limpo em modo não-interativo) — o turno inteiro do
/// CLI roda, e o texto completo chega de uma vez quando o processo termina.
/// Resposta de um turno no modo Agente: o texto final e, quando o CLI expõe,
/// o id da sessão dele (para o próximo turno retomar) e o uso de tokens.
pub struct AgentReply {
    pub content: String,
    pub cli_session_id: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

pub async fn dispatch(
    backend: CliBackendId,
    prompt: &str,
    model: Option<&str>,
    full_access: bool,
    cfg: &ExternalCliConfig,
    cwd: Option<&std::path::Path>,
    attachments: &[crate::models::ChatMessage],
    resume: Option<&str>,
) -> Result<AgentReply> {
    let bin = resolve_bin(backend, cfg);
    // O CLI guarda histórico e contexto da sessão: retomamos o id dele em vez
    // de reenviar a conversa. Outros CLIs ainda abrem uma sessão por turno.
    // Pi aceita o id no primeiro turno (`--session-id`), então o Cerne gera o
    // id antes de chamar. Os demais CLIs devolvem o id na própria saída.
    let cli_id: Option<String> = match (backend, resume) {
        (_, Some(id)) => Some(id.to_string()),
        (CliBackendId::Pi, None) => Some(uuid::Uuid::new_v4().to_string()),
        _ => None,
    };
    let mut args = match (backend, resume) {
        (CliBackendId::Codex, Some(id)) => codex_resume_args(model, full_access, id),
        _ => backend.build_args(model, full_access),
    };
    args.extend(cfg.override_for(backend).extra_args.iter().cloned());
    if let Some(id) = &cli_id {
        match backend {
            CliBackendId::Claude | CliBackendId::Gemini | CliBackendId::Qwen if resume.is_some() => {
                args.extend(["--resume".into(), id.clone()]);
            }
            CliBackendId::Pi => args.extend(["--session-id".into(), id.clone()]),
            _ => {}
        }
    }
    let has_images = attachments.iter().any(|m| !m.images.is_empty());
    let mut input_payload = prompt.to_owned();
    let mut image_workspace = None;
    let tmp_path = if matches!(backend.output_parse(), OutputParse::LastMessageFile) {
        let p = std::env::temp_dir().join(format!(
            "cerne-cli-{}-{}.txt",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        args.push("--output-last-message".into());
        args.push(p.to_string_lossy().into_owned());
        Some(p)
    } else {
        None
    };

    if has_images {
        match backend {
            CliBackendId::Claude => {
                let output = args.iter().position(|s| s == "--output-format").unwrap();
                args[output + 1] = "stream-json".into();
                args.extend(["--input-format".into(), "stream-json".into(), "--verbose".into()]);
                input_payload = super::cli_inference::claude_input(prompt, attachments)?;
            }
            CliBackendId::Codex => {
                let workspace = super::cli_inference::Workspace::new()?;
                // The helper keeps the stdin marker last.
                args.push("-".into());
                super::cli_inference::append_codex_images(&workspace.0, &mut args, attachments)?;
                image_workspace = Some(workspace);
            }
            _ => return Err(anyhow!("Imagens anexadas estao disponiveis nos adaptadores Claude e Codex")),
        }
    }

    // `exec resume` sem prompt não lê stdin de forma confiável: marcador `-` explícito.
    if matches!(backend, CliBackendId::Codex) && resume.is_some() && !has_images {
        args.push("-".into());
    }

    if matches!(backend.prompt_via(), PromptVia::Arg) {
        args.push("-p".into());
        args.push(prompt.to_string());
    }

    let mut command = command_for_bin(&bin)?;
    if let Some(cwd) = cwd { command.current_dir(cwd); }
    command
        .args(&args)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    shell::apply_creation_flags(&mut command);
    shell::apply_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| anyhow!("nao foi possivel iniciar '{bin}' ({backend_label}): {e} — confira em Configuracoes se o CLI esta instalado e no PATH", backend_label = backend.label()))?;

    let mut tree = super::cli_inference::ProcessTree(child.id());
    let stdin = child.stdin.take();
    let write = async {
        use tokio::io::AsyncWriteExt;
        if matches!(backend.prompt_via(), PromptVia::Stdin) {
            if let Some(mut stdin) = stdin {
                match stdin.write_all(input_payload.as_bytes()).await {
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {},
                    result => { result?; }
                }
                let _ = stdin.shutdown().await;
            }
        }
        Ok::<_, std::io::Error>(())
    };
    let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
    let (_, output) = tokio::time::timeout(timeout, async {
        tokio::try_join!(write, child.wait_with_output())
    }).await
        .map_err(|_| anyhow!("{} nao respondeu em {}s (timeout)", backend.label(), DEFAULT_TIMEOUT_SECS))?
        .map_err(|e| anyhow!("erro esperando '{bin}' terminar: {e}"))?;
    tree.0 = None;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    drop(image_workspace);
    let mut cli_session_id = None;
    let (mut prompt_tokens, mut completion_tokens) = (0u32, 0u32);
    let content = match backend.output_parse() {
        OutputParse::PlainText => {
            cli_session_id = cli_id.clone();
            stdout.trim().to_string()
        }
        OutputParse::ClaudeJson => {
            let envelope = if has_images {
                super::cli_inference::claude_envelope(&stdout)?
            } else {
                serde_json::from_str::<serde_json::Value>(&stdout)
                    .map_err(|e| anyhow!("resposta do Claude CLI nao e JSON valido: {e}"))?
            };
            cli_session_id = envelope["session_id"].as_str().map(str::to_owned);
            prompt_tokens = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                .iter()
                .map(|k| envelope["usage"][k].as_u64().unwrap_or(0))
                .sum::<u64>()
                .min(u32::MAX as u64) as u32;
            completion_tokens = envelope["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
            parse_claude_json(&envelope.to_string())?
        },
        OutputParse::GeminiJson => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&stdout) {
                cli_session_id = find_session_id(&value).or(cli_session_id);
            }
            parse_gemini_json(&stdout)?
        }
        OutputParse::LastMessageFile => {
            if matches!(backend, CliBackendId::Codex) {
                let (id, input, output) = codex_jsonl_meta(&stdout)?;
                cli_session_id = id;
                prompt_tokens = input;
                completion_tokens = output;
            }
            let from_file = tmp_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .unwrap_or_default();
            let text = if from_file.trim().is_empty() { stdout.clone() } else { from_file };
            text.trim().to_string()
        }
    };

    if let Some(p) = &tmp_path {
        let _ = std::fs::remove_file(p);
    }

    if content.trim().is_empty() {
        let reason = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("saida vazia (exit code {:?})", output.status.code())
        };
        return Err(anyhow!("{} nao devolveu resposta: {reason}", backend.label()));
    }

    Ok(AgentReply { content, cli_session_id, prompt_tokens, completion_tokens })
}

/// Argumentos de `codex exec resume <id>`. Esse subcomando não aceita
/// `--sandbox`, então a política vai por `-c sandbox_mode`.
fn codex_resume_args(model: Option<&str>, full_access: bool, id: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "--ask-for-approval",
        "never",
        "exec",
        "resume",
        "--skip-git-repo-check",
        "--json",
        "-c",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(format!(
        "sandbox_mode=\"{}\"",
        if full_access { "danger-full-access" } else { "read-only" }
    ));
    if let Some(m) = model {
        args.extend(["-m".into(), m.into()]);
    }
    args.push(id.to_string());
    args
}

/// Lê o id da thread (`thread.started`) e o uso (`turn.completed`) do JSONL
/// do `codex exec --json`. Falha explícita quando o turno falha.
fn codex_jsonl_meta(stdout: &str) -> Result<(Option<String>, u32, u32)> {
    let (mut id, mut input, mut output) = (None, 0u32, 0u32);
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        match event["type"].as_str() {
            Some("thread.started") => id = event["thread_id"].as_str().map(str::to_owned),
            Some("turn.completed") => {
                input = event["usage"]["input_tokens"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32;
                output = event["usage"]["output_tokens"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32;
            }
            Some("turn.failed") => return Err(anyhow!(
                "Codex nao concluiu a inferencia: {}",
                event["error"]["message"].as_str().unwrap_or("erro desconhecido")
            )),
            _ => {}
        }
    }
    Ok((id, input, output))
}

/// Erro que indica que o CLI não conhece mais a sessão guardada (apagada ou
/// expirada): o próximo turno deve abrir uma sessão nova.
pub fn is_missing_session_error(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("no conversation found")
        || m.contains("no session found")
        || m.contains("session not found")
        || m.contains("thread not found")
}

fn parse_claude_json(stdout: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(stdout).map_err(|e| anyhow!("resposta do Claude CLI nao e JSON valido: {e}"))?;
    if value.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
        let msg = value
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("erro desconhecido");
        return Err(anyhow!("Claude CLI retornou erro: {msg}"));
    }
    Ok(value
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string())
}

fn parse_gemini_json(stdout: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(stdout).map_err(|e| anyhow!("resposta do CLI nao e JSON valido: {e}"))?;
    // Qwen pode devolver um array de eventos; o resultado fica no evento `result`.
    let text = match &value {
        serde_json::Value::Array(items) => items
            .iter()
            .rev()
            .find(|v| v["type"] == "result")
            .and_then(|v| v["result"].as_str()),
        _ => value.get("response").and_then(|v| v.as_str()),
    };
    Ok(text.unwrap_or_default().trim().to_string())
}

/// Procura o `session_id` na saída JSON (objeto ou array de eventos), que é
/// como Gemini e Qwen informam a sessão para `--resume`.
fn find_session_id(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => map
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| map.values().find_map(find_session_id)),
        serde_json::Value::Array(items) => items.iter().find_map(find_session_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_args_read_only_usa_permission_mode_plan() {
        let args = CliBackendId::Claude.build_args(Some("claude-sonnet-5"), false);
        assert!(args.contains(&"--permission-mode".to_string()));
        assert!(args.contains(&"plan".to_string()));
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(args.contains(&"claude-sonnet-5".to_string()));
    }

    #[test]
    fn claude_args_full_access_usa_dangerously_skip_permissions() {
        let args = CliBackendId::Claude.build_args(None, true);
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!args.contains(&"--permission-mode".to_string()));
    }

    #[test]
    fn codex_args_read_only_usa_sandbox_read_only() {
        let args = CliBackendId::Codex.build_args(Some("gpt-5.5"), false);
        assert!(args.contains(&"read-only".to_string()));
        assert!(!args.contains(&"danger-full-access".to_string()));
        assert!(args.contains(&"never".to_string()), "ask-for-approval sempre never (o Cerne decide via sandbox, nao via prompt interativo)");
    }

    #[test]
    fn codex_args_full_access_usa_danger_full_access() {
        let args = CliBackendId::Codex.build_args(None, true);
        assert!(args.contains(&"danger-full-access".to_string()));
    }

    #[test]
    fn gemini_args_read_only_usa_approval_mode_plan() {
        let args = CliBackendId::Gemini.build_args(None, false);
        assert!(args.contains(&"plan".to_string()));
    }

    #[test]
    fn gemini_args_full_access_usa_yolo() {
        let args = CliBackendId::Gemini.build_args(None, true);
        assert!(args.contains(&"yolo".to_string()));
    }

    #[test]
    fn qwen_args_read_only_nao_inclui_yolo() {
        let args = CliBackendId::Qwen.build_args(None, false);
        assert!(!args.contains(&"--yolo".to_string()));
    }

    #[test]
    fn qwen_args_full_access_inclui_yolo() {
        let args = CliBackendId::Qwen.build_args(None, true);
        assert!(args.contains(&"--yolo".to_string()));
    }

    #[test]
    fn from_id_reconhece_os_quatro_backends() {
        assert_eq!(CliBackendId::from_id("claude"), Some(CliBackendId::Claude));
        assert_eq!(CliBackendId::from_id("codex"), Some(CliBackendId::Codex));
        assert_eq!(CliBackendId::from_id("gemini"), Some(CliBackendId::Gemini));
        assert_eq!(CliBackendId::from_id("qwen"), Some(CliBackendId::Qwen));
        assert_eq!(CliBackendId::from_id("nao-existe"), None);
    }

    #[test]
    fn resolve_bin_usa_default_sem_override() {
        let cfg = ExternalCliConfig::default();
        assert_eq!(resolve_bin(CliBackendId::Claude, &cfg), discover_bin(CliBackendId::Claude).unwrap_or_else(|| "claude".into()));
    }

    #[test]
    fn resolve_bin_usa_override_quando_presente() {
        let mut cfg = ExternalCliConfig::default();
        cfg.claude.bin_path = Some(r"C:\tools\claude.exe".to_string());
        assert_eq!(resolve_bin(CliBackendId::Claude, &cfg), r"C:\tools\claude.exe");
    }

    #[test]
    fn resolve_bin_ignora_override_vazio() {
        let mut cfg = ExternalCliConfig::default();
        cfg.claude.bin_path = Some("   ".to_string());
        assert_eq!(resolve_bin(CliBackendId::Claude, &cfg), discover_bin(CliBackendId::Claude).unwrap_or_else(|| "claude".into()));
    }

    #[test]
    fn discovery_respects_directory_priority_and_missing_files() {
        let root = std::env::temp_dir().join(format!("cerne-cli-discovery-{}", uuid::Uuid::new_v4()));
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        #[cfg(windows)] let filename = "claude.exe";
        #[cfg(not(windows))] let filename = "claude";
        let file = second.join(filename);
        std::fs::write(&file, b"fixture").unwrap();
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(find_in_dirs("claude", &[first.clone(),second.clone()]), Some(file.clone()));
        assert!(find_in_dirs("absent", &[first.clone(),second.clone()]).is_none());
        std::fs::copy(&file, first.join(filename)).unwrap();
        assert_eq!(find_in_dirs("claude", &[first.clone(),second]), Some(first.join(filename)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "Local installation diagnostic; runs no inference"]
    fn local_cli_installations() {
        for cli in check_readiness(&ExternalCliConfig::default()) {
            println!("{}: installed={} bin={}",cli.backend,cli.installed,cli.bin);
        }
    }

    #[test]
    fn parse_claude_json_le_o_campo_result() {
        let content = parse_claude_json(r#"{"result": "ola", "is_error": false}"#).unwrap();
        assert_eq!(content, "ola");
    }

    #[test]
    fn parse_claude_json_propaga_is_error() {
        let err = parse_claude_json(r#"{"result": "algo deu errado", "is_error": true}"#);
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("algo deu errado"));
    }

    #[test]
    fn parse_gemini_json_le_o_campo_response() {
        let content = parse_gemini_json(r#"{"response": "oi"}"#).unwrap();
        assert_eq!(content, "oi");
    }

    #[test]
    fn parse_gemini_json_aceita_array_de_eventos_do_qwen() {
        let out = r#"[{"type":"system","subtype":"session_start","session_id":"s1"},{"type":"result","result":"pronto"}]"#;
        assert_eq!(parse_gemini_json(out).unwrap(), "pronto");
        let value: serde_json::Value = serde_json::from_str(out).unwrap();
        assert_eq!(find_session_id(&value).as_deref(), Some("s1"));
    }

    #[test]
    fn find_session_id_le_objeto_e_ignora_ausencia() {
        let obj: serde_json::Value = serde_json::from_str(r#"{"session_id":"abc","response":"x"}"#).unwrap();
        assert_eq!(find_session_id(&obj).as_deref(), Some("abc"));
        let none: serde_json::Value = serde_json::from_str(r#"{"response":"x"}"#).unwrap();
        assert_eq!(find_session_id(&none), None);
    }

    #[test]
    fn codex_resume_usa_config_para_sandbox_e_id_no_fim() {
        let args = codex_resume_args(Some("gpt-5.5"), false, "thread-1");
        assert_eq!(args.last().map(String::as_str), Some("thread-1"));
        assert!(args.contains(&"resume".to_string()));
        assert!(args.contains(&"sandbox_mode=\"read-only\"".to_string()));
        assert!(!args.contains(&"--sandbox".to_string()));
    }

    #[test]
    fn pi_agente_nao_desliga_a_sessao() {
        let args = CliBackendId::Pi.build_args(None, true);
        assert!(!args.contains(&"--no-session".to_string()));
    }

    #[test]
    fn detecta_sessao_inexistente_no_cli() {
        assert!(is_missing_session_error("Claude nao concluiu a inferencia: No conversation found with session ID x"));
        assert!(!is_missing_session_error("timeout de rede"));
    }

    #[test]
    fn pi_model_listing_ignores_headers_and_returns_qualified_ids() {
        let text="provider model context max-out thinking images\nopenai example 256K 32K yes yes\nlocal local-model 32K 8K no no\nNo models available";
        assert_eq!(parse_pi_models(text),vec!["openai/example","local/local-model"]);
    }

    #[test]
    fn check_readiness_devolve_os_cinco_backends() {
        let cfg = ExternalCliConfig::default();
        let readiness = check_readiness(&cfg);
        assert_eq!(readiness.len(), 5);
        assert!(readiness.iter().any(|r| r.backend == "claude"));
        assert!(readiness.iter().any(|r| r.backend == "codex"));
        assert!(readiness.iter().any(|r| r.backend == "gemini"));
        assert!(readiness.iter().any(|r| r.backend == "qwen"));
    }
}
