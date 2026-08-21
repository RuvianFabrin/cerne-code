use crate::models::Folder;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

fn folders_file(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("folders.json")
}

pub fn list_folders(app_data_dir: &Path) -> Result<Vec<Folder>> {
    let file = folders_file(app_data_dir);
    if !file.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&file)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&text)?)
}

fn save_folders(app_data_dir: &Path, folders: &[Folder]) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    std::fs::write(
        folders_file(app_data_dir),
        serde_json::to_string_pretty(folders)?,
    )?;
    Ok(())
}

/// Usado pelo backup completo (zip/git): soma pastas vindas de uma
/// importação às pastas locais, SEM sobrescrever — pasta cujo id já existe
/// localmente é pulada (mesmo espírito do "nunca sobrescreve" já usado pra
/// sessão importada). Devolve os ids das pastas realmente adicionadas.
pub fn merge_folders(app_data_dir: &Path, incoming: Vec<Folder>) -> Result<Vec<String>> {
    let mut folders = list_folders(app_data_dir)?;
    let existing_ids: std::collections::HashSet<String> =
        folders.iter().map(|f| f.id.clone()).collect();
    let mut added = Vec::new();
    for folder in incoming {
        if existing_ids.contains(&folder.id) {
            continue;
        }
        added.push(folder.id.clone());
        folders.push(folder);
    }
    if !added.is_empty() {
        save_folders(app_data_dir, &folders)?;
    }
    Ok(added)
}

/// Recusa criar uma subpasta de uma subpasta — só 2 níveis (pasta de raiz →
/// sessões e subpastas; subpasta → só sessões).
pub fn create_folder(app_data_dir: &Path, name: &str, parent_id: Option<String>) -> Result<Folder> {
    let mut folders = list_folders(app_data_dir)?;
    if let Some(ref parent_id) = parent_id {
        let parent = folders
            .iter()
            .find(|f| &f.id == parent_id)
            .ok_or_else(|| anyhow!("pasta pai '{parent_id}' nao encontrada"))?;
        if parent.parent_id.is_some() {
            return Err(anyhow!(
                "nao e possivel criar subpasta dentro de uma subpasta (maximo 2 niveis)"
            ));
        }
    }
    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        parent_id,
    };
    folders.push(folder.clone());
    save_folders(app_data_dir, &folders)?;
    Ok(folder)
}

pub fn rename_folder(app_data_dir: &Path, id: &str, name: &str) -> Result<Folder> {
    let mut folders = list_folders(app_data_dir)?;
    let folder = folders
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(|| anyhow!("pasta '{id}' nao encontrada"))?;
    folder.name = name.trim().to_string();
    let updated = folder.clone();
    save_folders(app_data_dir, &folders)?;
    Ok(updated)
}

/// Apaga a pasta. Subpastas e sessões que estavam nela são movidas pra raiz
/// automaticamente (não precisa de confirmação extra — é reversível
/// arrastando/movendo de volta pelo menu "Mover para").
pub fn delete_folder(app_data_dir: &Path, id: &str) -> Result<Vec<String>> {
    let mut folders = list_folders(app_data_dir)?;
    let before = folders.len();
    folders.retain(|f| f.id != id);
    if folders.len() == before {
        return Err(anyhow!("pasta '{id}' nao encontrada"));
    }
    // Subpastas orfas (parent_id apontava pra essa pasta) sobem pra raiz.
    let mut orphaned_subfolder_ids = Vec::new();
    for f in folders.iter_mut() {
        if f.parent_id.as_deref() == Some(id) {
            f.parent_id = None;
            orphaned_subfolder_ids.push(f.id.clone());
        }
    }
    save_folders(app_data_dir, &folders)?;
    Ok(orphaned_subfolder_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-folders-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_then_list_roundtrip() {
        let dir = scratch_dir();
        let f = create_folder(&dir, "Trabalho", None).unwrap();
        let found = list_folders(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, f.id);
        assert_eq!(found[0].name, "Trabalho");
        assert!(found[0].parent_id.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_subfolder_under_root_folder_works() {
        let dir = scratch_dir();
        let root = create_folder(&dir, "Trabalho", None).unwrap();
        let sub = create_folder(&dir, "Cliente X", Some(root.id.clone())).unwrap();
        assert_eq!(sub.parent_id, Some(root.id));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_subfolder_under_subfolder_is_rejected() {
        let dir = scratch_dir();
        let root = create_folder(&dir, "Trabalho", None).unwrap();
        let sub = create_folder(&dir, "Cliente X", Some(root.id)).unwrap();
        let result = create_folder(&dir, "Nivel 3", Some(sub.id));
        assert!(result.is_err(), "nao deveria permitir 3 niveis de pasta");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_updates_name() {
        let dir = scratch_dir();
        let f = create_folder(&dir, "Antigo", None).unwrap();
        let renamed = rename_folder(&dir, &f.id, "Novo nome").unwrap();
        assert_eq!(renamed.name, "Novo nome");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_moves_subfolders_to_root() {
        let dir = scratch_dir();
        let root = create_folder(&dir, "Trabalho", None).unwrap();
        let sub = create_folder(&dir, "Cliente X", Some(root.id.clone())).unwrap();
        let orphaned = delete_folder(&dir, &root.id).unwrap();
        assert_eq!(orphaned, vec![sub.id.clone()]);
        let found = list_folders(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, sub.id);
        assert!(found[0].parent_id.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_missing_folder_errors() {
        let dir = scratch_dir();
        assert!(delete_folder(&dir, "nao-existe").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
