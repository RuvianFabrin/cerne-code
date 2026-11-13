//! Varredora de pasta: percorre um diretório do projeto e classifica cada arquivo pela assinatura
//! (os primeiros bytes), não pela extensão, porque a extensão engana (DLL é PE, APK é zip, etc.).
//! Grava `.anything/alvos.json` com o caminho completo, o formato, se o plano atual já suporta e o
//! status. Arquivo que não reconhece fica só contado em `ignorados`.
//! Não decompila: só monta a lista.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Pastas que a varredora não entra.
const IGNORADAS: &[&str] = &[".anything", ".git", "node_modules", "target"];
/// Profundidade máxima da varredura.
const PROFUNDIDADE_MAXIMA: usize = 8;
/// Quantas entradas de um zip são lidas para decidir se é APK ou JAR.
const ENTRADAS_ZIP_LIDAS: usize = 5000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Alvo {
    pub caminho: String,
    pub formato: String,
    /// `nativo` (o plano atual já estuda) ou a fase em que o formato entra.
    pub suporte: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Varredura {
    pub alvos: Vec<Alvo>,
    pub ignorados: usize,
}

/// Formato pela assinatura do arquivo. `None` = não é um formato que o motor conhece.
pub fn classificar(caminho: &Path) -> Option<&'static str> {
    let mut cab = [0u8; 16];
    let n = std::fs::File::open(caminho).ok()?.read(&mut cab).ok()?;
    let c = &cab[..n];
    if c.starts_with(b"MZ") {
        return Some("pe");
    }
    if c.starts_with(&[0x7f, b'E', b'L', b'F']) {
        return Some("elf");
    }
    let macho: [[u8; 4]; 4] = [[0xfe, 0xed, 0xfa, 0xce], [0xfe, 0xed, 0xfa, 0xcf], [0xce, 0xfa, 0xed, 0xfe], [0xcf, 0xfa, 0xed, 0xfe]];
    if macho.iter().any(|m| c.starts_with(m)) {
        return Some("macho");
    }
    if c.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        // Mesmo começo para .class do Java e para Mach-O "fat". O que muda é o número que vem depois:
        // no .class é a versão maior (45 a 70); no fat é a quantidade de arquiteturas (poucas).
        if n >= 8 {
            let maior = u16::from_be_bytes([c[6], c[7]]);
            if (45..=70).contains(&maior) {
                return Some("java-class");
            }
        }
        return Some("macho-fat");
    }
    if c.starts_with(b"PK\x03\x04") {
        return Some(classificar_zip(caminho));
    }
    if c.starts_with(b"dex\n") {
        return Some("dex");
    }
    if c.starts_with(b"\0asm") {
        return Some("wasm");
    }
    if c.starts_with(&[0xd4, 0xc3, 0xb2, 0xa1]) || c.starts_with(&[0xa1, 0xb2, 0xc3, 0xd4]) {
        return Some("pcap");
    }
    if c.starts_with(&[0x0a, 0x0d, 0x0d, 0x0a]) {
        return Some("pcapng");
    }
    if c.starts_with(b"SQLite format 3\0") {
        return Some("sqlite");
    }
    if c.starts_with(&[b'7', b'z', 0xbc, 0xaf, 0x27, 0x1c]) {
        return Some("7z");
    }
    // Sem assinatura conhecida: a extensão só decide o texto de web e o pacote do Electron.
    match caminho.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("asar") => Some("asar"),
        Some("js") | Some("mjs") | Some("html") | Some("htm") => Some("web"),
        _ => None,
    }
}

/// Zip: APK tem `AndroidManifest.xml`; JAR tem `META-INF/MANIFEST.MF`; senão, zip comum.
fn classificar_zip(caminho: &Path) -> &'static str {
    let Ok(f) = std::fs::File::open(caminho) else { return "zip" };
    let Ok(mut zip) = zip::ZipArchive::new(f) else { return "zip" };
    let mut apk = false;
    let mut jar = false;
    for i in 0..zip.len().min(ENTRADAS_ZIP_LIDAS) {
        if let Ok(e) = zip.by_index(i) {
            match e.name() {
                "AndroidManifest.xml" => apk = true,
                "META-INF/MANIFEST.MF" => jar = true,
                _ => {}
            }
        }
    }
    if apk {
        "apk"
    } else if jar {
        "jar"
    } else {
        "zip"
    }
}

/// O que o plano atual já estuda. Só o nativo: o resto entra na fase 12.
pub fn suporte(formato: &str) -> &'static str {
    match formato {
        "pe" | "elf" | "macho" | "macho-fat" => "nativo",
        "java-class" | "jar" => "java (fase 12)",
        "apk" | "dex" => "android (fase 12)",
        "web" => "web (fase 12)",
        "asar" => "electron (fase 12)",
        "wasm" | "pcap" | "pcapng" | "sqlite" | "zip" | "7z" => "outro (fase 12)",
        _ => "não suportado",
    }
}

/// Percorre `dir` e classifica tudo. Ordem de caminho (estável).
pub fn varrer(dir: &Path) -> Result<Varredura> {
    let mut alvos = Vec::new();
    let mut ignorados = 0usize;
    recolher(dir, 0, &mut alvos, &mut ignorados)?;
    alvos.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    Ok(Varredura { alvos, ignorados })
}

fn recolher(dir: &Path, profundidade: usize, alvos: &mut Vec<Alvo>, ignorados: &mut usize) -> Result<()> {
    if profundidade > PROFUNDIDADE_MAXIMA {
        return Ok(());
    }
    for entrada in std::fs::read_dir(dir)?.flatten() {
        let caminho: PathBuf = entrada.path();
        let nome = entrada.file_name().to_string_lossy().to_string();
        if caminho.is_dir() {
            if !IGNORADAS.contains(&nome.as_str()) {
                recolher(&caminho, profundidade + 1, alvos, ignorados)?;
            }
        } else if caminho.is_file() {
            match classificar(&caminho) {
                Some(f) => alvos.push(Alvo {
                    caminho: caminho.display().to_string(),
                    formato: f.to_string(),
                    suporte: suporte(f).to_string(),
                    status: "fazer".into(),
                }),
                None => *ignorados += 1,
            }
        }
    }
    Ok(())
}

/// Fontes da varredura: pastas e arquivos que o usuário adicionou. Ficam em `.anything/fontes.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fonte {
    pub caminho: String,
    /// `pasta` ou `arquivo`.
    pub tipo: String,
}

fn arquivo_de_fontes(root: &Path) -> PathBuf {
    root.join(".anything").join("fontes.json")
}

/// Lê as fontes (vazio se o arquivo não existe).
pub fn ler_fontes(root: &Path) -> Result<Vec<Fonte>> {
    let caminho = arquivo_de_fontes(root);
    if !caminho.is_file() {
        return Ok(Vec::new());
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&caminho)?)?;
    Ok(serde_json::from_value(v["fontes"].clone()).unwrap_or_default())
}

fn gravar_fontes(root: &Path, fontes: &[Fonte]) -> Result<()> {
    let texto = serde_json::to_string_pretty(&serde_json::json!({ "fontes": fontes }))?;
    super::workspace::escrever_atomico(&arquivo_de_fontes(root), &texto)
}

/// Adiciona uma pasta ou um arquivo à lista. O tipo vem do disco; caminho que não existe é recusado.
/// Adicionar o que já está na lista não duplica.
pub fn adicionar_fonte(root: &Path, caminho: &str) -> Result<Vec<Fonte>> {
    let p = Path::new(caminho);
    let tipo = if p.is_dir() {
        "pasta"
    } else if p.is_file() {
        "arquivo"
    } else {
        anyhow::bail!("'{caminho}' não existe");
    };
    let mut fontes = ler_fontes(root)?;
    if !fontes.iter().any(|f| f.caminho == caminho) {
        fontes.push(Fonte { caminho: caminho.to_string(), tipo: tipo.into() });
        gravar_fontes(root, &fontes)?;
    }
    Ok(fontes)
}

/// Tira uma fonte da lista e os alvos que só ela cobria. O que já foi estudado em
/// `.anything/alvos/` não é apagado.
pub fn remover_fonte(root: &Path, caminho: &str) -> Result<Vec<Fonte>> {
    let mut fontes = ler_fontes(root)?;
    fontes.retain(|f| f.caminho != caminho);
    gravar_fontes(root, &fontes)?;
    let alvos: Vec<Alvo> = ler(root)?.into_iter().filter(|a| coberto(&fontes, &a.caminho)).collect();
    let ignorados = ler_ignorados(root);
    gravar(root, &Varredura { alvos, ignorados })?;
    Ok(fontes)
}

fn coberto(fontes: &[Fonte], caminho: &str) -> bool {
    let c = Path::new(caminho);
    fontes.iter().any(|f| if f.tipo == "pasta" { c.starts_with(&f.caminho) } else { c == Path::new(&f.caminho) })
}

fn ler_ignorados(root: &Path) -> usize {
    std::fs::read_to_string(root.join(".anything").join("alvos.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["ignorados"].as_u64())
        .unwrap_or(0) as usize
}

/// Varre todas as fontes (a pasta do projeto se a lista está vazia) e junta com o que já havia em
/// `alvos.json`: quem já estava na lista mantém o status (não volta a `fazer`), quem sumiu do disco sai.
pub fn varrer_fontes(root: &Path) -> Result<Varredura> {
    let mut fontes = ler_fontes(root)?;
    if fontes.is_empty() {
        fontes.push(Fonte { caminho: root.display().to_string(), tipo: "pasta".into() });
    }
    let mut alvos: Vec<Alvo> = Vec::new();
    let mut ignorados = 0usize;
    for f in &fontes {
        let p = Path::new(&f.caminho);
        if f.tipo == "pasta" {
            if p.is_dir() {
                let v = varrer(p)?;
                ignorados += v.ignorados;
                alvos.extend(v.alvos);
            }
        } else if p.is_file() {
            // Arquivo escolhido à mão entra mesmo sem assinatura conhecida: quem decide é o usuário.
            let formato = classificar(p).unwrap_or("desconhecido");
            alvos.push(Alvo { caminho: p.display().to_string(), formato: formato.into(), suporte: suporte(formato).into(), status: "fazer".into() });
        }
    }
    alvos.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    alvos.dedup_by(|a, b| a.caminho == b.caminho);
    let antigos = ler(root).unwrap_or_default();
    for a in alvos.iter_mut() {
        if let Some(velho) = antigos.iter().find(|o| o.caminho == a.caminho) {
            a.status = velho.status.clone();
        }
    }
    let v = Varredura { alvos, ignorados };
    gravar(root, &v)?;
    Ok(v)
}

/// Grava `alvos.json` na pasta `.anything` do projeto (escrita atômica).
pub fn gravar(root: &Path, varredura: &Varredura) -> Result<PathBuf> {
    let caminho = root.join(".anything").join("alvos.json");
    let texto = serde_json::to_string_pretty(&serde_json::json!({ "alvos": varredura.alvos, "ignorados": varredura.ignorados }))?;
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

    fn zip_com(caminho: &Path, nomes: &[&str]) {
        use std::io::Write;
        let f = std::fs::File::create(caminho).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        for n in nomes {
            zw.start_file(*n, opts).unwrap();
            zw.write_all(b"x").unwrap();
        }
        zw.finish().unwrap();
    }

    #[test]
    fn dll_is_pe_whatever_the_extension() {
        let d = tmp("dll");
        let p = d.join("biblioteca.dll");
        std::fs::write(&p, b"MZ\x90\x00").unwrap();
        assert_eq!(classificar(&p), Some("pe"));
        assert_eq!(suporte("pe"), "nativo");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn java_class_and_fat_macho_share_a_magic_but_are_told_apart() {
        let d = tmp("cafe");
        let classe = d.join("Main.class");
        std::fs::write(&classe, [0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52]).unwrap();
        let fat = d.join("universal");
        std::fs::write(&fat, [0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2]).unwrap();
        assert_eq!(classificar(&classe), Some("java-class"));
        assert_eq!(classificar(&fat), Some("macho-fat"));
        assert_eq!(suporte("java-class"), "java (fase 12)");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn apk_jar_and_plain_zip_are_separated_by_their_entries() {
        let d = tmp("zip");
        let apk = d.join("app.apk");
        let jar = d.join("lib.jar");
        let zip = d.join("dados.zip");
        zip_com(&apk, &["AndroidManifest.xml", "classes.dex"]);
        zip_com(&jar, &["META-INF/MANIFEST.MF", "A.class"]);
        zip_com(&zip, &["texto.txt"]);
        assert_eq!(classificar(&apk), Some("apk"));
        assert_eq!(classificar(&jar), Some("jar"));
        assert_eq!(classificar(&zip), Some("zip"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn other_systems_formats_are_recognised() {
        let d = tmp("outros");
        let casos: Vec<(&str, Vec<u8>, &str)> = vec![
            ("classes.dex", b"dex\n035\0".to_vec(), "dex"),
            ("modulo.wasm", b"\0asm\x01\0\0\0".to_vec(), "wasm"),
            ("captura.pcap", vec![0xd4, 0xc3, 0xb2, 0xa1, 0, 0], "pcap"),
            ("banco.db", b"SQLite format 3\0".to_vec(), "sqlite"),
            ("app.asar", b"\x04\0\0\0".to_vec(), "asar"),
        ];
        for (nome, bytes, esperado) in casos {
            let p = d.join(nome);
            std::fs::write(&p, &bytes).unwrap();
            assert_eq!(classificar(&p), Some(esperado), "{nome}");
        }
        let web = d.join("index.html");
        std::fs::write(&web, b"<!doctype html>").unwrap();
        assert_eq!(classificar(&web), Some("web"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unknown_files_are_counted_not_listed() {
        let d = tmp("varre");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::create_dir_all(d.join(".anything")).unwrap();
        std::fs::write(d.join("app.exe"), b"MZ0000").unwrap();
        std::fs::write(d.join("sub").join("lib.so"), [0x7f, b'E', b'L', b'F', 0]).unwrap();
        std::fs::write(d.join("leia.txt"), b"nada").unwrap();
        std::fs::write(d.join(".anything").join("oculto.exe"), b"MZ").unwrap();
        let v = varrer(&d).unwrap();
        assert_eq!(v.alvos.len(), 2);
        assert_eq!(v.ignorados, 1, "o txt conta como ignorado; a pasta .anything nem entra");
        assert!(v.alvos.iter().all(|a| a.status == "fazer"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sources_are_added_removed_and_scanned_keeping_status() {
        let raiz = tmp("fontes");
        let pasta = tmp("pasta-extra");
        std::fs::write(pasta.join("a.dll"), b"MZ00").unwrap();
        std::fs::write(pasta.join("nota.txt"), b"x").unwrap();
        let solto = tmp("solto").join("b.exe");
        std::fs::write(&solto, b"MZ00").unwrap();
        let desconhecido = solto.with_file_name("dados.bin");
        std::fs::write(&desconhecido, b"zzzz").unwrap();

        assert!(adicionar_fonte(&raiz, "Z:/nao/existe").is_err());
        adicionar_fonte(&raiz, &pasta.display().to_string()).unwrap();
        adicionar_fonte(&raiz, &solto.display().to_string()).unwrap();
        let fontes = adicionar_fonte(&raiz, &solto.display().to_string()).unwrap();
        assert_eq!(fontes.len(), 2, "repetida não duplica");
        assert_eq!(fontes[0].tipo, "pasta");
        assert_eq!(fontes[1].tipo, "arquivo");

        let v = varrer_fontes(&raiz).unwrap();
        assert_eq!(v.alvos.len(), 2);
        // Marca um como feito e varre de novo: o status fica.
        let mut alvos = ler(&raiz).unwrap();
        alvos[0].status = "feito".into();
        gravar(&raiz, &Varredura { alvos, ignorados: v.ignorados }).unwrap();
        adicionar_fonte(&raiz, &desconhecido.display().to_string()).unwrap();
        let v2 = varrer_fontes(&raiz).unwrap();
        assert_eq!(v2.alvos.len(), 3, "arquivo escolhido à mão entra mesmo sem assinatura");
        assert!(v2.alvos.iter().any(|a| a.formato == "desconhecido"));
        assert_eq!(v2.alvos.iter().filter(|a| a.status == "feito").count(), 1);

        // Remover a pasta tira os alvos dela e deixa os outros.
        remover_fonte(&raiz, &pasta.display().to_string()).unwrap();
        let resto = ler(&raiz).unwrap();
        assert_eq!(resto.len(), 2);
        assert!(resto.iter().all(|a| !a.caminho.ends_with("a.dll")));
        assert_eq!(ler_fontes(&raiz).unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn without_sources_the_project_folder_is_scanned() {
        let raiz = tmp("padrao");
        std::fs::write(raiz.join("x.exe"), b"MZ00").unwrap();
        assert_eq!(varrer_fontes(&raiz).unwrap().alvos.len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn the_list_is_saved_inside_the_project_and_read_back() {
        let d = tmp("grava");
        let v = Varredura {
            alvos: vec![Alvo { caminho: "C:/p/a.exe".into(), formato: "pe".into(), suporte: "nativo".into(), status: "fazer".into() }],
            ignorados: 3,
        };
        let caminho = gravar(&d, &v).unwrap();
        assert!(caminho.ends_with(Path::new(".anything").join("alvos.json")));
        assert_eq!(ler(&d).unwrap(), v.alvos);
        let _ = std::fs::remove_dir_all(&d);
    }
}
