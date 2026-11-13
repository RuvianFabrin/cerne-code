//! Modo Anything: estudo em lote. Cada alvo da varredora (`alvos.json`) ganha uma pasta própria em
//! `.anything\alvos\<nº>-<nome>\`, com o próprio `tarefas.json`. O planejamento roda para cada alvo
//! nativo que está `fazer`, e o status dele vai para `planejado` ou `erro`. Estudar as fichas é da fila.

use super::plan::{self, Contexto};
use super::session::project_dir;
use super::varredor::{self, Alvo};
use super::workspace::Workspace;
use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Pasta de um alvo dentro de `.anything\alvos\`. O número garante ordem e unicidade; o nome vai
/// reduzido a caracteres seguros (letras, números, `-` e `_`).
pub fn pasta_do_alvo(root: &Path, indice: usize, caminho: &str) -> PathBuf {
    let nome = Path::new(caminho)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "alvo".into());
    let seguro: String = nome.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    root.join(".anything").join("alvos").join(format!("{:03}-{}", indice + 1, seguro))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Resultado {
    pub caminho: String,
    pub status: String,
    pub detalhe: String,
}

/// Planeja cada alvo nativo que está `fazer`. Um erro num alvo não para os outros: ele fica como
/// `erro` com o motivo. Grava o status de volta em `alvos.json`.
pub async fn planejar_lote(exe: &Path, root: &Path, goal: &str, modo: &str, teto: usize, flags: Vec<String>) -> Result<Vec<Resultado>> {
    let mut alvos = varredor::ler(root)?;
    let mut resultados = Vec::new();
    for (i, alvo) in alvos.iter_mut().enumerate() {
        if alvo.status != "fazer" || alvo.suporte != "nativo" {
            continue;
        }
        let pasta = pasta_do_alvo(root, i, &alvo.caminho);
        let ws = Workspace { root: pasta };
        let ctx = Contexto { exe, projeto: project_dir(root), alvo: &alvo.caminho, flags: flags.clone() };
        let (status, detalhe) = match plan::planejar(&ctx, &ws, modo, teto, goal).await {
            Ok(r) => ("planejado".to_string(), format!("{} tarefas no arquivo", r.tarefas_no_arquivo)),
            Err(e) => ("erro".to_string(), e.to_string()),
        };
        alvo.status = status.clone();
        resultados.push(Resultado { caminho: alvo.caminho.clone(), status, detalhe });
    }
    gravar_alvos(root, &alvos)?;
    Ok(resultados)
}

/// Regrava `alvos.json` com os status atuais (mantém o `ignorados` que já estava lá).
fn gravar_alvos(root: &Path, alvos: &[Alvo]) -> Result<()> {
    let ignorados = std::fs::read_to_string(root.join(".anything").join("alvos.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["ignorados"].as_u64())
        .unwrap_or(0) as usize;
    varredor::gravar(root, &varredor::Varredura { alvos: alvos.to_vec(), ignorados })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_target_gets_its_own_numbered_folder_with_safe_name() {
        let r = Path::new("C:/projeto");
        let p = pasta_do_alvo(r, 0, "C:/projeto/bin/meu app (v2).exe");
        assert_eq!(p, Path::new("C:/projeto").join(".anything").join("alvos").join("001-meu_app__v2_"));
        let q = pasta_do_alvo(r, 1, "C:/projeto/lib/core.dll");
        assert!(q.ends_with("002-core"));
        assert_ne!(p, q);
    }

    #[test]
    fn a_target_without_a_name_still_gets_a_folder() {
        let p = pasta_do_alvo(Path::new("C:/p"), 4, "");
        assert!(p.ends_with("005-alvo"));
    }
}

/// Teste de verdade, com o motor instalado: varre uma pasta com executáveis reais e planeja em lote.
/// `ANYTHING_TEST_STORE=<pasta> ANYTHING_TEST_ROOT=<pasta com executáveis> cargo test --lib lote_real -- --ignored --nocapture`
#[cfg(test)]
mod real {
    use super::*;

    #[tokio::test]
    #[ignore = "precisa do motor instalado e de uma pasta com executáveis"]
    async fn lote_real() {
        let store = PathBuf::from(std::env::var("ANYTHING_TEST_STORE").expect("ANYTHING_TEST_STORE"));
        let raiz = PathBuf::from(std::env::var("ANYTHING_TEST_ROOT").expect("ANYTHING_TEST_ROOT"));
        let (exe, _) = super::super::cli::find_cli(&store).expect("motor instalado");
        let v = varredor::varrer(&raiz).unwrap();
        println!("VARREDURA: {} alvos, {} ignorados", v.alvos.len(), v.ignorados);
        varredor::gravar(&raiz, &v).unwrap();
        let r = planejar_lote(&exe, &raiz, "user host name", "completo", 15, vec!["--deep".into()]).await.unwrap();
        println!("LOTE: {}", serde_json::to_string_pretty(&r).unwrap());
        let depois = varredor::ler(&raiz).unwrap();
        println!("STATUS NO JSON: {:?}", depois.iter().map(|a| (&a.caminho, &a.status)).collect::<Vec<_>>());
        assert!(depois.iter().filter(|a| a.suporte == "nativo").all(|a| a.status == "planejado" || a.status == "erro"));
        // Rodar de novo não refaz o que já foi planejado.
        let r2 = planejar_lote(&exe, &raiz, "user host name", "completo", 15, vec!["--deep".into()]).await.unwrap();
        assert!(r2.is_empty(), "nada novo para planejar na segunda vez");
    }
}
