use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Given a real project root, returns the mirrored sandbox root
/// `{parent}/{project_name}_cerne_sandbox`, sitting next to the project,
/// never inside it.
pub fn sandbox_root_for(project_root: &Path) -> Result<PathBuf> {
    let parent = project_root
        .parent()
        .ok_or_else(|| anyhow!("project root has no parent directory"))?;
    let name = project_root
        .file_name()
        .ok_or_else(|| anyhow!("project root has no file name"))?
        .to_string_lossy();
    Ok(parent.join(format!("{name}_cerne_sandbox")))
}

/// Maps a path to its sandbox mirror path. Caminhos sob `project_root`
/// mantêm a estrutura relativa de sempre; caminhos externos (permitidos nas
/// ferramentas de escrita em modo Auto/YOLO — ver `resolve_path` em
/// `agent/tools.rs`) caem num subdiretório `_external`, espelhados por uma
/// versão sanitizada do caminho absoluto (preserva a estrutura de pastas
/// pra facilitar debug, sem colidir entre unidades/pastas diferentes).
pub fn to_sandbox_path(project_root: &Path, target: &Path) -> Result<PathBuf> {
    let root = sandbox_root_for(project_root)?;
    if let Ok(rel) = target.strip_prefix(project_root) {
        return Ok(root.join(rel));
    }
    Ok(root.join("_external").join(sanitize_external_path(target)))
}

fn sanitize_external_path(target: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in target.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                let name = prefix
                    .as_os_str()
                    .to_string_lossy()
                    .replace([':', '\\', '/'], "_");
                out.push(format!("{name}_drive"));
            }
            std::path::Component::Normal(part) => out.push(part),
            std::path::Component::RootDir
            | std::path::Component::CurDir
            | std::path::Component::ParentDir => {}
        }
    }
    out
}

/// Conteudo "atual" de um arquivo pra fins de EDICAO (nao de exibicao de
/// diff): prefere a versao ja em stage na sandbox, se ja existir uma edicao
/// anterior ainda nao aceita pro mesmo arquivo, em vez do arquivo real.
///
/// **Bug real encontrado testando sub-agente ao vivo**: pedi pra adicionar
/// docstring em 3 funcoes do mesmo arquivo; cada `edit_file` lia o arquivo
/// REAL (que nunca muda ate o aceite), entao as 3 edicoes partiam todas do
/// mesmo original, cada uma so com sua propria docstring — aceitar a
/// terceira sobrescrevia (perdia) as duas primeiras em vez de acumular. So
/// sobrou a docstring de `multiply` no arquivo final, apesar da UI mostrar
/// "3 edicoes aceitas" sem erro nenhum — perda silenciosa de duas das tres
/// edicoes. Ler a partir da sandbox (quando ja existe) faz cada edicao nova
/// nascer em cima da anterior, entao aceitar a ultima leva tudo.
pub fn read_current_content(project_root: &Path, target: &Path) -> Result<String> {
    let sandbox_path = to_sandbox_path(project_root, target)?;
    let path_to_read = if sandbox_path.exists() {
        sandbox_path.as_path()
    } else {
        target
    };
    match std::fs::read(path_to_read) {
        Ok(bytes) => Ok(crate::encoding::decode(&bytes).0),
        Err(e) => Err(anyhow!(
            "nao foi possivel ler {}: {e}",
            path_to_read.display()
        )),
    }
}

/// Escreve `new_content` na sandbox preservando a codificacao original do
/// arquivo real (UTF-8/UTF-16LE/UTF-16BE/Windows-1252, com ou sem BOM) — ver
/// `crate::encoding`. Reescrever sempre como UTF-8 corromperia silenciosamente
/// um arquivo UTF-16 ou Windows-1252/ISO real (comum em projeto gerado no
/// Windows). Arquivo novo (ainda nao existe) usa UTF-8 sem BOM, o default
/// razoavel quando nao ha um "original" de onde herdar.
pub fn write_sandboxed(
    project_root: &Path,
    target: &Path,
    new_content: &str,
) -> Result<(String, bool)> {
    let sandbox_path = to_sandbox_path(project_root, target)?;
    if let Some(parent) = sandbox_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let is_new_file = !target.exists();
    let (original, file_encoding) = match std::fs::read(target) {
        Ok(bytes) => crate::encoding::decode(&bytes),
        Err(_) => (String::new(), crate::encoding::FileEncoding::UTF8_NO_BOM),
    };

    let encoded = crate::encoding::encode(new_content, file_encoding);
    std::fs::write(&sandbox_path, &encoded)?;

    let diff = similar::TextDiff::from_lines(original.as_str(), new_content)
        .unified_diff()
        .context_radius(3)
        .header(&target.to_string_lossy(), &target.to_string_lossy())
        .to_string();

    Ok((diff, is_new_file))
}

/// Escreve `new_content` DIRETO no arquivo real (sem sandbox), preservando
/// a codificacao original. Usado no modo YOLO. Retorna (diff, is_new_file).
pub fn write_direct(target: &Path, new_content: &str) -> Result<(String, bool)> {
    let is_new_file = !target.exists();
    let (original, file_encoding) = match std::fs::read(target) {
        Ok(bytes) => crate::encoding::decode(&bytes),
        Err(_) => (String::new(), crate::encoding::FileEncoding::UTF8_NO_BOM),
    };

    let diff = similar::TextDiff::from_lines(original.as_str(), new_content)
        .unified_diff()
        .context_radius(3)
        .header(&target.to_string_lossy(), &target.to_string_lossy())
        .to_string();

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = crate::encoding::encode(new_content, file_encoding);
    std::fs::write(target, &encoded)?;

    Ok((diff, is_new_file))
}

pub fn accept_edit(sandbox_path: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(sandbox_path, target)?;
    Ok(())
}

pub fn reject_edit(sandbox_path: &Path) -> Result<()> {
    if sandbox_path.exists() {
        std::fs::remove_file(sandbox_path)?;
    }
    Ok(())
}
