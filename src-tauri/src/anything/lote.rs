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

/// Pasta de um alvo dentro de `.anything\alvos\`. O prefixo é um hash do caminho completo (não a
/// posição na lista), então adicionar ou remover fontes não troca a pasta de quem já foi estudado.
/// O nome vai reduzido a caracteres seguros (letras, números, `-` e `_`).
pub fn pasta_do_alvo(root: &Path, caminho: &str) -> PathBuf {
    // FNV-1a de 32 bits: simples e estável entre execuções (o hasher padrão do Rust não é).
    let mut h: u32 = 0x811c_9dc5;
    for b in caminho.to_lowercase().replace('\\', "/").bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    let nome = Path::new(caminho)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "alvo".into());
    let seguro: String = nome.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    root.join(".anything").join("alvos").join(format!("{:08x}-{}", h, seguro))
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
    for alvo in alvos.iter_mut() {
        if alvo.status != "fazer" || alvo.suporte != "nativo" {
            continue;
        }
        let pasta = pasta_do_alvo(root, &alvo.caminho);
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

/// Primeiro alvo com status `planejado`: o alvo e a pasta dele em `.anything/alvos/`.
pub fn proximo_planejado(root: &Path) -> Option<(Alvo, PathBuf)> {
    let alvos = varredor::ler(root).ok()?;
    alvos.into_iter().find(|a| a.status == "planejado").map(|a| {
        let pasta = pasta_do_alvo(root, &a.caminho);
        (a, pasta)
    })
}

/// Marca como `feito` o alvo cuja pasta é `ws_dir`. Devolve `true` só se havia um alvo assim e ainda
/// existe outro `planejado` para a fila seguir; `false` em qualquer outro caso (a fila termina).
pub fn concluir_alvo(root: &Path, ws_dir: &Path) -> bool {
    let Ok(mut alvos) = varredor::ler(root) else { return false };
    let Some(i) = alvos.iter().position(|a| pasta_do_alvo(root, &a.caminho) == ws_dir) else { return false };
    if alvos[i].status != "planejado" {
        return false;
    }
    alvos[i].status = "feito".into();
    if gravar_alvos(root, &alvos).is_err() {
        return false;
    }
    alvos.iter().any(|a| a.status == "planejado")
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
    fn each_target_gets_its_own_stable_folder_with_safe_name() {
        let r = Path::new("C:/projeto");
        let p = pasta_do_alvo(r, "C:/projeto/bin/meu app (v2).exe");
        let nome = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(nome.ends_with("-meu_app__v2_"), "{nome}");
        assert_eq!(nome.len(), 8 + 1 + "meu_app__v2_".len());
        let q = pasta_do_alvo(r, "C:/projeto/lib/core.dll");
        assert!(q.ends_with(format!("{}-core", &q.file_name().unwrap().to_string_lossy()[..8])));
        assert_ne!(p, q);
        // Mesma entrada, mesma pasta; barra e maiúscula não mudam o resultado.
        assert_eq!(p, pasta_do_alvo(r, "c:\\projeto\\bin\\meu app (v2).exe"));
        // Mesmo nome de arquivo em pastas diferentes não colide.
        assert_ne!(pasta_do_alvo(r, "C:/a/x.dll"), pasta_do_alvo(r, "C:/b/x.dll"));
    }

    #[test]
    fn a_target_without_a_name_still_gets_a_folder() {
        let p = pasta_do_alvo(Path::new("C:/p"), "");
        assert!(p.to_string_lossy().ends_with("-alvo"));
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

#[cfg(test)]
mod avanco_tests {
    use super::*;

    fn projeto_com_dois_alvos() -> PathBuf {
        let d = std::env::temp_dir().join(format!("anything-lote-avanco-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(d.join(".anything")).unwrap();
        let alvos = vec![
            Alvo { caminho: "C:/p/a.exe".into(), formato: "pe".into(), suporte: "nativo".into(), status: "planejado".into() },
            Alvo { caminho: "C:/p/b.dll".into(), formato: "pe".into(), suporte: "nativo".into(), status: "planejado".into() },
        ];
        varredor::gravar(&d, &varredor::Varredura { alvos, ignorados: 0 }).unwrap();
        d
    }

    #[test]
    fn the_queue_goes_to_the_first_planned_target_then_the_next_then_ends() {
        let d = projeto_com_dois_alvos();
        let (a, pasta) = proximo_planejado(&d).unwrap();
        assert_eq!(a.caminho.as_str(), "C:/p/a.exe");
        // Terminou o primeiro: há outro planejado, a fila segue.
        assert!(concluir_alvo(&d, &pasta));
        let (a2, pasta2) = proximo_planejado(&d).unwrap();
        assert_eq!(a2.caminho.as_str(), "C:/p/b.dll");
        // Terminou o último: nada mais planejado, a fila termina.
        assert!(!concluir_alvo(&d, &pasta2));
        assert!(proximo_planejado(&d).is_none());
        let status: Vec<String> = varredor::ler(&d).unwrap().into_iter().map(|a| a.status).collect();
        assert_eq!(status, ["feito", "feito"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn finishing_a_folder_that_is_not_a_target_changes_nothing() {
        let d = projeto_com_dois_alvos();
        assert!(!concluir_alvo(&d, &d.join(".anything")), "a pasta da raiz não é de nenhum alvo");
        assert_eq!(varredor::ler(&d).unwrap().iter().filter(|a| a.status == "planejado").count(), 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_target_already_done_is_not_marked_again() {
        let d = projeto_com_dois_alvos();
        let (_, pasta) = proximo_planejado(&d).unwrap();
        assert!(concluir_alvo(&d, &pasta));
        assert!(!concluir_alvo(&d, &pasta), "já está feito");
        let _ = std::fs::remove_dir_all(&d);
    }
}
