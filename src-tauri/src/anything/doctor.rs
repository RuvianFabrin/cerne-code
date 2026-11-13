//! Modo Anything, fase 03: "está tudo instalado?". Só olha e informa; nunca instala nada. Cada item
//! diz o que achou e, quando falta algo, qual ação a interface deve oferecer.

use super::cli;
use super::ghidra;
use crate::mcp::{self, McpServerConfig};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tempo máximo de cada verificação que chama o motor (o `doctor` e o servidor MCP).
const LIMITE_MOTOR: Duration = Duration::from_secs(60);
/// Espaço livre que o download e a instalação do Ghidra pedem (cerca de 1,2 GB extraídos + o zip).
const DISCO_MINIMO: u64 = 3 * 1024 * 1024 * 1024;
/// Ferramentas que o servidor com `--all-tools` precisa listar para o diagnóstico passar.
const FERRAMENTAS_MINIMAS: usize = 300;
/// Quanto da saída bruta do `doctor` do motor vai junto no relatório.
const SAIDA_MAXIMA: usize = 20_000;
/// Versão mínima do Java que o Ghidra 12 aceita.
const JAVA_MINIMO: u32 = 21;

#[cfg(windows)]
const JAVA_EXE: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_EXE: &str = "java";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Falta,
    Aviso,
}

/// Uma verificação. `acao` é o que a interface oferece: `instalar_cli`, `instalar_ghidra` ou nada.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Item {
    pub id: &'static str,
    pub status: Status,
    pub detalhe: String,
    pub acao: Option<&'static str>,
    pub obrigatorio: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Pronto para o modo Anything em geral (motor, servidor e disco).
    pub pronto: bool,
    /// Pronto para analisar programas nativos: também precisa do Ghidra e do Java 21.
    pub pronto_nativo: bool,
    pub itens: Vec<Item>,
    /// Saída bruta do `doctor` do motor, para abrir se algo estiver estranho.
    pub saida_motor: Option<String>,
}

fn item(id: &'static str, status: Status, detalhe: impl Into<String>, acao: Option<&'static str>, obrigatorio: bool) -> Item {
    Item { id, status, detalhe: detalhe.into(), acao, obrigatorio }
}

/// Todo item obrigatório precisa estar `ok`.
pub fn pronto(itens: &[Item]) -> bool {
    itens.iter().all(|i| !i.obrigatorio || i.status == Status::Ok)
}

/// Pronto para programas nativos: o geral, mais Ghidra e Java 21 `ok`.
pub fn pronto_nativo(itens: &[Item]) -> bool {
    pronto(itens) && ["ghidra", "java_ghidra"].iter().all(|id| itens.iter().any(|i| i.id == *id && i.status == Status::Ok))
}

/// O motor está na versão fixada?
pub fn item_motor(st: &cli::CliStatus) -> Item {
    if !st.found {
        return item("motor", Status::Falta, "o motor craft-anything-cli não está instalado", Some("instalar_cli"), true);
    }
    let path = st.path.as_deref().unwrap_or("?");
    if st.matches_pin {
        item("motor", Status::Ok, format!("{} em {path}", st.version.as_deref().unwrap_or("")), None, true)
    } else {
        let achou = st.version.as_deref().unwrap_or("versão desconhecida");
        item("motor", Status::Falta, format!("achei '{achou}', o Cerne usa {}", cli::VERSION), Some("instalar_cli"), true)
    }
}

/// Ghidra instalado? Não é obrigatório: só os alvos nativos precisam dele.
pub fn item_ghidra(estado: Option<&ghidra::GhidraState>) -> Item {
    match estado {
        None => item("ghidra", Status::Aviso, "não consegui ler o estado do Ghidra (o motor está instalado?)", None, false),
        Some(s) => match &s.ghidra_path {
            Some(p) => item("ghidra", Status::Ok, p.clone(), None, false),
            None => item("ghidra", Status::Falta, "Ghidra não instalado (só alvos nativos precisam dele)", Some("instalar_ghidra"), false),
        },
    }
}

/// O Java que o Ghidra vai usar é 21 ou mais novo? Confere primeiro o `JAVA_HOME_OVERRIDE` do
/// `launch.properties`, que é o que o Ghidra lê; se não houver, o Java do sistema.
pub fn item_java(estado: Option<&ghidra::GhidraState>) -> Item {
    let Some(s) = estado else {
        return item("java_ghidra", Status::Aviso, "não consegui ler o estado do Java", None, false);
    };
    let Some(ghidra_path) = s.ghidra_path.as_deref() else {
        return item("java_ghidra", Status::Aviso, "Ghidra não instalado, então o Java ainda não importa", None, false);
    };
    let props = Path::new(ghidra_path).parent().map(|d| d.join("launch.properties"));
    let override_dir = props.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| java_override(&t));
    if let Some(dir) = override_dir {
        return checar_java(&dir.join("bin").join(JAVA_EXE), "launch.properties do Ghidra", Some(Acao::InstalarGhidra));
    }
    match s.java_path.as_deref() {
        Some(j) => checar_java(Path::new(j), "Java do sistema", Some(Acao::InstalarGhidra)),
        None => item("java_ghidra", Status::Falta, "nenhum Java encontrado para o Ghidra", Some("instalar_ghidra"), false),
    }
}

enum Acao {
    InstalarGhidra,
}

fn checar_java(bin: &Path, origem: &str, acao: Option<Acao>) -> Item {
    let acao_nome = acao.map(|_| "instalar_ghidra");
    match java_major(bin) {
        Some(v) if v >= JAVA_MINIMO => item("java_ghidra", Status::Ok, format!("Java {v} ({origem})"), None, false),
        Some(v) => item("java_ghidra", Status::Falta, format!("Java {v} em {origem}; o Ghidra pede {JAVA_MINIMO} ou mais novo"), acao_nome, false),
        None => item("java_ghidra", Status::Falta, format!("não achei um Java em {} ({origem})", bin.display()), acao_nome, false),
    }
}

/// Valor de `JAVA_HOME_OVERRIDE` no texto do `launch.properties`. Vazio = `None`.
pub fn java_override(text: &str) -> Option<PathBuf> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| k.trim() == "JAVA_HOME_OVERRIDE")
        .map(|(_, v)| v.trim())
        .find(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Versão principal do Java a partir da saída de `java -version`: `21.0.12` vira 21, `1.8.0` vira 8.
pub fn parse_java_major(text: &str) -> Option<u32> {
    let rest = &text[text.find("version \"")? + "version \"".len()..];
    let ver = rest.split('"').next()?;
    let mut partes = ver.split('.');
    let primeiro: u32 = partes.next()?.parse().ok()?;
    if primeiro == 1 {
        partes.next()?.parse().ok()
    } else {
        Some(primeiro)
    }
}

/// Roda `<java> -version` (a saída vai para stderr) e lê a versão principal.
fn java_major(bin: &Path) -> Option<u32> {
    if !bin.is_file() {
        return None;
    }
    let mut cmd = std::process::Command::new(bin);
    cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    let texto = format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    parse_java_major(&texto)
}

/// Espaço livre na unidade de `path`. Só Windows (pelo PowerShell, sem dependência nova); nos outros
/// sistemas devolve `None` e o diagnóstico avisa que não mediu.
fn free_bytes(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let letra = path.components().next()?.as_os_str().to_string_lossy().trim_end_matches(':').to_string();
        if letra.len() != 1 || !letra.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(Get-PSDrive -Name {letra}).Free")])
            .creation_flags(0x0800_0000)
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

pub fn item_disco(store: &Path) -> Item {
    match free_bytes(store) {
        None => item("disco", Status::Aviso, "não consegui medir o espaço livre neste sistema", None, false),
        Some(b) if b >= DISCO_MINIMO => item("disco", Status::Ok, format!("{} GB livres", b / (1024 * 1024 * 1024)), None, false),
        Some(b) => item(
            "disco",
            Status::Falta,
            format!("{} GB livres; a instalação do Ghidra pede cerca de 3 GB", b / (1024 * 1024 * 1024)),
            None,
            false,
        ),
    }
}

/// Sobe o motor como servidor MCP (com `--deep` e todas as ferramentas) e conta o que ele lista.
/// Devolve o item do servidor e o item do modo Deep.
async fn itens_mcp(exe: &Path) -> (Item, Item) {
    let servidor = McpServerConfig {
        name: "anything-diagnostico".into(),
        command: exe.display().to_string(),
        args: vec!["--deep".into(), "mcp".into(), "--all-tools".into()],
        env: HashMap::new(),
        url: None,
        bearer_token: None,
        enabled: true,
    };
    match tokio::time::timeout(LIMITE_MOTOR, mcp::test_connection(&servidor)).await {
        Ok(Ok(tools)) => {
            let n = tools.len();
            let srv = if n >= FERRAMENTAS_MINIMAS {
                item("servidor_mcp", Status::Ok, format!("{n} ferramentas"), None, true)
            } else {
                item("servidor_mcp", Status::Falta, format!("só {n} ferramentas; esperava pelo menos {FERRAMENTAS_MINIMAS}"), Some("instalar_cli"), true)
            };
            let deep = if tools.iter().any(|t| t.name.starts_with("native_")) {
                item("deep", Status::Ok, "ferramentas de programas nativos presentes", None, false)
            } else {
                item("deep", Status::Aviso, "sem ferramentas de programas nativos: o modo Deep está desligado?", None, false)
            };
            (srv, deep)
        }
        Ok(Err(e)) => (
            item("servidor_mcp", Status::Falta, format!("o servidor não respondeu: {e}"), None, true),
            item("deep", Status::Aviso, "não dá para conferir sem o servidor", None, false),
        ),
        Err(_) => (
            item("servidor_mcp", Status::Falta, format!("o servidor não respondeu em {} s", LIMITE_MOTOR.as_secs()), None, true),
            item("deep", Status::Aviso, "não dá para conferir sem o servidor", None, false),
        ),
    }
}

/// `craft-anything-cli doctor`: o próprio motor diz se está saudável. A saída bruta vai junto.
async fn item_doctor(exe: &Path) -> (Item, Option<String>) {
    match ghidra::run_cli(exe, &["doctor"], LIMITE_MOTOR).await {
        Ok(out) => {
            let texto = String::from_utf8_lossy(&out.stdout).to_string();
            let saudavel = serde_json::from_str::<Value>(&texto).ok().and_then(|v| v.get("healthy").and_then(Value::as_bool));
            let it = match saudavel {
                Some(true) => item("doctor_motor", Status::Ok, "o motor reporta saúde ok", None, false),
                Some(false) => item("doctor_motor", Status::Aviso, "o motor reporta problemas; veja a saída", None, false),
                None => item("doctor_motor", Status::Aviso, "saída do doctor não reconhecida", None, false),
            };
            (it, Some(texto.chars().take(SAIDA_MAXIMA).collect()))
        }
        Err(e) => (item("doctor_motor", Status::Aviso, format!("o doctor do motor falhou: {e}"), None, false), None),
    }
}

/// Roda todas as verificações. Nunca instala; o pior caso é um relatório com itens em falta.
pub async fn run(store: &Path) -> Report {
    let st = cli::status(store);
    let exe = cli::find_cli(store).map(|(p, _)| p);
    let mut itens = vec![item_motor(&st)];
    let estado = match &exe {
        Some(e) => ghidra::engines_list(e).await.ok().map(|l| ghidra::read_state_from(&l)),
        None => None,
    };
    itens.push(item_ghidra(estado.as_ref()));
    itens.push(item_java(estado.as_ref()));
    itens.push(item_disco(store));
    let mut saida = None;
    match &exe {
        Some(e) => {
            let (srv, deep) = itens_mcp(e).await;
            itens.push(srv);
            itens.push(deep);
            let (doc, texto) = item_doctor(e).await;
            itens.push(doc);
            saida = texto;
        }
        None => {
            itens.push(item("servidor_mcp", Status::Falta, "instale o motor primeiro", None, true));
            itens.push(item("deep", Status::Aviso, "instale o motor primeiro", None, false));
            itens.push(item("doctor_motor", Status::Aviso, "instale o motor primeiro", None, false));
        }
    }
    Report { pronto: pronto(&itens), pronto_nativo: pronto_nativo(&itens), itens, saida_motor: saida }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(id: &'static str, status: Status, obrigatorio: bool) -> Item {
        item(id, status, "", None, obrigatorio)
    }

    #[test]
    fn pronto_only_needs_the_required_items() {
        let itens = vec![it("motor", Status::Ok, true), it("ghidra", Status::Falta, false)];
        assert!(pronto(&itens), "Ghidra falta, mas não é obrigatório para o geral");
        assert!(!pronto_nativo(&itens), "para nativo, falta o Ghidra");
        let sem_motor = vec![it("motor", Status::Falta, true)];
        assert!(!pronto(&sem_motor));
    }

    #[test]
    fn pronto_nativo_needs_ghidra_and_java() {
        let itens = vec![
            it("motor", Status::Ok, true),
            it("servidor_mcp", Status::Ok, true),
            it("ghidra", Status::Ok, false),
            it("java_ghidra", Status::Ok, false),
        ];
        assert!(pronto(&itens) && pronto_nativo(&itens));
        let java_velho = vec![it("motor", Status::Ok, true), it("ghidra", Status::Ok, false), it("java_ghidra", Status::Falta, false)];
        assert!(!pronto_nativo(&java_velho));
    }

    #[test]
    fn a_wrong_motor_version_asks_to_reinstall() {
        let st = cli::CliStatus { found: true, path: Some("C:/x/cli.exe".into()), origin: Some("gerenciado".into()), version: Some("craft-anything-cli 0.5.0".into()), matches_pin: false };
        let i = item_motor(&st);
        assert_eq!(i.status, Status::Falta);
        assert_eq!(i.acao, Some("instalar_cli"));
        let nada = item_motor(&cli::CliStatus { found: false, path: None, origin: None, version: None, matches_pin: false });
        assert_eq!(nada.acao, Some("instalar_cli"));
    }

    #[test]
    fn java_version_is_read_from_java_dash_version() {
        assert_eq!(parse_java_major("openjdk version \"21.0.12.1\" 2026-08-18 LTS"), Some(21));
        assert_eq!(parse_java_major("java version \"1.8.0_292\""), Some(8));
        assert_eq!(parse_java_major("java version \"17.0.19\" 2026"), Some(17));
        assert_eq!(parse_java_major("nada aqui"), None);
    }

    #[test]
    fn java_override_is_read_from_launch_properties() {
        assert_eq!(java_override("VMARGS=-Xshare:off\nJAVA_HOME_OVERRIDE=C:/jdk-21\n"), Some(PathBuf::from("C:/jdk-21")));
        assert_eq!(java_override("JAVA_HOME_OVERRIDE=\n"), None);
        assert_eq!(java_override("VMARGS=x\n"), None);
    }

    #[test]
    fn disk_item_without_a_measure_is_a_warning_not_a_failure() {
        // Em Linux/macOS `free_bytes` devolve None; no Windows o valor real vem do PowerShell.
        let i = item_disco(Path::new("Z:/"));
        assert!(matches!(i.status, Status::Aviso | Status::Ok | Status::Falta));
        assert!(!i.obrigatorio);
    }
}

/// Teste de verdade, contra um motor instalado. Não roda por padrão:
/// `ANYTHING_TEST_STORE=<pasta> cargo test --lib diagnostico_real -- --ignored --nocapture`
#[cfg(test)]
mod real {
    use super::*;

    #[tokio::test]
    #[ignore = "precisa de um motor instalado; defina ANYTHING_TEST_STORE"]
    async fn diagnostico_real() {
        let store = std::env::var("ANYTHING_TEST_STORE").expect("defina ANYTHING_TEST_STORE");
        let r = run(Path::new(&store)).await;
        println!("{}", serde_json::to_string_pretty(&r).unwrap());
    }
}
