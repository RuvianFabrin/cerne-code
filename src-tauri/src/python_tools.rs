//! T17: o LLM pode criar suas próprias ferramentas Python reutilizáveis
//! (`create_python_tool`/`update_python_tool`, ver `agent/tools.rs`), que
//! ficam disponíveis pra QUALQUER sessão futura — não só a que criou —
//! porque cada ferramenta vira automaticamente uma skill global (ver
//! `skills.rs`) com instruções de como chamá-la via `run_command`.
//!
//! Cada script roda via `uv run`, que lê um cabeçalho PEP 723 (metadata
//! inline de script — https://peps.python.org/pep-0723/) embutido no
//! `tool.py` gerado e instala as dependências listadas num ambiente
//! Python efêmero na hora, sem precisar gerenciar venv nenhum na mão.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythonToolMeta {
    name: String,
    description: String,
    #[serde(default)]
    dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PythonTool {
    pub name: String,
    pub description: String,
    pub dependencies: Vec<String>,
    /// Corpo do script SEM o cabeçalho PEP 723 (esse é gerado a partir de
    /// `dependencies` toda vez que o arquivo é escrito) — o que o
    /// LLM/usuário de fato le/edita.
    pub script: String,
    pub tool_path: String,
}

pub fn python_tools_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("python_tools")
}

fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "ferramenta".to_string()
    } else {
        slug
    }
}

fn tool_dir(app_data_dir: &Path, slug: &str) -> PathBuf {
    python_tools_dir(app_data_dir).join(slug)
}

fn skill_slug_for(slug: &str) -> String {
    format!("python-tool-{slug}")
}

/// Cabeçalho PEP 723 — é isso que faz `uv run tool.py` instalar as
/// dependências listadas num ambiente efêmero, sem precisar de um venv
/// gerenciado manualmente pelo Cerne.
fn pep723_header(dependencies: &[String]) -> String {
    let deps = dependencies
        .iter()
        .map(|d| format!("\"{}\"", d.replace('"', "'")))
        .collect::<Vec<_>>()
        .join(", ");
    format!("# /// script\n# dependencies = [{deps}]\n# ///\n\n")
}

/// Remove o cabeçalho PEP 723 de volta, pra devolver só o corpo do script
/// (o que o LLM/usuário escreveu de fato) na hora de listar/ler.
fn strip_pep723_header(full: &str) -> String {
    if let Some(start) = full.find("# /// script") {
        if let Some(end_rel) = full[start..].find("# ///\n") {
            let end = start + end_rel + "# ///\n".len();
            return full[end..].trim_start_matches(['\r', '\n']).to_string();
        }
    }
    full.to_string()
}

fn write_tool_file(dir: &Path, script: &str, dependencies: &[String]) -> Result<PathBuf> {
    let full = format!("{}{}", pep723_header(dependencies), script);
    let path = dir.join("tool.py");
    std::fs::write(&path, full)?;
    Ok(path)
}

/// Skill auto-gerada que ensina o agente (em QUALQUER sessão futura) a
/// chamar essa ferramenta — nome prefixado `python-tool-` pra não colidir
/// com skills que o usuário criou manualmente e pra ficar claro na lista
/// que ela foi gerada, não escrita à mão.
fn write_companion_skill(
    app_data_dir: &Path,
    slug: &str,
    description: &str,
    tool_path: &Path,
    dependencies: &[String],
) -> Result<()> {
    let skill_dir = crate::skills::global_skills_dir(app_data_dir).join(skill_slug_for(slug));
    std::fs::create_dir_all(&skill_dir)?;
    let deps_note = if dependencies.is_empty() {
        "nenhuma (so biblioteca padrao do Python)".to_string()
    } else {
        dependencies.join(", ")
    };
    let body = format!(
        "---\nname: {skill_name}\ndescription: Ferramenta Python criada dinamicamente pelo proprio agente. {description}\n---\n\n\
## O que e\n\n\
Uma ferramenta Python que uma sessao anterior criou com `create_python_tool`, disponivel pra reusar agora — nao precisa reescrever o codigo.\n\n\
## Dependencias (instaladas automaticamente pelo `uv`, sem venv manual)\n\n\
{deps_note}\n\n\
## Como usar\n\n\
Rode via `run_command`:\n\n\
```\nuv run \"{tool_path}\" [argumentos aqui]\n```\n\n\
`uv` cuida de instalar as dependencias listadas acima num ambiente efemero na primeira execucao. Se nao tiver certeza dos argumentos esperados, use `read_file` pra ler o script antes de chamar. Se encontrar um bug ou quiser mudar o comportamento, use `update_python_tool` em vez de criar uma ferramenta nova do zero.\n",
        skill_name = skill_slug_for(slug),
        description = description,
        deps_note = deps_note,
        tool_path = tool_path.display(),
    );
    std::fs::write(skill_dir.join("SKILL.md"), body)?;
    Ok(())
}

fn read_meta(dir: &Path) -> Result<PythonToolMeta> {
    let text = std::fs::read_to_string(dir.join("meta.json"))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn list_python_tools(app_data_dir: &Path) -> Result<Vec<PythonTool>> {
    let dir = python_tools_dir(app_data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut tools = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let Ok(meta) = read_meta(&entry.path()) else {
            continue;
        };
        let tool_path = entry.path().join("tool.py");
        let script = std::fs::read_to_string(&tool_path)
            .map(|full| strip_pep723_header(&full))
            .unwrap_or_default();
        tools.push(PythonTool {
            name: meta.name,
            description: meta.description,
            dependencies: meta.dependencies,
            script,
            tool_path: tool_path.to_string_lossy().to_string(),
        });
    }
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tools)
}

pub fn create_python_tool(
    app_data_dir: &Path,
    name: &str,
    description: &str,
    script: &str,
    dependencies: Vec<String>,
) -> Result<PythonTool> {
    let slug = slugify(name);
    let dir = tool_dir(app_data_dir, &slug);
    if dir.exists() {
        return Err(anyhow!(
            "ja existe uma ferramenta python chamada '{slug}' - use update_python_tool pra editar a existente"
        ));
    }
    std::fs::create_dir_all(&dir)?;
    let tool_path = write_tool_file(&dir, script, &dependencies)?;
    let meta = PythonToolMeta {
        name: slug.clone(),
        description: description.to_string(),
        dependencies: dependencies.clone(),
    };
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;
    write_companion_skill(app_data_dir, &slug, description, &tool_path, &dependencies)?;
    Ok(PythonTool {
        name: slug,
        description: description.to_string(),
        dependencies,
        script: script.to_string(),
        tool_path: tool_path.to_string_lossy().to_string(),
    })
}

pub fn update_python_tool(
    app_data_dir: &Path,
    name: &str,
    description: &str,
    script: &str,
    dependencies: Vec<String>,
) -> Result<PythonTool> {
    let slug = slugify(name);
    let dir = tool_dir(app_data_dir, &slug);
    if !dir.exists() {
        return Err(anyhow!(
            "ferramenta python '{slug}' nao encontrada - use create_python_tool pra criar uma nova"
        ));
    }
    let tool_path = write_tool_file(&dir, script, &dependencies)?;
    let meta = PythonToolMeta {
        name: slug.clone(),
        description: description.to_string(),
        dependencies: dependencies.clone(),
    };
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;
    write_companion_skill(app_data_dir, &slug, description, &tool_path, &dependencies)?;
    Ok(PythonTool {
        name: slug,
        description: description.to_string(),
        dependencies,
        script: script.to_string(),
        tool_path: tool_path.to_string_lossy().to_string(),
    })
}

/// Remove a ferramenta E a skill companheira dela — sem isso o catalogo de
/// skills ficaria com uma entrada "fantasma" apontando pra um `tool.py` que
/// nao existe mais.
pub fn delete_python_tool(app_data_dir: &Path, name: &str) -> Result<()> {
    let slug = slugify(name);
    let dir = tool_dir(app_data_dir, &slug);
    if !dir.exists() {
        return Err(anyhow!("ferramenta python '{slug}' nao encontrada"));
    }
    std::fs::remove_dir_all(&dir)?;
    let skill_dir = crate::skills::global_skills_dir(app_data_dir).join(skill_slug_for(&slug));
    if skill_dir.exists() {
        std::fs::remove_dir_all(&skill_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-pytools-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_then_list_roundtrip() {
        let dir = scratch_dir();
        let tool = create_python_tool(
            &dir,
            "Gerador de QR Code",
            "Gera um QR code a partir de um texto",
            "import sys\nprint(sys.argv[1])\n",
            vec!["qrcode".to_string()],
        )
        .unwrap();
        assert_eq!(tool.name, "gerador-de-qr-code");
        assert_eq!(tool.dependencies, vec!["qrcode"]);

        let found = list_python_tools(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "gerador-de-qr-code");
        assert_eq!(found[0].script, "import sys\nprint(sys.argv[1])\n");

        // tool.py de verdade no disco tem o cabecalho PEP 723 embutido, pra
        // `uv run` conseguir instalar a dependencia sozinho.
        let raw = std::fs::read_to_string(&found[0].tool_path).unwrap();
        assert!(raw.contains("# /// script"));
        assert!(raw.contains("\"qrcode\""));

        // skill companheira criada junto, com instrucoes de uso.
        let skills = crate::skills::list_skills(&dir, None).unwrap();
        assert!(skills.iter().any(|s| s.name == "python-tool-gerador-de-qr-code"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_duplicate_name_is_rejected() {
        let dir = scratch_dir();
        create_python_tool(&dir, "Meu Tool", "desc", "print('a')", vec![]).unwrap();
        let result = create_python_tool(&dir, "Meu Tool", "outra desc", "print('b')", vec![]);
        assert!(result.is_err(), "nome duplicado deveria falhar em create");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_changes_script_and_regenerates_skill() {
        let dir = scratch_dir();
        create_python_tool(&dir, "Meu Tool", "desc original", "print('v1')", vec![]).unwrap();
        let updated = update_python_tool(
            &dir,
            "Meu Tool",
            "desc atualizada",
            "print('v2')",
            vec!["requests".to_string()],
        )
        .unwrap();
        assert_eq!(updated.script, "print('v2')");
        assert_eq!(updated.dependencies, vec!["requests"]);

        let found = list_python_tools(&dir).unwrap();
        assert_eq!(found.len(), 1, "update nao deveria criar uma segunda entrada");
        assert_eq!(found[0].script, "print('v2')");

        let body = crate::skills::load_skill_body(&dir, None, "python-tool-meu-tool").unwrap();
        assert!(body.contains("requests"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_missing_tool_errors() {
        let dir = scratch_dir();
        let result = update_python_tool(&dir, "nao-existe", "desc", "print(1)", vec![]);
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_removes_tool_and_companion_skill() {
        let dir = scratch_dir();
        create_python_tool(&dir, "Descartavel", "desc", "print(1)", vec![]).unwrap();
        assert_eq!(list_python_tools(&dir).unwrap().len(), 1);

        delete_python_tool(&dir, "Descartavel").unwrap();
        assert!(list_python_tools(&dir).unwrap().is_empty());
        let skills = crate::skills::list_skills(&dir, None).unwrap();
        assert!(!skills.iter().any(|s| s.name == "python-tool-descartavel"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_missing_tool_errors() {
        let dir = scratch_dir();
        assert!(delete_python_tool(&dir, "nao-existe").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
