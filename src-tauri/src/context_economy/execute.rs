//! Execution host in Rust. JavaScript uses Boa, never Node; other languages
//! use optional project toolchains. The app's own executable isolates JS.
use crate::agent::shell;
use anyhow::{anyhow, Result};
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsNativeError, JsResult, JsValue, NativeFunction, Source};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const MAX_BYTES: usize = 50 * 1024 * 1024;
#[derive(Serialize, Deserialize, Clone)]
pub struct JsInput {
    pub project: PathBuf,
    pub roots: Vec<PathBuf>,
    pub code: String,
    pub file_content: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
}
struct Host {
    input: JsInput,
    output: RefCell<String>,
    stream: bool,
}

pub fn resolve_read(project: &Path, roots: &[PathBuf], path: &str) -> Result<PathBuf> {
    let target = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        project.join(path)
    };
    let canonical = target.canonicalize()?;
    if !std::iter::once(project)
        .chain(roots.iter().map(PathBuf::as_path))
        .any(|root| {
            root.canonicalize()
                .is_ok_and(|root| canonical.starts_with(root))
        })
    {
        return Err(anyhow!(
            "Caminho fora do projeto e das pastas autorizadas: {path}"
        ));
    }
    Ok(canonical)
}
pub fn read_text(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(anyhow!("Nao e um arquivo regular"));
    }
    if file.metadata()?.len() > MAX_BYTES as u64 {
        return Err(anyhow!("Arquivo excede 50 MB; processe em partes"));
    }
    let mut bytes = vec![];
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(anyhow!("Arquivo excede 50 MB"));
    }
    Ok(crate::encoding::decode(&bytes).0)
}
fn js_error(error: impl std::fmt::Display) -> boa_engine::JsError {
    JsNativeError::error()
        .with_message(error.to_string())
        .into()
}
fn arg(args: &[JsValue], n: usize, context: &mut Context) -> JsResult<String> {
    args.get(n)
        .unwrap_or(&JsValue::undefined())
        .to_string(context)
        .map(|s| s.to_std_string_escaped())
}
fn log(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let text = arg(args, 0, context)?;
    let host = context.get_data::<Host>().unwrap();
    let mut output = host.output.borrow_mut();
    if output.len() + text.len() + 1 > MAX_BYTES {
        return Err(js_error("Saida excede 50 MB"));
    }
    output.push_str(&text);
    output.push('\n');
    if host.stream {
        // stderr is the worker's live output channel; stdout carries final JSON.
        // This retains prints even when the parent must kill the worker.
        let mut live = std::io::stderr().lock();
        live.write_all(text.as_bytes()).map_err(js_error)?;
        live.write_all(b"\n").map_err(js_error)?;
    }
    Ok(JsValue::undefined())
}
fn read(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let path = arg(args, 0, context)?;
    let host = context.get_data::<Host>().unwrap();
    let path = resolve_read(&host.input.project, &host.input.roots, &path).map_err(js_error)?;
    Ok(js_string!(read_text(&path).map_err(js_error)?).into())
}
fn list(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let path = arg(args, 0, context)?;
    let host = context.get_data::<Host>().unwrap();
    let path = resolve_read(&host.input.project, &host.input.roots, &path).map_err(js_error)?;
    let names: Vec<_> = std::fs::read_dir(path)
        .map_err(js_error)?
        .take(10000)
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<_>>()
        .map_err(js_error)?;
    Ok(js_string!(serde_json::to_string(&names).unwrap()).into())
}
fn exists(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let path = arg(args, 0, context)?;
    let host = context.get_data::<Host>().unwrap();
    Ok(resolve_read(&host.input.project, &host.input.roots, &path)
        .is_ok()
        .into())
}
fn write(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let path = arg(args, 0, context)?;
    let text = arg(args, 1, context)?;
    if text.len() > MAX_BYTES {
        return Err(js_error("Escrita excede 50 MB"));
    }
    let host = context.get_data::<Host>().unwrap();
    let target = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        host.input.project.join(&path)
    };
    // Canonical existing parent prevents symlink and '..' escapes on new files.
    let parent = target
        .parent()
        .ok_or_else(|| js_error("Caminho invalido"))?
        .canonicalize()
        .map_err(js_error)?;
    let root = host.input.project.canonicalize().map_err(js_error)?;
    if !parent.starts_with(&root) {
        return Err(js_error("Escrita permitida apenas no projeto"));
    }
    if target.exists() && !target.canonicalize().map_err(js_error)?.starts_with(&root) {
        return Err(js_error("Escrita fora do projeto"));
    }
    std::fs::write(target, text).map_err(js_error)?;
    Ok(JsValue::undefined())
}
fn exec(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let command = arg(args, 0, context)?;
    let host = context.get_data::<Host>().unwrap();
    let shell = shell::detect_shell();
    let mut child = std::process::Command::new(&shell.executable);
    shell::apply_std_creation_flags(&mut child);
    child
        .args(&shell.args_prefix)
        .arg(&command)
        .current_dir(&host.input.project);
    child
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child.spawn().map_err(js_error)?;
    let pid = child.id();
    let bounded = move |mut reader: Box<dyn Read + Send>| -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES {
            shell::kill_pid_tree_blocking(pid);
            return Err(std::io::Error::other("Saida excede 50 MB"));
        }
        Ok(bytes)
    };
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || bounded(Box::new(out)));
    let err = std::thread::spawn(move || bounded(Box::new(err)));
    let status = child.wait().map_err(js_error)?;
    let out = out
        .join()
        .map_err(|_| js_error("Falha ao capturar stdout"))?
        .map_err(js_error)?;
    let err = err
        .join()
        .map_err(|_| js_error("Falha ao capturar stderr"))?
        .map_err(js_error)?;
    if out.len() + err.len() > MAX_BYTES {
        return Err(js_error("Saida excede 50 MB"));
    }
    if !status.success() {
        return Err(js_error(format!(
            "exit {}: {}",
            status.code().unwrap_or(-1),
            shell::decode_output(&err)
        )));
    }
    Ok(js_string!(shell::decode_output(&out)).into())
}

#[cfg(test)]
pub fn javascript(input: JsInput) -> ExecResult {
    javascript_host(input, false)
}
fn javascript_host(mut input: JsInput, stream: bool) -> ExecResult {
    let mut context = Context::default();
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(2_000_000);
    context.runtime_limits_mut().set_recursion_limit(128);
    context.runtime_limits_mut().set_stack_size_limit(4096);
    let file = input.file_content.take().unwrap_or_default();
    let code = std::mem::take(&mut input.code);
    let project = input.project.to_string_lossy().into_owned();
    context.insert_data(Host {
        input,
        output: RefCell::new(String::new()),
        stream,
    });
    let evaluation = (|| -> JsResult<()> {
        // Inject data as a value. Parsing a multi-megabyte quoted JS literal
        // wastes time and memory and conflates untrusted data with source code.
        context.register_global_property(
            js_string!("FILE_CONTENT"),
            js_string!(file),
            Attribute::READONLY,
        )?;
        for (name, len, callback) in [
            (
                "__cerneLog",
                1,
                log as boa_engine::native_function::NativeFunctionPointer,
            ),
            ("__cerneRead", 1, read),
            ("__cerneList", 1, list),
            ("__cerneExists", 1, exists),
            ("__cerneWrite", 2, write),
            ("__cerneExec", 1, exec),
        ] {
            context.register_global_builtin_callable(
                js_string!(name),
                len,
                NativeFunction::from_fn_ptr(callback),
            )?;
        }
        let setup = format!(
            r#"
          const console = Object.freeze({{ log: (...args) => __cerneLog(args.map(x => typeof x === 'object' ? JSON.stringify(x) : String(x)).join(' ')), error: (...args) => __cerneLog(args.map(String).join(' ')), warn: (...args) => __cerneLog(args.map(String).join(' ')) }});
          const process = Object.freeze({{ cwd: () => {}, env: Object.freeze({{}}) }});
          const require = name => {{
            if (name === 'fs' || name === 'node:fs') return Object.freeze({{ readFileSync: __cerneRead, readdirSync: p => JSON.parse(__cerneList(p)), existsSync: __cerneExists, writeFileSync: __cerneWrite }});
            if (name === 'child_process' || name === 'node:child_process') return Object.freeze({{ execSync: __cerneExec }});
            if (name === 'path' || name === 'node:path') return Object.freeze({{ join: (...p) => p.join('/'), basename: p => String(p).split(/[\/\\]/).pop(), dirname: p => String(p).replace(/[\/\\][^\/\\]*$/, ''), extname: p => {{ const b = String(p).split(/[\/\\]/).pop(); const i = b.lastIndexOf('.'); return i > 0 ? b.slice(i) : ''; }} }});
            throw new Error('Modulo indisponivel no motor Rust: ' + name + '. Use ferramentas nativas ou uma linguagem opcional instalada.');
          }};
        "#,
            serde_json::to_string(&project).unwrap()
        );
        context.eval(Source::from_bytes(&setup))?;
        context.eval(Source::from_bytes(&code))?;
        context.run_jobs()?;
        Ok(())
    })();
    let stdout = context.get_data::<Host>().unwrap().output.borrow().clone();
    let stderr = evaluation.err().map(|e| e.to_string()).unwrap_or_default();
    ExecResult {
        stdout,
        exit_code: if stderr.is_empty() { 0 } else { 1 },
        stderr,
        timed_out: false,
    }
}

pub fn worker() -> i32 {
    let result = (|| -> Result<ExecResult> {
        let mut bytes = vec![];
        std::io::stdin()
            .take(MAX_BYTES as u64 * 2)
            .read_to_end(&mut bytes)?;
        let input: JsInput = serde_json::from_slice(&bytes)?;
        Ok(javascript_host(input, true))
    })();
    let result = result.unwrap_or_else(|e| ExecResult {
        stdout: String::new(),
        stderr: e.to_string(),
        exit_code: 1,
        timed_out: false,
    });
    if let Ok(bytes) = serde_json::to_vec(&result) {
        let _ = std::io::stdout().write_all(&bytes);
    }
    0
}

struct ProcessTree(Option<u32>);
impl Drop for ProcessTree {
    fn drop(&mut self) {
        if let Some(pid) = self.0.take() {
            shell::kill_pid_tree_blocking(pid);
        }
    }
}

type Capture = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;
async fn read_capture(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    cap: usize,
    capture: Capture,
) -> Result<()> {
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }
        let mut bytes = capture.lock().unwrap();
        if bytes.len().saturating_add(n) > cap {
            return Err(anyhow!(
                "Saida excede o limite de armazenamento; processo encerrado"
            ));
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
}
#[cfg(test)]
async fn bounded_output(reader: impl tokio::io::AsyncRead + Unpin, cap: usize) -> Result<Vec<u8>> {
    let capture = Capture::default();
    read_capture(reader, cap, capture.clone()).await?;
    let bytes = std::mem::take(&mut *capture.lock().unwrap());
    Ok(bytes)
}

async fn output(
    mut command: tokio::process::Command,
    input: Option<Vec<u8>>,
    timeout: u64,
) -> Result<ExecResult> {
    shell::apply_creation_flags(&mut command);
    shell::apply_process_group(&mut command);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let mut tree = ProcessTree(child.id());
    // A worker's JSON can escape each output byte into six protocol bytes.
    let is_worker = input.is_some();
    let cap = if is_worker {
        MAX_BYTES * 6 + 4096
    } else {
        MAX_BYTES
    };
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    if let Some(bytes) = input {
        let mut stdin = child.stdin.take().unwrap();
        // Awaiting the writer is part of the same timeout, including large FILE_CONTENT.
        let write = tokio::spawn(async move { stdin.write_all(&bytes).await });
        let _ = write;
    } else {
        drop(child.stdin.take());
    }
    let captured_out = Capture::default();
    let captured_err = Capture::default();
    let pending = async {
        let (status, _, _) = tokio::try_join!(
            async { Ok::<_, anyhow::Error>(child.wait().await?) },
            read_capture(stdout, cap, captured_out.clone()),
            read_capture(stderr, MAX_BYTES, captured_err.clone())
        )?;
        Ok::<_, anyhow::Error>(status)
    };
    match tokio::time::timeout(Duration::from_millis(timeout.clamp(100, 300_000)), pending).await {
        Ok(result) => {
            let status = result?;
            let stdout = std::mem::take(&mut *captured_out.lock().unwrap());
            let stderr = std::mem::take(&mut *captured_err.lock().unwrap());
            tree.0 = None;
            if stdout.len() + stderr.len() > cap + if is_worker { MAX_BYTES } else { 0 } {
                return Err(anyhow!("Saida excede o limite de armazenamento"));
            }
            Ok(ExecResult {
                stdout: shell::strip_ansi(&shell::decode_output(&stdout)),
                stderr: shell::strip_ansi(&shell::decode_output(&stderr)),
                exit_code: status.code().unwrap_or(-1),
                timed_out: false,
            })
        }
        Err(_) => {
            drop(tree);
            Ok(ExecResult {
                stdout: shell::strip_ansi(&shell::decode_output(&captured_out.lock().unwrap())),
                stderr: format!(
                    "{}\nTempo limite atingido; processo e filhos encerrados",
                    shell::strip_ansi(&shell::decode_output(&captured_err.lock().unwrap()))
                ),
                exit_code: -1,
                timed_out: true,
            })
        }
    }
}

struct ScriptDirectory(PathBuf);
impl ScriptDirectory {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().canonicalize()?;
        let path = root.join(format!("cerne-context-script-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for ScriptDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn php_literal(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn perl_literal(text: &str) -> String {
    php_literal(text)
}
fn csharp_literal(text: &str) -> String {
    format!("@\"{}\"", text.replace('"', "\"\""))
}

pub async fn run(
    language: &str,
    code: &str,
    project: &Path,
    roots: &[PathBuf],
    file: Option<String>,
    timeout: u64,
) -> Result<ExecResult> {
    if language == "javascript" {
        let input = JsInput {
            project: project.into(),
            roots: roots.into(),
            code: code.into(),
            file_content: file,
        };
        #[cfg(test)]
        {
            return Ok(javascript(input));
        }
        #[cfg(not(test))]
        {
            let mut command = tokio::process::Command::new(std::env::current_exe()?);
            command.arg("--cerne-context-worker");
            let outcome = output(command, Some(serde_json::to_vec(&input)?), timeout).await?;
            if outcome.timed_out {
                let (printed, reason) = outcome
                    .stderr
                    .rsplit_once("\nTempo limite atingido")
                    .unwrap_or(("", &outcome.stderr));
                return Ok(ExecResult {
                    stdout: printed.into(),
                    stderr: format!("Tempo limite atingido{reason}"),
                    exit_code: -1,
                    timed_out: true,
                });
            }
            return serde_json::from_str(&outcome.stdout)
                .map_err(|e| anyhow!("Falha no executor Rust: {e}. {}", outcome.stderr));
        }
    }
    if language == "shell" {
        let mut command = shell::build_shell_command(code);
        command.current_dir(project);
        if let Some(file) = file {
            command.env("FILE_CONTENT", file);
        }
        return output(command, None, timeout).await;
    }
    let temporary = ScriptDirectory::new()?;
    let content_path = temporary.0.join("input.txt");
    let file_available = file.is_some();
    if let Some(file) = file {
        std::fs::write(&content_path, file)?;
    }
    let path = serde_json::to_string(&content_path.to_string_lossy())?;
    let (runtime, extension) = match language {
        "python" => (if cfg!(windows) { "python" } else { "python3" }, "py"),
        "ruby" => ("ruby", "rb"),
        "php" => ("php", "php"),
        "perl" => ("perl", "pl"),
        "r" => ("Rscript", "R"),
        "elixir" => ("elixir", "exs"),
        "go" => ("go", "go"),
        "rust" => ("rustc", "rs"),
        "typescript" => ("deno", "ts"),
        "csharp" => ("dotnet-script", "csx"),
        _ => return Err(anyhow!("Linguagem desconhecida: {language}")),
    };
    let mut source = code.to_string();
    if file_available {
        source=match language {
            "python"=>format!("import json\nFILE_CONTENT_PATH=json.loads({})\nfile_path=FILE_CONTENT_PATH\nwith open(FILE_CONTENT_PATH, encoding='utf-8') as _f: FILE_CONTENT=_f.read()\n{code}",serde_json::to_string(&path)?),
            "ruby"=>format!("FILE_CONTENT_PATH={path}\nfile_path=FILE_CONTENT_PATH\nFILE_CONTENT=File.read(file_path, encoding: 'utf-8')\n{code}"),
            "php"=>format!("<?php\n$FILE_CONTENT_PATH={};\n$file_path=$FILE_CONTENT_PATH;\n$FILE_CONTENT=file_get_contents($file_path);\n{code}",php_literal(&content_path.to_string_lossy())),
            "perl"=>format!("my $FILE_CONTENT_PATH={}; my $file_path=$FILE_CONTENT_PATH;\nopen(my $fh, '<:encoding(UTF-8)', $file_path) or die $!;\nmy $FILE_CONTENT=do {{local $/; <$fh>}};close($fh);\n{code}",perl_literal(&content_path.to_string_lossy())),
            "r"=>format!("FILE_CONTENT_PATH <- {path}\nfile_path <- FILE_CONTENT_PATH\nFILE_CONTENT <- paste(readLines(file_path, warn=FALSE, encoding='UTF-8'), collapse='\\n')\n{code}"),
            "elixir"=>format!("file_content_path={path}\nfile_path=file_content_path\nfile_content=File.read!(file_path)\n{code}"),
            "go"=>format!("package main\nimport (\"fmt\"; \"os\")\nfunc main() {{ b,err:=os.ReadFile({path}); if err!=nil {{panic(err)}}; FILE_CONTENT:=string(b);FILE_CONTENT_PATH:={path};file_path:=FILE_CONTENT_PATH;_=FILE_CONTENT;_=file_path;_=fmt.Sprint();\n{code}\n}}"),
            "rust"=>format!("#![allow(unused_variables)]\nfn main() {{let file_content_path={:?}; let file_path=file_content_path;let file_content=std::fs::read_to_string(file_path).unwrap();let FILE_CONTENT=&file_content;\n{code}\n}}",content_path.to_string_lossy()),
            "typescript"=>format!("const FILE_CONTENT_PATH={path};const file_path=FILE_CONTENT_PATH;const FILE_CONTENT=Deno.readTextFileSync(file_path);\n{code}"),
            "csharp"=>format!("var FILE_CONTENT_PATH={}; var file_path=FILE_CONTENT_PATH;var FILE_CONTENT=System.IO.File.ReadAllText(file_path);\n{code}",csharp_literal(&content_path.to_string_lossy())),
            _=>unreachable!(),
        };
    } else if language == "go" && !code.contains("package ") {
        source = format!("package main\nimport \"fmt\"\nfunc main() {{_=fmt.Sprint();\n{code}\n}}");
    } else if language == "php" && !code.trim_start().starts_with("<?") {
        source = format!("<?php\n{code}");
    }
    let script = temporary.0.join(format!("script.{extension}"));
    std::fs::write(&script, source)?;
    let started = std::time::Instant::now();
    let mut command = tokio::process::Command::new(runtime);
    command.current_dir(project);
    if language == "rust" {
        let binary = temporary.0.join(if cfg!(windows) {
            "script.exe"
        } else {
            "script"
        });
        command.arg(&script).arg("-o").arg(&binary);
        let compile = output(command, None, timeout).await?;
        if compile.exit_code != 0 || compile.timed_out {
            return Ok(compile);
        }
        let remaining =
            timeout.saturating_sub(started.elapsed().as_millis().min(u64::MAX as u128) as u64);
        if remaining < 100 {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "Tempo limite atingido durante compilacao".into(),
                exit_code: -1,
                timed_out: true,
            });
        }
        let mut command = tokio::process::Command::new(binary);
        command.current_dir(project);
        return output(command, None, remaining).await;
    }
    if language == "go" {
        command.arg("run");
    }
    if language == "typescript" {
        command.args([
            "run",
            "--no-prompt",
            "--no-remote",
            "--allow-read",
            "--allow-write",
            "--allow-run",
        ]);
    }
    command.arg(script);
    output(command, None, timeout).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn capture_enforces_the_limit_while_reading() {
        assert!(bounded_output(&b"0123456789"[..], 4).await.is_err());
        assert_eq!(bounded_output(&b"0123"[..], 4).await.unwrap(), b"0123");
    }
    #[tokio::test]
    async fn shell_timeout_terminates_execution() {
        let code = if cfg!(windows) {
            "Start-Sleep -Seconds 30"
        } else {
            "sleep 30"
        };
        let result = run("shell", code, &std::env::temp_dir(), &[], None, 500)
            .await
            .unwrap();
        assert!(result.timed_out);
        assert_eq!(result.exit_code, -1);
    }
    #[tokio::test]
    async fn shell_timeout_preserves_output_already_printed() {
        let code = if cfg!(windows) {
            "Write-Output 'beforetimeout731'; Start-Sleep -Seconds 30"
        } else {
            "printf beforetimeout731; sleep 30"
        };
        let result = run("shell", code, &std::env::temp_dir(), &[], None, 2000)
            .await
            .unwrap();
        assert!(result.timed_out);
        assert!(result.stdout.contains("beforetimeout731"));
    }
    #[tokio::test]
    async fn optional_rust_compiles_file_analysis_without_node() {
        if std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let result=run("rust","println!(\"{}\", file_content.lines().filter(|s|s.contains(\"ERROR\")).collect::<Vec<_>>().join(\"\\n\"));",&std::env::temp_dir(),&[],Some("OK\nERROR ação 中文".into()),30_000).await.unwrap();
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert!(result.stdout.contains("ERROR ação 中文"));
    }
    #[test]
    fn javascript_reduces_large_file_without_node() {
        let result = javascript(JsInput {
            project: std::env::temp_dir(),
            roots: vec![],
            code:
                "console.log(FILE_CONTENT.split('\\n').filter(x=>x.includes('ERROR')).join('\\n'))"
                    .into(),
            file_content: Some(format!("{}ERROR rarecerne731", "OK\n".repeat(4000))),
        });
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(result.stdout.trim(), "ERROR rarecerne731");
    }
    #[test]
    fn javascript_runs_promise_jobs() {
        let result = javascript(JsInput {
            project: std::env::temp_dir(),
            roots: vec![],
            code: "Promise.resolve('resolvedjob731').then(value => console.log(value))".into(),
            file_content: None,
        });
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(result.stdout.trim(), "resolvedjob731");
    }
    #[test]
    fn javascript_loop_has_a_limit() {
        let result = javascript(JsInput {
            project: std::env::temp_dir(),
            roots: vec![],
            code: "while(true){}".into(),
            file_content: None,
        });
        assert_ne!(result.exit_code, 0);
    }
    #[test]
    fn node_modules_are_not_silently_faked() {
        let result = javascript(JsInput {
            project: std::env::temp_dir(),
            roots: vec![],
            code: "require('node:http')".into(),
            file_content: None,
        });
        assert!(result.stderr.contains("indisponivel"));
    }
}
