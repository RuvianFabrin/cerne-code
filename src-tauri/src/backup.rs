// Item da lista "Pra implementar": backup de
// sessão como `.zip` — exportar uma sessão específica ou todas, e reimportar depois. Cada sessão
// no disco é só 3 arquivos JSON pequenos (`session.json`, `chat_log.json`, `tasks.json`, dentro de
// `<app_data_dir>/sessions/<id>/`) — sem anexos binários dentro da pasta da sessão (attachments são
// lidos de onde o usuário escolheu no sistema, nunca copiados pra dentro), então o zip fica simples.
//
// Decisões já registradas em 14_backlog_pendente.md antes de codar (seguidas aqui):
// - Sessão importada SEMPRE ganha um UUID NOVO — nunca reusa o id do zip (evita colisão e permite
//   reimportar o mesmo zip de novo pra ter uma cópia, em vez de travar achando que já existe).
// - `parent_session_id` (Fase G) é sempre limpo na importação — a sessão-mãe quase certamente não
//   existe na máquina de destino.
// - `folder_id`: só é preservado quando o zip é um BACKUP COMPLETO (todas as sessões, exportado
//   junto de `folders.json` — os ids batem porque foram exportados do mesmo estado). Exportação de
//   UMA sessão não carrega `folders.json`, então o `folder_id` apontaria pra uma pasta que não
//   existe no destino — é limpo (sessão importada cai na raiz) em vez de deixar uma referência
//   pendurada. Um manifesto (`cerne_backup_manifest.json`) marca qual dos dois casos é.
//
// Cuidado central, como o item já registrava: zip-slip na importação. Mitigado usando
// `ZipFile::enclosed_name()` (API da própria crate `zip` pensada pra isso — devolve `None` pra
// qualquer entrada com `..`/caminho absoluto) em vez de tentar validar a string manualmente, mais
// uma checagem redundante de `starts_with` no caminho final resolvido.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const SESSION_FILES: [&str; 3] = ["session.json", "chat_log.json", "tasks.json"];
const MANIFEST_NAME: &str = "cerne_backup_manifest.json";
const FOLDERS_NAME: &str = "folders.json";

fn sessions_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("sessions")
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    full_backup: bool,
}

/// Exporta as sessões pedidas (todas, se `session_ids` vier vazio — nesse
/// caso conta como "backup completo" e também leva `folders.json`) pra um
/// `.zip` no caminho de destino. Devolve quantas sessões foram exportadas.
pub fn export_sessions_zip(
    app_data_dir: &Path,
    session_ids: &[String],
    dest_path: &Path,
) -> Result<usize> {
    let sessions_root = sessions_dir(app_data_dir);
    let full_backup = session_ids.is_empty();
    let ids: Vec<String> = if full_backup {
        std::fs::read_dir(&sessions_root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect()
    } else {
        session_ids.to_vec()
    };

    let file = std::fs::File::create(dest_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    let manifest = Manifest {
        version: 1,
        full_backup,
    };
    zip.start_file(MANIFEST_NAME, opts)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    let mut exported = 0;
    for id in &ids {
        let dir = sessions_root.join(id);
        if !dir.is_dir() {
            continue;
        }
        for filename in SESSION_FILES {
            let path = dir.join(filename);
            if !path.is_file() {
                continue;
            }
            let content = std::fs::read(&path)?;
            zip.start_file(format!("sessions/{id}/{filename}"), opts)?;
            zip.write_all(&content)?;
        }
        exported += 1;
    }

    if full_backup {
        if let Ok(folders) = crate::folders::list_folders(app_data_dir) {
            if !folders.is_empty() {
                zip.start_file(FOLDERS_NAME, opts)?;
                zip.write_all(serde_json::to_string_pretty(&folders)?.as_bytes())?;
            }
        }
    }

    zip.finish()?;
    Ok(exported)
}

#[derive(Serialize)]
pub struct ImportSummary {
    /// Título de cada sessão importada (mais útil pro usuário do que o id
    /// interno, que muda na importação — ver comentário do módulo).
    pub imported: Vec<String>,
    /// Entradas de sessão no zip que não puderam ser lidas/interpretadas
    /// (`session.json` ausente ou corrompido) — não é mais possível "já
    /// existir" já que todo import ganha um id novo.
    pub skipped_invalid: Vec<String>,
    pub folders_added: usize,
}

/// Importa sessões de um `.zip` gerado por `export_sessions_zip`. Cada
/// sessão do zip vira uma sessão NOVA (id novo, `parent_session_id` limpo);
/// `folder_id` só é preservado se o manifesto indicar backup completo (ver
/// comentário do módulo).
pub fn import_sessions_zip(app_data_dir: &Path, source_path: &Path) -> Result<ImportSummary> {
    let file = std::fs::File::open(source_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let sessions_root = sessions_dir(app_data_dir);
    std::fs::create_dir_all(&sessions_root)?;

    let mut full_backup = false;
    let mut incoming_folders: Vec<crate::models::Folder> = Vec::new();
    let mut by_session: HashMap<String, Vec<(PathBuf, usize)>> = HashMap::new();

    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let enclosed_str = enclosed.to_string_lossy().to_string();
        if enclosed_str == MANIFEST_NAME {
            drop(entry);
            let mut text = String::new();
            archive.by_index(i)?.read_to_string(&mut text)?;
            if let Ok(manifest) = serde_json::from_str::<Manifest>(&text) {
                full_backup = manifest.full_backup;
            }
            continue;
        }
        if enclosed_str == FOLDERS_NAME {
            drop(entry);
            let mut text = String::new();
            archive.by_index(i)?.read_to_string(&mut text)?;
            incoming_folders = serde_json::from_str(&text).unwrap_or_default();
            continue;
        }

        // Espera "sessions/<id>/<arquivo>" — qualquer outra coisa é
        // ignorada (enclosed_name() já rejeitou `..`/caminho absoluto).
        let mut components = enclosed.components();
        let Some(first) = components.next() else { continue };
        if first.as_os_str() != "sessions" {
            continue;
        }
        let Some(id_component) = components.next() else {
            continue;
        };
        let Some(file_component) = components.next() else {
            continue;
        };
        if components.next().is_some() {
            continue;
        }
        let filename = file_component.as_os_str().to_string_lossy().to_string();
        if !SESSION_FILES.contains(&filename.as_str()) {
            continue;
        }
        let old_id = id_component.as_os_str().to_string_lossy().to_string();
        by_session.entry(old_id).or_default().push((enclosed.to_path_buf(), i));
    }

    let folders_added = if full_backup && !incoming_folders.is_empty() {
        crate::folders::merge_folders(app_data_dir, incoming_folders)?.len()
    } else {
        0
    };

    let mut imported = Vec::new();
    let mut skipped_invalid = Vec::new();
    for (old_id, files) in by_session {
        let session_json_entry = files.iter().find(|(rel, _)| {
            rel.file_name().map(|f| f == "session.json").unwrap_or(false)
        });
        let Some((_, idx)) = session_json_entry else {
            skipped_invalid.push(old_id);
            continue;
        };
        let mut raw = String::new();
        archive.by_index(*idx)?.read_to_string(&mut raw)?;
        let Ok(mut session_value) = serde_json::from_str::<Value>(&raw) else {
            skipped_invalid.push(old_id);
            continue;
        };

        let new_id = uuid::Uuid::new_v4().to_string();
        let title = session_value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(&old_id)
            .to_string();
        if let Some(obj) = session_value.as_object_mut() {
            obj.insert("id".to_string(), Value::String(new_id.clone()));
            obj.insert("parent_session_id".to_string(), Value::Null);
            if !full_backup {
                obj.insert("folder_id".to_string(), Value::Null);
            }
        }

        let target_dir = sessions_root.join(&new_id);
        std::fs::create_dir_all(&target_dir)?;
        for (rel, idx) in &files {
            let filename = rel.file_name().unwrap().to_string_lossy().to_string();
            let dest = target_dir.join(&filename);
            // Checagem redundante além de enclosed_name(): o caminho final
            // resolvido tem que continuar dentro de sessions_root.
            if !dest.starts_with(&sessions_root) {
                continue;
            }
            if filename == "session.json" {
                std::fs::write(&dest, serde_json::to_string_pretty(&session_value)?)?;
            } else {
                let mut entry = archive.by_index(*idx)?;
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                std::fs::write(&dest, buf)?;
            }
        }
        imported.push(title);
    }

    Ok(ImportSummary {
        imported,
        skipped_invalid,
        folders_added,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-backup-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed_session(app_data_dir: &Path, id: &str, title: &str, folder_id: Option<&str>) {
        let dir = sessions_dir(app_data_dir).join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let folder_json = match folder_id {
            Some(f) => format!("\"{f}\""),
            None => "null".to_string(),
        };
        std::fs::write(
            dir.join("session.json"),
            format!(
                r#"{{"id":"{id}","title":"{title}","parent_session_id":null,"folder_id":{folder_json}}}"#
            ),
        )
        .unwrap();
        std::fs::write(dir.join("chat_log.json"), "[]").unwrap();
        std::fs::write(dir.join("tasks.json"), "[]").unwrap();
    }

    #[test]
    fn export_all_then_import_gives_fresh_ids_and_keeps_folder_id() {
        let src = scratch_dir();
        seed_session(&src, "session-a", "A", Some("folder-1"));
        seed_session(&src, "session-b", "B", None);
        std::fs::write(
            src.join("folders.json"),
            r#"[{"id":"folder-1","name":"Trabalho","parent_id":null}]"#,
        )
        .unwrap();
        let zip_path = src.join("backup.zip");

        let exported = export_sessions_zip(&src, &[], &zip_path).unwrap();
        assert_eq!(exported, 2);

        let dest = scratch_dir();
        let summary = import_sessions_zip(&dest, &zip_path).unwrap();
        assert_eq!(summary.skipped_invalid.len(), 0);
        assert_eq!(summary.folders_added, 1);
        let mut titles = summary.imported.clone();
        titles.sort();
        assert_eq!(titles, vec!["A".to_string(), "B".to_string()]);

        // Sessão restaurada tem um id NOVO, diferente de "session-a".
        let restored_dirs: Vec<String> = std::fs::read_dir(sessions_dir(&dest))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();
        assert_eq!(restored_dirs.len(), 2);
        assert!(!restored_dirs.contains(&"session-a".to_string()));

        // A sessão que tinha folder_id="folder-1" continua apontando pra
        // ela (backup completo preserva, e a pasta foi importada junto).
        let a_dir = restored_dirs
            .iter()
            .find(|d| {
                let content = std::fs::read_to_string(sessions_dir(&dest).join(d).join("session.json")).unwrap();
                let value: Value = serde_json::from_str(&content).unwrap();
                value["title"] == "A"
            })
            .unwrap();
        let content = std::fs::read_to_string(sessions_dir(&dest).join(a_dir).join("session.json")).unwrap();
        let value: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value["folder_id"], "folder-1");

        let folders = std::fs::read_to_string(dest.join("folders.json")).unwrap();
        assert!(folders.contains("Trabalho"));
    }

    #[test]
    fn export_single_session_clears_folder_id_on_import() {
        let src = scratch_dir();
        seed_session(&src, "session-a", "A", Some("folder-1"));
        let zip_path = src.join("backup.zip");

        export_sessions_zip(&src, &["session-a".to_string()], &zip_path).unwrap();

        let dest = scratch_dir();
        let summary = import_sessions_zip(&dest, &zip_path).unwrap();
        assert_eq!(summary.imported, vec!["A".to_string()]);
        assert_eq!(summary.folders_added, 0);

        let restored_dir = std::fs::read_dir(sessions_dir(&dest))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let content = std::fs::read_to_string(restored_dir.join("session.json")).unwrap();
        let value: Value = serde_json::from_str(&content).unwrap();
        assert!(value["folder_id"].is_null());
    }

    #[test]
    fn reimporting_the_same_zip_creates_a_second_copy_not_a_conflict() {
        let src = scratch_dir();
        seed_session(&src, "session-a", "A", None);
        let zip_path = src.join("backup.zip");
        export_sessions_zip(&src, &[], &zip_path).unwrap();

        let dest = scratch_dir();
        import_sessions_zip(&dest, &zip_path).unwrap();
        let summary2 = import_sessions_zip(&dest, &zip_path).unwrap();
        assert_eq!(summary2.imported, vec!["A".to_string()]);

        let count = std::fs::read_dir(sessions_dir(&dest)).unwrap().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn import_rejects_zip_slip_entries() {
        let dir = scratch_dir();
        let zip_path = dir.join("malicious.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        // Tenta escapar de sessions_root via ".." no nome da entrada.
        zip.start_file("sessions/../../evil.json", opts).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();

        let dest = scratch_dir();
        let summary = import_sessions_zip(&dest, &zip_path).unwrap();
        assert_eq!(summary.imported.len(), 0);
        assert!(!dest.parent().unwrap().join("evil.json").exists());
    }
}
