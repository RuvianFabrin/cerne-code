//! Runtime Node embutido no instalador. Os scripts que criam Word, PDF e
//! PowerPoint usam as bibliotecas JS (`docx`, `pdf-lib`, `pptxgenjs`) que vêm
//! junto com o app, então o usuário não instala Node nem npm.
//! Layout em `resources/node-runtime/`: `node.exe` + `node_modules/`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(windows)]
const NODE_BIN: &str = "node.exe";
#[cfg(not(windows))]
const NODE_BIN: &str = "node";

static RUNTIME_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Chamado no setup do Tauri com o diretório de recursos do app instalado.
pub fn set_resource_dir(resource_dir: &Path) {
    let _ = RUNTIME_DIR.set(resource_dir.join("node-runtime"));
}

/// Pasta do binário Node deste sistema dentro do runtime, por OS e arquitetura.
/// Cada instalador traz só o binário da sua plataforma (ver tauri.*.conf.json);
/// no macOS vão os dois (arm64 e x64) e escolhemos pela arquitetura.
fn platform_dir() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "win-x64",
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("linux", "x86_64") => "linux-x64",
        _ => "",
    }
}

/// Caminho do executável Node embutido para este sistema.
pub fn node_binary(runtime: &Path) -> PathBuf {
    runtime.join("bin").join(platform_dir()).join(NODE_BIN)
}

/// Pasta do runtime. Instalado: recurso do app. Em `cargo run`/testes: a pasta
/// de fonte, que tem o mesmo conteúdo.
pub fn runtime_dir() -> Option<PathBuf> {
    if let Some(dir) = RUNTIME_DIR.get() {
        if node_binary(dir).is_file() {
            return Some(dir.clone());
        }
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join("node-runtime");
    node_binary(&source).is_file().then_some(source)
}

/// `NODE_PATH` aponta para o `node_modules` embutido, assim o script resolve
/// `require("docx")` mesmo estando fora da pasta do runtime.
pub fn node_path(runtime: &Path) -> PathBuf {
    runtime.join("node_modules")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-node-runtime-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn runtime_embutido_e_encontrado() {
        let runtime = runtime_dir().expect("node-runtime nao encontrado");
        assert!(node_binary(&runtime).is_file());
        assert!(node_path(&runtime).join("docx").is_dir());
        assert!(node_path(&runtime).join("pdf-lib").is_dir());
        assert!(node_path(&runtime).join("pptxgenjs").is_dir());
    }

    #[test]
    fn script_gera_docx_pptx_e_pdf_sem_node_instalado() {
        let runtime = runtime_dir().expect("node-runtime nao encontrado");
        let dir = scratch_dir("gera");
        let script = dir.join("gera.js");
        let saida = dir.join("saida");
        std::fs::create_dir_all(&saida).unwrap();
        let code = format!(
            r#"
const fs = require("fs");
const {{ Document, Packer, Paragraph, TextRun }} = require("docx");
const PptxGenJS = require("pptxgenjs");
const {{ PDFDocument, StandardFonts }} = require("pdf-lib");
const out = {saida:?};
const doc = new Document({{ sections: [{{ children: [new Paragraph({{ children: [new TextRun("ola")] }})] }}] }});
Packer.toBuffer(doc).then((buf) => fs.writeFileSync(out + "/teste.docx", buf));
const ppt = new PptxGenJS();
ppt.addSlide().addText("ola", {{ x: 1, y: 1, w: 4, h: 1 }});
ppt.writeFile({{ fileName: out + "/teste.pptx" }});
PDFDocument.create().then(async (pdf) => {{
  const page = pdf.addPage();
  page.drawText("ola", {{ x: 50, y: 700, font: await pdf.embedFont(StandardFonts.Helvetica) }});
  fs.writeFileSync(out + "/teste.pdf", await pdf.save());
}});
"#
        );
        std::fs::write(&script, code).unwrap();

        let status = Command::new(node_binary(&runtime))
            .arg(&script)
            .env("NODE_PATH", node_path(&runtime))
            .status()
            .unwrap();
        assert!(status.success());

        // pptxgenjs grava de forma assíncrona; espera o arquivo aparecer.
        for _ in 0..50 {
            if saida.join("teste.pptx").is_file() && saida.join("teste.docx").is_file() && saida.join("teste.pdf").is_file() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        for (arquivo, magic) in [("teste.docx", b"PK"), ("teste.pptx", b"PK"), ("teste.pdf", b"%P")] {
            let bytes = std::fs::read(saida.join(arquivo)).unwrap_or_else(|_| panic!("{arquivo} nao foi gerado"));
            assert!(bytes.starts_with(magic), "{arquivo} com cabecalho invalido");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
