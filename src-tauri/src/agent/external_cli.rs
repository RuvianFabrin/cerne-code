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
}

impl CliBackendId {
    pub fn all() -> [CliBackendId; 4] {
        [
            CliBackendId::Claude,
            CliBackendId::Codex,
            CliBackendId::Gemini,
            CliBackendId::Qwen,
        ]
    }

    pub fn id(self) -> &'static str {
        match self {
            CliBackendId::Claude => "claude",
            CliBackendId::Codex => "codex",
            CliBackendId::Gemini => "gemini",
            CliBackendId::Qwen => "qwen",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CliBackendId::Claude => "Claude Code CLI",
            CliBackendId::Codex => "Codex CLI",
            CliBackendId::Gemini => "Gemini CLI",
            CliBackendId::Qwen => "Qwen Code CLI",
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
        }
    }

    fn prompt_via(self) -> PromptVia {
        match self {
            CliBackendId::Claude | CliBackendId::Codex => PromptVia::Stdin,
            CliBackendId::Gemini | CliBackendId::Qwen => PromptVia::Arg,
        }
    }

    fn output_parse(self) -> OutputParse {
        match self {
            CliBackendId::Claude => OutputParse::ClaudeJson,
            CliBackendId::Codex => OutputParse::LastMessageFile,
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
}

impl ExternalCliConfig {
    fn override_for(&self, backend: CliBackendId) -> &CliBackendOverride {
        match backend {
            CliBackendId::Claude => &self.claude,
            CliBackendId::Codex => &self.codex,
            CliBackendId::Gemini => &self.gemini,
            CliBackendId::Qwen => &self.qwen,
        }
    }
}

/// Resolve o caminho/nome do binário: override explícito do usuário, senão o
/// nome default (procurado no PATH pelo próprio `Command::new`/`shell::command_exists`).
fn resolve_bin(backend: CliBackendId, cfg: &ExternalCliConfig) -> String {
    cfg.override_for(backend)
        .bin_path
        .clone()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| backend.bin_default().to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct CliReadiness {
    pub backend: &'static str,
    pub label: &'static str,
    pub installed: bool,
    pub bin: String,
    pub models: Vec<&'static str>,
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
                models: backend.known_models().to_vec(),
            }
        })
        .collect()
}

const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Dispara uma chamada não-interativa pro CLI externo e devolve o texto da
/// resposta final. Nunca faz streaming de verdade (os CLIs não garantem um
/// formato incremental limpo em modo não-interativo) — o turno inteiro do
/// CLI roda, e o texto completo chega de uma vez quando o processo termina.
pub async fn dispatch(
    backend: CliBackendId,
    prompt: &str,
    model: Option<&str>,
    full_access: bool,
    cfg: &ExternalCliConfig,
) -> Result<String> {
    let bin = resolve_bin(backend, cfg);
    let mut args = backend.build_args(model, full_access);
    args.extend(cfg.override_for(backend).extra_args.iter().cloned());

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

    if matches!(backend.prompt_via(), PromptVia::Arg) {
        args.push("-p".into());
        args.push(prompt.to_string());
    }

    let mut command = tokio::process::Command::new(&bin);
    command
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    shell::apply_creation_flags(&mut command);
    shell::apply_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| anyhow!("nao foi possivel iniciar '{bin}' ({backend_label}): {e} — confira em Configuracoes se o CLI esta instalado e no PATH", backend_label = backend.label()))?;

    if matches!(backend.prompt_via(), PromptVia::Stdin) {
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }
    } else {
        drop(child.stdin.take());
    }

    let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("{} nao respondeu em {}s (timeout)", backend.label(), DEFAULT_TIMEOUT_SECS))?
        .map_err(|e| anyhow!("erro esperando '{bin}' terminar: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let content = match backend.output_parse() {
        OutputParse::ClaudeJson => parse_claude_json(&stdout)?,
        OutputParse::GeminiJson => parse_gemini_json(&stdout)?,
        OutputParse::LastMessageFile => {
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

    Ok(content)
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
    Ok(value
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string())
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
        assert_eq!(resolve_bin(CliBackendId::Claude, &cfg), "claude");
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
        assert_eq!(resolve_bin(CliBackendId::Claude, &cfg), "claude");
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
    fn check_readiness_devolve_os_quatro_backends() {
        let cfg = ExternalCliConfig::default();
        let readiness = check_readiness(&cfg);
        assert_eq!(readiness.len(), 4);
        assert!(readiness.iter().any(|r| r.backend == "claude"));
        assert!(readiness.iter().any(|r| r.backend == "codex"));
        assert!(readiness.iter().any(|r| r.backend == "gemini"));
        assert!(readiness.iter().any(|r| r.backend == "qwen"));
    }
}
