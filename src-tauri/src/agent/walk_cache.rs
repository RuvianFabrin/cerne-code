//! Cache de travessia de diretorio com TTL, inspirado no `pi-walker` do
//! oh-my-pi (`crates/pi-walker/src/cache.rs`) — mas sem vendorizar o fork:
//! usa `ignore` (ja dependencia do Cerne) e `dashmap` (crate real e mantida,
//! nao um fork) direto, reduzido ao que o Cerne de fato precisa (um unico
//! usuario, uma sessao ativa por vez, sem a API generica de builder do
//! original).
//!
//! `grep` faz uma travessia recursiva completa da raiz de busca a cada
//! chamada; num turno em que o agente chama `grep` varias vezes seguidas na
//! mesma subpasta (comum ao investigar um bug), cada chamada repetia o
//! `WalkBuilder` inteiro. Esse cache guarda a lista de arquivos por
//! (caminho canonico, ttl) e invalida sozinho depois de `CACHE_TTL_MS` ou
//! quando o arquivo real muda por baixo (edicao aceita, `run_command`).
//!
//! Travessia usa `WalkBuilder::build_parallel()` — o walker paralelo nativo
//! da propria crate `ignore` (o mesmo motor de travessia que o binario `rg`
//! usa por baixo), nao a crate `rayon` do `pi-walker` original: escala
//! sozinho pro numero de CPUs disponiveis sem precisar de outra dependencia.

use dashmap::DashMap;
use ignore::{WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_millis(1_000);

struct CacheEntry {
    created_at: Instant,
    files: Vec<PathBuf>,
}

static CACHE: LazyLock<DashMap<PathBuf, CacheEntry>> = LazyLock::new(DashMap::new);

/// Lista os arquivos (nao diretorios) sob `root`, respeitando gitignore/
/// ignore como o `WalkBuilder` padrao ja faz. Reusa o resultado da ultima
/// varredura dessa mesma raiz se ainda estiver dentro do TTL.
pub fn files_under(root: &Path) -> Vec<PathBuf> {
    let key = root.to_path_buf();
    if let Some(entry) = CACHE.get(&key) {
        if entry.created_at.elapsed() < CACHE_TTL {
            return entry.files.clone();
        }
    }
    let found = Mutex::new(Vec::new());
    WalkBuilder::new(root).build_parallel().run(|| {
        Box::new(|entry| {
            if let Ok(entry) = entry {
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    found.lock().unwrap().push(entry.path().to_path_buf());
                }
            }
            WalkState::Continue
        })
    });
    let mut files = found.into_inner().unwrap();
    // A travessia paralela nao garante ordem de chegada entre threads;
    // ordena pra saida deterministica (grep/ast_grep reportam resultado na
    // mesma ordem toda vez, mais facil de ler e de testar).
    files.sort();
    CACHE.insert(
        key,
        CacheEntry {
            created_at: Instant::now(),
            files: files.clone(),
        },
    );
    files
}

/// Descarta entradas de cache cuja raiz contem (ou e igual a) `changed_path`
/// — chamado depois de qualquer operacao que possa ter mudado o filesystem
/// real (edicao aceita, `run_command`), pra nao servir uma lista de arquivos
/// desatualizada na proxima busca.
pub fn invalidate(changed_path: &Path) {
    CACHE.retain(|root, _| !changed_path.starts_with(root) && !root.starts_with(changed_path));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cerne-walkcache-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn files_under_finds_every_file_across_many_nested_dirs_in_sorted_order() {
        let dir = scratch_dir();
        // Arvore com varias subpastas pra dar trabalho de verdade pro walker
        // paralelo distribuir entre threads, nao só 1-2 arquivos triviais.
        for i in 0..40 {
            let sub = dir.join(format!("sub{}", i % 5));
            fs::create_dir_all(&sub).unwrap();
            fs::write(sub.join(format!("f{i}.txt")), "x").unwrap();
        }
        let files = files_under(&dir);
        assert_eq!(files.len(), 40, "walker paralelo nao pode perder arquivo");
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(
            files, sorted,
            "saida deveria vir ordenada, apesar da travessia ser paralela"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn files_under_lists_files_and_caches_result() {
        let dir = scratch_dir();
        fs::write(dir.join("a.txt"), "a").unwrap();
        let first = files_under(&dir);
        assert_eq!(first.len(), 1);

        // Escreve um segundo arquivo por baixo do cache; dentro do TTL a
        // lista cacheada ainda deve valer (mesmo comportamento do pi-walker).
        fs::write(dir.join("b.txt"), "b").unwrap();
        let cached = files_under(&dir);
        assert_eq!(
            cached.len(),
            1,
            "deveria servir do cache, ainda dentro do TTL"
        );

        invalidate(&dir);
        let fresh = files_under(&dir);
        assert_eq!(
            fresh.len(),
            2,
            "apos invalidate deveria reescanear e ver os 2 arquivos"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalidate_matches_subpaths_of_cached_root() {
        let dir = scratch_dir();
        fs::write(dir.join("a.txt"), "a").unwrap();
        files_under(&dir);
        assert!(CACHE.contains_key(&dir));

        invalidate(&dir.join("a.txt"));
        assert!(
            !CACHE.contains_key(&dir),
            "mudanca num arquivo dentro da raiz cacheada deve invalidar a raiz"
        );

        fs::remove_dir_all(&dir).ok();
    }
}
