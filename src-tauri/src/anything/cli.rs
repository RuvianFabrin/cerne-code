//! Modo Anything, fase 01: o `craft-anything-cli` (motor externo) fica numa pasta gerenciada
//! dentro do `app_data_dir`. Baixa a versão fixada só depois do clique do usuário, confere o
//! SHA-256 enquanto recebe os bytes e só então descompacta. Nada aqui roda sozinho.

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

/// Versão do motor que o Cerne fixa. Mudar aqui exige trocar a URL e o hash juntos.
pub const VERSION: &str = "0.6.1";
/// Zip portátil do Windows x64 (traz `portable.txt`, então as configurações ficam na pasta dele).
pub const WINDOWS_X64_URL: &str =
    "https://github.com/storytold/craft-anything/releases/download/v0.6.1/craft-anything-0.6.1-windows-x64-portable.zip";
/// SHA-256 desse zip, copiado do `SHA256SUMS.txt` do release v0.6.1.
pub const WINDOWS_X64_SHA256: &str = "bd370fbaa2b44dbc13581db5ffd2bf66924ed40e433c7902212aa17ca781de54";
/// Teto para o download (o zip tem poucos MB; isto só barra arquivo errado).
const MAX_ZIP_BYTES: u64 = 256 * 1024 * 1024;
/// Teto para o que a descompactação pode escrever no total.
const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;
/// Nome do executável dentro do zip.
pub const CLI_EXE: &str = "craft-anything-cli.exe";
/// Evento que a interface escuta para a barra de progresso.
pub const PROGRESS_EVENT: &str = "anything:progress";

/// Pasta do motor dentro do `app_data_dir` (`anything\`).
pub fn store_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("anything")
}

/// Pasta onde a versão fixada fica descompactada.
pub fn install_dir(store: &Path) -> PathBuf {
    store.join(format!("craft-anything-{VERSION}"))
}

/// URL e hash fixados para esta plataforma. Só Windows x64 nesta fase.
pub fn pinned_zip() -> Option<(&'static str, &'static str)> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some((WINDOWS_X64_URL, WINDOWS_X64_SHA256))
    } else {
        None
    }
}

/// O que o diagnóstico e a interface mostram sobre o motor.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CliStatus {
    pub found: bool,
    pub path: Option<String>,
    /// `gerenciado` (pasta do Cerne) ou `PATH`.
    pub origin: Option<String>,
    pub version: Option<String>,
    /// `true` quando a versão achada é a fixada.
    pub matches_pin: bool,
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    stage: &'a str,
    received: u64,
    total: Option<u64>,
}

fn emit(app: &AppHandle, stage: &str, received: u64, total: Option<u64>) {
    let _ = app.emit(PROGRESS_EVENT, Progress { stage, received, total });
}

/// Procura o executável em `dir` e, se não estiver ali, numa subpasta (o zip pode trazer uma pasta
/// de topo). Subpastas são vistas em ordem alfabética, para o resultado ser estável.
pub fn find_exe_in(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join(CLI_EXE);
    if direct.is_file() {
        return Some(direct);
    }
    let mut subs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort();
    subs.iter().map(|d| d.join(CLI_EXE)).find(|p| p.is_file())
}

/// Procura no PATH, na ordem em que o sistema procura.
pub fn find_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(CLI_EXE)).find(|p| p.is_file())
}

/// Primeiro o que está na pasta gerenciada, depois o PATH. `None` = não instalado.
pub fn find_cli(store: &Path) -> Option<(PathBuf, &'static str)> {
    if let Some(p) = find_exe_in(&install_dir(store)) {
        return Some((p, "gerenciado"));
    }
    find_on_path().map(|p| (p, "PATH"))
}

/// Roda `<exe> --version` sem abrir janela. Devolve a saída limpa, por exemplo `craft-anything-cli 0.6.1`.
pub fn cli_version(exe: &Path) -> Result<String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().with_context(|| format!("não consegui rodar {}", exe.display()))?;
    if !out.status.success() {
        bail!("{} --version terminou com erro", exe.display());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Estado atual do motor. Nunca baixa nada; só olha e, se achar, pergunta a versão.
pub fn status(store: &Path) -> CliStatus {
    let Some((exe, origin)) = find_cli(store) else {
        return CliStatus { found: false, path: None, origin: None, version: None, matches_pin: false };
    };
    let version = cli_version(&exe).ok();
    let matches_pin = version.as_deref().is_some_and(|v| v.split_whitespace().last() == Some(VERSION));
    CliStatus { found: true, path: Some(exe.display().to_string()), origin: Some(origin.to_string()), version, matches_pin }
}

/// SHA-256 de um arquivo, em hexadecimal minúsculo. (Usada pelos testes; a conferência de download é `download_verified`.)
#[allow(dead_code)]
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).with_context(|| format!("não consegui abrir {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Baixa `url` para `dest` e confere o SHA-256. Escreve num `.part` ao lado; se o hash não bater, ou
/// o tamanho passar de `max`, apaga o `.part` e falha. Só renomeia para `dest` quando tudo bate.
pub async fn download_verified(app: &AppHandle, url: &str, dest: &Path, sha256: &str, max: u64) -> Result<()> {
    let mut resp = reqwest::Client::new().get(url).send().await?.error_for_status()?;
    let total = resp.content_length();
    if total.is_some_and(|t| t > max) {
        bail!("o arquivo tem {} bytes, acima do limite de {max}", total.unwrap_or(0));
    }
    let part = dest.with_extension("part");
    let mut file = std::fs::File::create(&part).with_context(|| format!("não consegui criar {}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut got: u64 = 0;
    let mut last_report: u64 = 0;
    emit(app, "Baixando o motor", 0, total);
    let result: Result<()> = async {
        while let Some(chunk) = resp.chunk().await? {
            got += chunk.len() as u64;
            if got > max {
                bail!("o download passou do limite de {max} bytes");
            }
            hasher.update(&chunk);
            file.write_all(&chunk)?;
            if got - last_report >= 1 << 20 {
                last_report = got;
                emit(app, "Baixando o motor", got, total);
            }
        }
        file.flush()?;
        Ok(())
    }
    .await;
    drop(file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(&part);
        bail!("o hash do download não bate: esperado {sha256}, recebido {actual}");
    }
    std::fs::rename(&part, dest).with_context(|| format!("não consegui renomear para {}", dest.display()))?;
    Ok(())
}

/// Descompacta `archive` em `dest`. Recusa qualquer entrada com `..` ou caminho absoluto (o próprio
/// zip faz essa checagem em `enclosed_name`) e para se o total passar de `MAX_UNPACKED_BYTES`.
pub fn unpack_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).with_context(|| format!("não consegui abrir {}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file)?;
    std::fs::create_dir_all(dest)?;
    let mut written: u64 = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        let Some(rel) = entry.enclosed_name() else {
            bail!("entrada com caminho inseguro no zip: {name}");
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(&out).with_context(|| format!("não consegui criar {}", out.display()))?;
        written += std::io::copy(&mut entry, &mut f)?;
        if written > MAX_UNPACKED_BYTES {
            bail!("o zip descompactado passou de {MAX_UNPACKED_BYTES} bytes");
        }
    }
    Ok(())
}

/// Nome do zip que vai dentro do instalador (`resources/anything-engine/`).
pub const BUNDLED_ZIP: &str = "craft-anything-0.6.1-windows-x64-portable.zip";

/// Zip do motor que veio junto com o Cerne, se existir e tiver o hash fixado. Não vem em builds de
/// outras plataformas, nem num `tauri dev` que não conseguiu baixar na hora de preparar.
pub fn bundled_zip(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    let (_, sha) = pinned_zip()?;
    let zip = app.path().resource_dir().ok()?.join("anything-engine").join(BUNDLED_ZIP);
    (zip.is_file() && sha256_file(&zip).ok().as_deref() == Some(sha)).then_some(zip)
}

/// Instala a versão fixada. Usa o zip que vem junto com o Cerne, se houver (sem rede); senão baixa e
/// confere. Descompacta numa pasta temporária, troca pela pasta final e confere a versão. Se algo
/// falhar no meio, a pasta final não é tocada.
pub async fn install(app: &AppHandle, store: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(store)?;
    let (archive, downloaded) = match bundled_zip(app) {
        Some(zip) => {
            emit(app, "Instalando o motor que vem com o Cerne", 0, None);
            (zip, false)
        }
        None => {
            let (url, sha) = pinned_zip().ok_or_else(|| {
                anyhow!("a instalação automática só existe para Windows x64. Baixe em https://github.com/storytold/craft-anything/releases")
            })?;
            let archive = store.join(".craft-anything-cli.zip");
            download_verified(app, url, &archive, sha, MAX_ZIP_BYTES).await?;
            (archive, true)
        }
    };
    emit(app, "Preparando o motor", 0, None);
    let incoming = store.join(".incoming");
    let _ = std::fs::remove_dir_all(&incoming);
    let unpacked = unpack_zip(&archive, &incoming);
    if downloaded {
        let _ = std::fs::remove_file(&archive);
    }
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&incoming);
        return Err(e);
    }
    let dest = install_dir(store);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&incoming, &dest).with_context(|| format!("não consegui instalar em {}", dest.display()))?;
    let exe = find_exe_in(&dest).ok_or_else(|| anyhow!("o zip não traz {CLI_EXE}"))?;
    let version = cli_version(&exe)?;
    if version.split_whitespace().last() != Some(VERSION) {
        bail!("a versão instalada é '{version}', esperava {VERSION}");
    }
    emit(app, "Motor pronto", 0, None);
    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anything-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn store_and_install_paths_are_under_the_app_data_dir() {
        let base = Path::new("C:/data");
        assert_eq!(store_dir(base), Path::new("C:/data").join("anything"));
        assert!(install_dir(&store_dir(base)).ends_with(format!("craft-anything-{VERSION}")));
    }

    #[test]
    fn the_pinned_hash_is_well_formed() {
        assert_eq!(WINDOWS_X64_SHA256.len(), 64);
        assert!(WINDOWS_X64_SHA256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert!(WINDOWS_X64_URL.contains(VERSION));
    }

    #[test]
    fn find_exe_looks_in_the_folder_then_one_level_down() {
        let d = tmp("find");
        assert!(find_exe_in(&d).is_none());
        let sub = d.join("craft-anything-0.6.1-windows-x64");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(CLI_EXE), b"x").unwrap();
        assert_eq!(find_exe_in(&d), Some(sub.join(CLI_EXE)));
        std::fs::write(d.join(CLI_EXE), b"x").unwrap();
        assert_eq!(find_exe_in(&d), Some(d.join(CLI_EXE)), "a arquivo na raiz vence a subpasta");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sha256_of_a_known_file() {
        let d = tmp("sha");
        let f = d.join("abc.txt");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(sha256_file(&f).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unpack_refuses_paths_that_escape_the_folder() {
        let d = tmp("zipslip");
        let archive = d.join("evil.zip");
        {
            let file = std::fs::File::create(&archive).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("../fora.txt", opts).unwrap();
            zw.write_all(b"nope").unwrap();
            zw.finish().unwrap();
        }
        let out = d.join("out");
        assert!(unpack_zip(&archive, &out).is_err());
        assert!(!d.join("fora.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unpack_writes_a_normal_zip() {
        let d = tmp("zipok");
        let archive = d.join("ok.zip");
        {
            let file = std::fs::File::create(&archive).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zw.add_directory("pasta/", opts).unwrap();
            zw.start_file("pasta/craft-anything-cli.exe", opts).unwrap();
            zw.write_all(b"bin").unwrap();
            zw.finish().unwrap();
        }
        let out = d.join("out");
        unpack_zip(&archive, &out).unwrap();
        assert_eq!(find_exe_in(&out), Some(out.join("pasta").join(CLI_EXE)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn status_without_anything_installed_says_not_found() {
        let d = tmp("status");
        let s = status(&d.join("nada"));
        if find_on_path().is_none() {
            assert_eq!(s, CliStatus { found: false, path: None, origin: None, version: None, matches_pin: false });
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod embutido_tests {
    use super::*;

    /// O script de build (`scripts/prepare-anything-engine.mjs`) lê estes valores do JSON; se alguém
    /// trocar a versão num lado só, o instalador levaria um zip que o Cerne recusa.
    #[test]
    fn the_build_script_pins_the_same_engine_as_the_code() {
        let j: serde_json::Value = serde_json::from_str(include_str!("../../../scripts/anything-engine.json")).unwrap();
        assert_eq!(j["version"], VERSION);
        assert_eq!(j["url"], WINDOWS_X64_URL);
        assert_eq!(j["sha256"], WINDOWS_X64_SHA256);
        assert_eq!(j["file"], BUNDLED_ZIP);
    }

    /// Com o zip embutido já baixado (`npm run prepare:anything-engine`): confere o hash, descompacta e
    /// pede a versão ao motor. `cargo test --lib zip_embutido -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn zip_embutido_real() {
        let zip = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join("anything-engine").join(BUNDLED_ZIP);
        assert_eq!(sha256_file(&zip).unwrap(), WINDOWS_X64_SHA256);
        let out = std::env::temp_dir().join(format!("anything-embutido-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        unpack_zip(&zip, &out).unwrap();
        let exe = find_exe_in(&out).expect("o zip traz o executavel");
        let v = cli_version(&exe).unwrap();
        println!("versao: {v}");
        assert_eq!(v.split_whitespace().last(), Some(VERSION));
        let _ = std::fs::remove_dir_all(&out);
    }
}
