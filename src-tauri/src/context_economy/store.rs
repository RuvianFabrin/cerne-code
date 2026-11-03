//! Native SQLite/FTS5 storage. Context Mode port, Elastic License 2.0.
use super::chunk;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
pub struct Store {
    pub db: Connection,
    pub root: PathBuf,
}

impl Store {
    pub fn open(data: &Path, id: &str) -> Result<Self> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow!("Identificador de conversa invalido"));
        }
        let root = data.join("sessions").join(id).join("context-economy");
        std::fs::create_dir_all(&root)?;
        let db = Connection::open(root.join("native.sqlite3"))?;
        db.busy_timeout(Duration::from_secs(10))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;
          CREATE TABLE IF NOT EXISTS sources(id INTEGER PRIMARY KEY, label TEXT NOT NULL UNIQUE, raw TEXT NOT NULL, hash TEXT NOT NULL, path TEXT, at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')));
          CREATE VIRTUAL TABLE IF NOT EXISTS chunks USING fts5(title,content,source_id UNINDEXED,content_type UNINDEXED,tokenize='porter unicode61');
          CREATE VIRTUAL TABLE IF NOT EXISTS chunks_trigram USING fts5(title,content,source_id UNINDEXED,content_type UNINDEXED,tokenize='trigram');
          CREATE TABLE IF NOT EXISTS vocabulary(word TEXT PRIMARY KEY);
          CREATE TABLE IF NOT EXISTS records(key TEXT PRIMARY KEY, ordinal TEXT NOT NULL, role TEXT NOT NULL, name TEXT NOT NULL,args TEXT NOT NULL,content TEXT NOT NULL,is_error INTEGER NOT NULL,at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')));
          CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY,record_key TEXT NOT NULL,kind TEXT NOT NULL,category TEXT NOT NULL,data TEXT NOT NULL,priority INTEGER NOT NULL,at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),UNIQUE(record_key,kind,category,data));
          CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS calls(id INTEGER PRIMARY KEY,name TEXT NOT NULL,input_bytes INTEGER NOT NULL,output_bytes INTEGER NOT NULL,avoided_bytes INTEGER NOT NULL,elapsed_ms INTEGER NOT NULL,is_error INTEGER NOT NULL,at INTEGER NOT NULL DEFAULT (unixepoch()));
          CREATE TABLE IF NOT EXISTS fetch_cache(url TEXT PRIMARY KEY,source TEXT NOT NULL,expires INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS search_calls(at INTEGER NOT NULL);
          PRAGMA user_version=1;")?;
        Ok(Self { db, root })
    }

    pub fn index(&mut self, text: &str, source: &str, path: Option<&str>) -> Result<usize> {
        let digest = hash(text);
        let previous: Option<(i64, String)> = self
            .db
            .query_row("SELECT id,hash FROM sources WHERE label=?", [source], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        if previous.as_ref().is_some_and(|(_, h)| h == &digest) {
            return Ok(self.db.query_row::<i64, _, _>(
                "SELECT count(*) FROM chunks WHERE source_id=?",
                [previous.unwrap().0],
                |r| r.get(0),
            )? as usize);
        }
        let chunks = chunk::split(text);
        let tx = self.db.transaction()?;
        let source_id = if let Some((id, _)) = previous {
            tx.execute("DELETE FROM chunks WHERE source_id=?", [id])?;
            tx.execute("DELETE FROM chunks_trigram WHERE source_id=?", [id])?;
            tx.execute("UPDATE sources SET raw=?,hash=?,path=?,at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?",params![text,digest,path,id])?;
            id
        } else {
            tx.execute(
                "INSERT INTO sources(label,raw,hash,path) VALUES (?,?,?,?)",
                params![source, text, digest, path],
            )?;
            tx.last_insert_rowid()
        };
        let mut vocabulary = std::collections::HashSet::new();
        for chunk in &chunks {
            let kind = if chunk.code { "code" } else { "prose" };
            tx.execute(
                "INSERT INTO chunks(title,content,source_id,content_type) VALUES (?,?,?,?)",
                params![chunk.title, chunk.content, source_id, kind],
            )?;
            let rowid = tx.last_insert_rowid();
            tx.execute("INSERT INTO chunks_trigram(rowid,title,content,source_id,content_type) VALUES (?,?,?,?,?)", params![rowid,chunk.title,chunk.content,source_id,kind])?;
            for word in chunk
                .content
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|w| w.chars().count() >= 3 && w.len() <= 120)
            {
                vocabulary.insert(word.to_lowercase());
            }
        }
        for word in vocabulary {
            tx.execute("INSERT OR IGNORE INTO vocabulary(word) VALUES (?)", [word])?;
        }
        tx.commit()?;
        Ok(chunks.len())
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row("SELECT value FROM meta WHERE key=?", [key], |r| r.get(0))
            .optional()?)
    }
    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.db.execute("INSERT INTO meta(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
        Ok(())
    }
    pub fn record_call(
        &self,
        name: &str,
        input: usize,
        output: usize,
        avoided: usize,
        elapsed: u128,
        error: bool,
    ) -> Result<()> {
        self.db.execute("INSERT INTO calls(name,input_bytes,output_bytes,avoided_bytes,elapsed_ms,is_error) VALUES (?,?,?,?,?,?)", params![name,input as i64,output as i64,avoided as i64,elapsed.min(i64::MAX as u128) as i64,error])?;
        Ok(())
    }
    pub fn stats(&self) -> Result<Value> {
        let sources: i64 = self
            .db
            .query_row("SELECT count(*) FROM sources", [], |r| r.get(0))?;
        let chunks: i64 = self
            .db
            .query_row("SELECT count(*) FROM chunks", [], |r| r.get(0))?;
        let raw: i64 = self.db.query_row(
            "SELECT coalesce(sum(length(cast(raw as blob))),0) FROM sources",
            [],
            |r| r.get(0),
        )?;
        let records: i64 = self
            .db
            .query_row("SELECT count(*) FROM records", [], |r| r.get(0))?;
        let events: i64 = self
            .db
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))?;
        let mut statement = self.db.prepare("SELECT name,count(*),sum(input_bytes),sum(output_bytes),sum(avoided_bytes),sum(elapsed_ms),sum(is_error) FROM calls GROUP BY name ORDER BY name")?;
        let calls: Vec<Value> = statement.query_map([], |r| Ok(json!({"tool":r.get::<_,String>(0)?,"calls":r.get::<_,i64>(1)?,"input_bytes":r.get::<_,i64>(2)?,"output_bytes":r.get::<_,i64>(3)?,"avoided_bytes":r.get::<_,i64>(4)?,"elapsed_ms":r.get::<_,i64>(5)?,"errors":r.get::<_,i64>(6)?})))?.collect::<rusqlite::Result<_>>()?;
        let mut statement = self
            .db
            .prepare("SELECT category,count(*) FROM events GROUP BY category ORDER BY category")?;
        let categories: Vec<Value> = statement
            .query_map([], |r| {
                Ok(json!({"category":r.get::<_,String>(0)?,"count":r.get::<_,i64>(1)?}))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(
            json!({"engine":"Rust / SQLite FTS5", "sources":sources,"chunks":chunks,"stored_bytes":raw,"records":records,"events":events,"categories":categories,"tools":calls}),
        )
    }

    /// Read-only, retryable migration. Old databases remain a recovery copy.
    pub fn import_legacy(&mut self) -> Result<usize> {
        if self.meta("legacy_imported")?.is_some() {
            return Ok(0);
        }
        let mut paths = vec![];
        fn visit(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
            if !root.exists() {
                return Ok(());
            }
            for entry in std::fs::read_dir(root)? {
                let entry = entry?;
                if entry.file_type()?.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    visit(&path, paths)?;
                } else if path
                    .extension()
                    .is_some_and(|ext| ext == "db" || ext == "sqlite")
                {
                    paths.push(path);
                }
            }
            Ok(())
        }
        visit(&self.root.join("content"), &mut paths)?;
        visit(&self.root.join("sessions"), &mut paths)?;
        let mut count = 0;
        for path in paths {
            let legacy = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            legacy.busy_timeout(Duration::from_secs(10))?;
            let has = |name: &str| -> Result<bool> {
                Ok(legacy.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?)",
                    [name],
                    |r| r.get(0),
                )?)
            };
            if has("sources")? && has("chunks")? {
                let mut statement = legacy.prepare("SELECT s.id,s.label,c.title,c.content FROM sources s JOIN chunks c ON c.source_id=s.id ORDER BY s.id,c.rowid")?;
                let rows: Vec<(i64, String, String, String)> = statement
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                // Import every old chunk with a distinct source; repeated labels lose nothing.
                for (number, (id, label, title, content)) in rows.into_iter().enumerate() {
                    self.index(
                        &content,
                        &format!(
                            "legacy:{}:{label}:{id}:{number}",
                            hash(&path.to_string_lossy())
                        ),
                        None,
                    )?;
                    self.set_meta(&format!("legacy-title:{id}:{number}"), &title)?;
                    count += 1;
                }
            }
            if has("session_events")? {
                let mut statement = legacy.prepare(
                    "SELECT type,category,data,priority,created_at FROM session_events ORDER BY id",
                )?;
                let rows: Vec<(String, String, String, i64, String)> = statement
                    .query_map([], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                for (n, (kind, category, data, priority, at)) in rows.into_iter().enumerate() {
                    let key = hash(&format!("{}:{n}:{data}", path.display()));
                    self.db.execute("INSERT OR IGNORE INTO events(record_key,kind,category,data,priority,at) VALUES (?,?,?,?,?,?)",params![key,kind,category,data,priority,at])?;
                    count += 1;
                }
            }
        }
        self.set_meta("legacy_imported", "1")?;
        Ok(count)
    }
}
