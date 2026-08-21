// Item da lista de ideias do Hermes Agent (14_backlog_pendente.md): memória entre sessões — um
// arquivo `MEMORY.md` global (fora de qualquer sessão, em `app_data_dir`) que o LLM pode gravar
// fatos duráveis nele ("usuário prefere respostas em português", "projeto X usa pnpm não npm") e
// que é lido no INÍCIO de toda sessão (system prompt), pra não precisar reexplicar contexto que já
// foi estabelecido numa conversa anterior. Escopo deliberadamente simples: só append (nunca edita
// ou apaga uma entrada existente) — deixa o controle fino (editar/remover) pro usuário mesmo, lendo
// o arquivo direto do disco, em vez de dar ao LLM uma ferramenta de reescrita livre que poderia
// apagar memória por engano ou entrar num loop se reescrever mal formatado.

use anyhow::Result;
use std::path::{Path, PathBuf};

const HEADER: &str = "# Memória do Cerne Code\n\n\
    > Fatos que o assistente registrou pra lembrar em conversas futuras (preferências do usuário, \
    convenções do projeto, decisões já tomadas). Edite ou apague linhas livremente — isso aqui é \
    só um arquivo de texto normal.\n\n";

fn memory_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("MEMORY.md")
}

/// Lê o conteúdo de `MEMORY.md` pra injetar no system prompt — string vazia
/// se o arquivo ainda não existe (nenhum fato registrado ainda).
pub fn load_memory(app_data_dir: &Path) -> Result<String> {
    match std::fs::read_to_string(memory_path(app_data_dir)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// Sobrescreve o arquivo inteiro — usado pela edição manual em
/// Configurações (o usuário pode querer apagar ou corrigir uma entrada,
/// coisa que a tool `remember` do LLM deliberadamente não faz).
pub fn save_memory(app_data_dir: &Path, content: &str) -> Result<()> {
    std::fs::write(memory_path(app_data_dir), content)?;
    Ok(())
}

/// Acrescenta um fato novo como item de lista — nunca reescreve nem apaga o
/// que já tinha. Cria o arquivo (com cabeçalho) na primeira chamada.
pub fn append_memory(app_data_dir: &Path, fact: &str) -> Result<()> {
    let fact = fact.trim();
    if fact.is_empty() {
        return Ok(());
    }
    let path = memory_path(app_data_dir);
    let mut content = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HEADER.to_string(),
        Err(e) => return Err(e.into()),
    };
    if !content.ends_with('\n') {
        content.push('\n');
    }
    // Fato de uma linha só (troca quebra de linha interna por espaço) pra
    // manter o formato "- item" simples e sempre parseável visualmente.
    let single_line = fact.replace('\n', " ");
    content.push_str(&format!("- {single_line}\n"));
    std::fs::write(&path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-memory-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_memory_is_empty_string_when_file_does_not_exist() {
        let dir = scratch_dir();
        assert_eq!(load_memory(&dir).unwrap(), "");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_memory_creates_file_with_header_on_first_call() {
        let dir = scratch_dir();
        append_memory(&dir, "usuario prefere respostas em portugues").unwrap();
        let content = load_memory(&dir).unwrap();
        assert!(content.contains("# Memória do Cerne Code"));
        assert!(content.contains("- usuario prefere respostas em portugues"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_memory_accumulates_without_losing_previous_facts() {
        let dir = scratch_dir();
        append_memory(&dir, "fato 1").unwrap();
        append_memory(&dir, "fato 2").unwrap();
        let content = load_memory(&dir).unwrap();
        assert!(content.contains("- fato 1"));
        assert!(content.contains("- fato 2"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_memory_ignores_blank_fact() {
        let dir = scratch_dir();
        append_memory(&dir, "   ").unwrap();
        assert_eq!(load_memory(&dir).unwrap(), "");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_memory_collapses_internal_newlines_to_a_single_line() {
        let dir = scratch_dir();
        append_memory(&dir, "linha 1\nlinha 2").unwrap();
        let content = load_memory(&dir).unwrap();
        assert!(content.contains("- linha 1 linha 2"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
