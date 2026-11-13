//! Varredora de pasta: percorre um diretório do projeto, acha os executáveis pela assinatura do arquivo
//! (não pela extensão) e grava `.anything/alvos.json` com o caminho completo e o status de cada um.
//! Ainda não decompila: o estudo de cada alvo é uma tarefa que a fila leva para a frente.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Pastas que a varredora não entra.
const IGNORADAS: &[&str] = &[".anything", ".git", "node_modules", "target"];
/// Profundidade máxima da varredura.
const PROFUNDIDADE_MAXIMA: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Alvo {
    pub caminho: String,
    pub formato: String,
    pub status: String,
}

/// Formato pela assinatura dos primeiros bytes. `None` = não é executável que o motor conhece.
pub fn formato(cabecalho: &[u8]) -> Option<&'static str> {
    if cabecalho.starts_with(b"MZ") {
        Some("pe")
    } else if cabecalho.starts_with(&[0x7f, b'E', b'L', b'F']) {
        Some("elf")
    } else if cabecalho.starts_with(&[0xfe, 0xed, 0xfa, 0xce]) || cabecalho.starts_with(&[0xfe, 0xed, 0xfa, 0xcf]) || cabecalho.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) || cabecalho.starts_with(&[0xce, 0xfa, 0xed, 0xfe]) {
        Some("macho")
    } else {
        None
    }
}

/// Percorre `dir` e devolve os executáveis achados, em ordem de caminho (estável).
pub fn varrer(dir: &Path) -> Result<Vec<Alvo>> {
    let mut achados = Vec::new();
    recolher(dir, 0, &mut achados)?;
    achados.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    Ok(achados)
}

fn recolher(dir: &Path, profundidade: usize, achados: &mut Vec<Alvo>) -> Result<()> {
    if profundidade > PROFUNDIDADE_MAXIMA {
        return Ok(());
    }
    for entrada in std::fs::read_dir(dir)?.flatten() {
        let caminho: PathBuf = entrada.path();
        let nome = entrada.file_name().to_string_lossy().to_string();
        if caminho.is_dir() {
            if !IGNORADAS.contains(&nome.as_str()) {
                recolher(&caminho, profundidade + 1, achados)?;
            }
        } else if caminho.is_file() {
            if let Some(f) = ler_formato(&caminho) {
                achados.push(Alvo { caminho: caminho.display().to_string(), formato: f.to_string(), status: "fazer".into() });
            }
        }
    }
    Ok(())
}

fn ler_formato(caminho: &Path) -> Option<&'static str> {
    use std::io::Read;
    let mut cabecalho = [0u8; 4];
    let mut f = std::fs::File::open(caminho).ok()?;
    let n = f.read(&mut cabecalho).ok()?;
    formato(&cabecalho[..n])
}

/// Grava `alvos.json` na pasta `.anything` do projeto (escrita atômica).
pub fn gravar(root: &Path, alvos: &[Alvo]) -> Result<PathBuf> {
    let caminho = root.join(".anything").join("alvos.json");
    let texto = serde_json::to_string_pretty(&serde_json::json!({ "alvos": alvos }))?;
    super::workspace::escrever_atomico(&caminho, &texto)?;
    Ok(caminho)
}

/// Lê `alvos.json` (vazio se não existe).
pub fn ler(root: &Path) -> Result<Vec<Alvo>> {
    let caminho = root.join(".anything").join("alvos.json");
    if !caminho.is_file() {
        return Ok(Vec::new());
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&caminho)?)?;
    Ok(serde_json::from_value(v["alvos"].clone())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anything-varredor-{nome}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn formats_come_from_the_file_signature_not_the_extension() {
        assert_eq!(formato(b"MZ\x90\x00"), Some("pe"));
        assert_eq!(formato(&[0x7f, b'E', b'L', b'F']), Some("elf"));
        assert_eq!(formato(&[0xcf, 0xfa, 0xed, 0xfe]), Some("macho"));
        assert_eq!(formato(b"texto"), None);
    }

    #[test]
    fn the_sweep_finds_binaries_and_skips_the_workspace_folder() {
        let d = tmp("varre");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::create_dir_all(d.join(".anything")).unwrap();
        std::fs::write(d.join("app.exe"), b"MZ0000").unwrap();
        std::fs::write(d.join("sub").join("lib.bin"), [0x7f, b'E', b'L', b'F', 0]).unwrap();
        std::fs::write(d.join("leia.txt"), b"nada").unwrap();
        std::fs::write(d.join(".anything").join("oculto.exe"), b"MZ").unwrap();
        let achados = varrer(&d).unwrap();
        assert_eq!(achados.len(), 2);
        assert!(achados.iter().all(|a| a.status == "fazer"));
        assert!(achados.iter().all(|a| !a.caminho.contains(".anything")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_list_is_saved_inside_the_project_and_read_back() {
        let d = tmp("grava");
        let alvos = vec![Alvo { caminho: "C:/p/a.exe".into(), formato: "pe".into(), status: "fazer".into() }];
        let caminho = gravar(&d, &alvos).unwrap();
        assert!(caminho.ends_with(Path::new(".anything").join("alvos.json")));
        assert_eq!(ler(&d).unwrap(), alvos);
        let _ = std::fs::remove_dir_all(&d);
    }
}
