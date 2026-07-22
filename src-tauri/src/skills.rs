use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    /// "global" (lives under the app data dir, every session) or "project"
    /// (lives under `{project_root}/.cerne/skills`, this session only).
    pub scope: String,
    pub dir: String,
}

const README_TEMPLATE: &str = "# Skills do Cerne Code\n\n\
Cada skill e uma pasta com um `SKILL.md` dentro, no mesmo formato do Claude \
Code:\n\n\
```\n\
---\n\
name: nome-da-skill\n\
description: Uma linha dizendo QUANDO usar essa skill (o agente decide se \
carrega com base nisso).\n\
---\n\n\
Instrucoes detalhadas aqui - o que fazer, passos, convencoes do projeto, etc.\n\
```\n\n\
O agente ve so o `name`/`description` de cada skill por padrao (pra nao \
inflar o prompt) e carrega o corpo inteiro sob demanda, via uma ferramenta \
`load_skill`, quando decide que a skill e relevante pro pedido atual.\n\n\
Skills aqui em `skills/` valem pra qualquer sessao. Skills dentro de um \
projeto, em `<projeto>/.cerne/skills/`, valem so pras sessoes daquele \
projeto.\n";

pub fn global_skills_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("skills")
}

pub fn project_skills_dir(project_root: &Path) -> PathBuf {
    project_root.join(".cerne").join("skills")
}

/// Ensures the global skills folder exists, writing a short README the
/// first time so the user has something to look at when they open it.
pub fn ensure_global_skills_dir(app_data_dir: &Path) -> Result<PathBuf> {
    let dir = global_skills_dir(app_data_dir);
    std::fs::create_dir_all(&dir)?;
    let readme = dir.join("_README.md");
    if !readme.exists() {
        std::fs::write(&readme, README_TEMPLATE)?;
    }
    Ok(dir)
}

pub fn list_skills(app_data_dir: &Path, project_root: Option<&Path>) -> Result<Vec<SkillMeta>> {
    let mut skills = Vec::new();
    skills.extend(scan_dir(&global_skills_dir(app_data_dir), "global")?);
    if let Some(root) = project_root {
        skills.extend(scan_dir(&project_skills_dir(root), "project")?);
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

fn scan_dir(dir: &Path, scope: &str) -> Result<Vec<SkillMeta>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut skills = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&skill_md)?;
        let (frontmatter, _) = split_frontmatter(&text);
        let name = frontmatter
            .get("name")
            .cloned()
            .unwrap_or_else(|| entry.file_name().to_string_lossy().to_string());
        let description = frontmatter.get("description").cloned().unwrap_or_default();
        skills.push(SkillMeta {
            name,
            description,
            scope: scope.to_string(),
            dir: entry.path().to_string_lossy().to_string(),
        });
    }
    Ok(skills)
}

/// Finds a skill by name (global first, then project-scoped) and returns
/// its body (the file content after the `---` frontmatter block).
pub fn load_skill_body(
    app_data_dir: &Path,
    project_root: Option<&Path>,
    name: &str,
) -> Result<String> {
    let mut dirs = vec![global_skills_dir(app_data_dir)];
    if let Some(root) = project_root {
        dirs.push(project_skills_dir(root));
    }

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            let skill_md = entry.path().join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&skill_md)?;
            let (frontmatter, body) = split_frontmatter(&text);
            let matches = frontmatter.get("name").map(|n| n == name).unwrap_or(false)
                || entry.file_name().to_string_lossy() == name;
            if matches {
                return Ok(body.to_string());
            }
        }
    }

    Err(anyhow!("skill '{name}' nao encontrada"))
}

/// Very small frontmatter parser: `---\nkey: value\n...\n---\nbody`. Good
/// enough for flat `name`/`description` pairs — skills don't need nested
/// YAML, so a full parser dependency isn't worth it.
fn split_frontmatter(text: &str) -> (std::collections::HashMap<String, String>, &str) {
    let mut map = std::collections::HashMap::new();
    let Some(rest) = text.strip_prefix("---") else {
        return (map, text);
    };
    let Some(end) = rest.find("\n---") else {
        return (map, text);
    };
    let frontmatter = &rest[..end];
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']);

    for line in frontmatter.lines() {
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    (map, body)
}

/// Idioma do corpo pré-formatado gerado por `create_skill`. As instruções
/// que o agente eventualmente lê (via `load_skill`) são texto livre — o
/// idioma aqui é só o do template inicial que o usuário preenche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillLanguage {
    PtBr,
    En,
}

const TEMPLATE_BODY_PT_BR: &str = "\n## Objetivo\n\n\
Explique em 1-2 frases o que essa skill ensina o agente a fazer.\n\n\
## Quando usar\n\n\
Descreva o gatilho: em que tipo de pedido do usuário essa skill é relevante? \
(a `description` no topo do arquivo já cobre isso resumidamente — aqui pode \
detalhar mais.)\n\n\
## Passo a passo\n\n\
1. Primeiro passo.\n\
2. Segundo passo.\n\
3. ...\n\n\
## Exemplo\n\n\
Se fizer sentido, cole um exemplo de entrada/saída ou um trecho de código que \
ilustre o resultado esperado.\n";

const TEMPLATE_BODY_EN: &str = "\n## Purpose\n\n\
Explain in 1-2 sentences what this skill teaches the agent to do.\n\n\
## When to use\n\n\
Describe the trigger: what kind of user request makes this skill relevant? \
(the `description` field at the top already covers this briefly — expand \
here if useful.)\n\n\
## Instructions\n\n\
1. First step.\n\
2. Second step.\n\
3. ...\n\n\
## Example\n\n\
If useful, paste an example input/output or a code snippet that illustrates \
the expected result.\n";

/// Corpo pré-formatado (sem frontmatter) pro idioma escolhido — usado tanto
/// por `create_skill` quanto pelo preview que a tela mostra antes de criar.
pub fn template_body(language: SkillLanguage) -> &'static str {
    match language {
        SkillLanguage::PtBr => TEMPLATE_BODY_PT_BR,
        SkillLanguage::En => TEMPLATE_BODY_EN,
    }
}

pub fn create_skill(
    app_data_dir: &Path,
    name: &str,
    description: &str,
    language: SkillLanguage,
) -> Result<PathBuf> {
    let slug = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let dir = ensure_global_skills_dir(app_data_dir)?.join(&slug);
    std::fs::create_dir_all(&dir)?;
    let skill_md = dir.join("SKILL.md");
    if skill_md.exists() {
        return Err(anyhow!("ja existe uma skill em {}", dir.display()));
    }
    std::fs::write(
        &skill_md,
        format!(
            "---\nname: {slug}\ndescription: {description}\n---\n{}",
            template_body(language)
        ),
    )?;
    Ok(dir)
}

/// Lê o `SKILL.md` inteiro (frontmatter + corpo) pra edição na tela. `dir` é
/// sempre um valor devolvido por `list_skills`, nunca digitado livremente
/// pelo usuário.
pub fn read_skill_file(dir: &str) -> Result<String> {
    Ok(std::fs::read_to_string(Path::new(dir).join("SKILL.md"))?)
}

pub fn write_skill_file(dir: &str, content: &str) -> Result<()> {
    std::fs::write(Path::new(dir).join("SKILL.md"), content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-skills-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn split_frontmatter_parses_name_and_description() {
        let text =
            "---\nname: minha-skill\ndescription: Quando usar isso\n---\n\nCorpo da skill aqui.\n";
        let (fm, body) = split_frontmatter(text);
        assert_eq!(fm.get("name").unwrap(), "minha-skill");
        assert_eq!(fm.get("description").unwrap(), "Quando usar isso");
        assert_eq!(body.trim(), "Corpo da skill aqui.");
    }

    #[test]
    fn split_frontmatter_handles_missing_frontmatter() {
        let (fm, body) = split_frontmatter("so um texto normal, sem frontmatter");
        assert!(fm.is_empty());
        assert_eq!(body, "so um texto normal, sem frontmatter");
    }

    #[test]
    fn create_then_list_then_load_roundtrip() {
        let app_data_dir = scratch_dir();
        create_skill(
            &app_data_dir,
            "Revisar PR",
            "Use ao revisar um pull request",
            SkillLanguage::PtBr,
        )
        .unwrap();

        let found = list_skills(&app_data_dir, None).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "revisar-pr");
        assert_eq!(found[0].scope, "global");

        let body = load_skill_body(&app_data_dir, None, "revisar-pr").unwrap();
        assert!(body.contains("Passo a passo"));

        std::fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn project_skills_are_scoped_to_their_project() {
        let app_data_dir = scratch_dir();
        let project_root = scratch_dir();
        let skill_dir = project_skills_dir(&project_root).join("so-deste-projeto");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: so-deste-projeto\ndescription: teste\n---\nconteudo\n",
        )
        .unwrap();

        assert!(list_skills(&app_data_dir, None).unwrap().is_empty());
        let with_project = list_skills(&app_data_dir, Some(&project_root)).unwrap();
        assert_eq!(with_project.len(), 1);
        assert_eq!(with_project[0].scope, "project");

        std::fs::remove_dir_all(&app_data_dir).ok();
        std::fs::remove_dir_all(&project_root).ok();
    }
}
