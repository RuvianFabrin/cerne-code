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

/// Skills de exemplo embarcadas no binário, semeadas só na primeira vez que
/// o app roda (junto do `_README.md`) — Fase B2 do roteiro de Agentes/Skills
/// (`PLANOS/13_roteiro_agentes_skills_fases.md`), adaptadas a partir das
/// skills do picoClaw (`weather`/`skill-creator` do repo original) pro
/// formato e ferramentas reais do Cerne (sem depender de `curl`/wttr.in como
/// o picoClaw fazia — usa `web_search`/`web_fetch`, que o Cerne já tem
/// embutido). `summarize` é nova, escrita do zero pro mesmo padrão. Fecha
/// de brinde a tarefa pendente da Fase 1 "skills de exemplo embarcadas".
/// Fica de fora de propósito: `github`/`tmux` (baixa prioridade
/// Windows-first) e `hardware`/`agent-browser`/`picoclaw-agent` (específicas
/// do domínio do picoClaw, não aproveitáveis como estão).
///
/// `file-organizer`/`email-triage` vieram da Fase 4 (skills de
/// produtividade) — mesma decisão de semear (skill só é lida quando o LLM
/// decide carregar, custo de "poluir" é baixo, diferente de Persona que
/// aparece na lista de escolha ativa). `english-tutor` (a 3ª ideia da Fase
/// 4) ficou de fora de propósito: é uma Persona (modo de sessão inteira),
/// não skill — Personas de exemplo são deliberadamente NÃO semeadas (ver
/// decisão registrada na Fase 2 do roteiro), só documentadas na ajuda.
///
/// `project-manager` veio da Fase 5 (multi-agente gerenciado) — único item
/// real que faltava depois de confirmar que toda a infraestrutura de
/// delegação dinâmica (catálogo de skills no prompt, `task` em paralelo via
/// API, rastreamento em tempo real) já existia. É skill, não Persona: a
/// decisão de "coordenar um objetivo grande delegando sub-tarefas" é pontual
/// pra um pedido específico, não um modo que deveria valer a sessão inteira.
const EXAMPLE_SKILLS: &[(&str, &str)] = &[
    ("weather", include_str!("skills_examples/weather.md")),
    ("summarize", include_str!("skills_examples/summarize.md")),
    (
        "skill-creator",
        include_str!("skills_examples/skill-creator.md"),
    ),
    (
        "file-organizer",
        include_str!("skills_examples/file_organizer.md"),
    ),
    (
        "email-triage",
        include_str!("skills_examples/email_triage.md"),
    ),
    (
        "project-manager",
        include_str!("skills_examples/project_manager.md"),
    ),
];

/// Slugs que faziam parte de `EXAMPLE_SKILLS` quando o mecanismo de
/// "semear uma vez" ainda dependia só da existência do `_README.md` (antes
/// de `_seeded.json` existir) — usado só pra migrar instalação já existente
/// sem forçar de volta um exemplo que o usuário já tinha deletado de
/// propósito (ver bug abaixo).
const LEGACY_SEEDED_SLUGS: &[&str] = &["weather", "summarize", "skill-creator"];

/// Ensures the global skills folder exists, writing a short README and
/// example skills so the user has something to look at (and a few working
/// examples to copy from) when they open it.
///
/// Cada slug de `EXAMPLE_SKILLS` é semeado no máximo UMA VEZ (rastreado em
/// `_seeded.json`, não pela existência da pasta — se o usuário deletar um
/// exemplo de propósito, ele não deve voltar sozinho na próxima abertura).
/// **Bug real corrigido em 2026-08-17** (achado testando ao vivo,
/// `file-organizer`/`email-triage`/`project-manager` não apareciam pro
/// usuário): a versão antiga gatilhava TODO o seeding só na existência do
/// `_README.md` — então qualquer instalação que já tinha rodado o Cerne
/// ANTES desses 3 exemplos serem adicionados a `EXAMPLE_SKILLS` (Fase 4/5)
/// nunca os recebia, já que o README já existia e o bloco de seeding nem
/// rodava de novo. Rastrear por slug individual corrige isso: instalação
/// existente ganha só os exemplos que ainda não tinha (backfill), sem
/// re-seedar os que já tinha antes (mesmo que o usuário tenha deletado
/// algum deles) — e continua funcionando do mesmo jeito pra qualquer
/// exemplo novo que for adicionado no futuro, sem precisar de outro fix.
pub fn ensure_global_skills_dir(app_data_dir: &Path) -> Result<PathBuf> {
    let dir = global_skills_dir(app_data_dir);
    std::fs::create_dir_all(&dir)?;
    let readme = dir.join("_README.md");
    let is_fresh_install = !readme.exists();
    if is_fresh_install {
        std::fs::write(&readme, README_TEMPLATE)?;
    }

    let seeded_path = dir.join("_seeded.json");
    let mut seeded: Vec<String> = if seeded_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&seeded_path)?).unwrap_or_default()
    } else if is_fresh_install {
        Vec::new()
    } else {
        // Instalação de antes de `_seeded.json` existir: trata os exemplos
        // originais como já oferecidos, só os novos entram como backfill.
        LEGACY_SEEDED_SLUGS.iter().map(|s| s.to_string()).collect()
    };

    let mut changed = false;
    for (slug, content) in EXAMPLE_SKILLS {
        if seeded.iter().any(|s| s == slug) {
            continue;
        }
        let skill_dir = dir.join(slug);
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(skill_dir.join("SKILL.md"), content)?;
        seeded.push(slug.to_string());
        changed = true;
    }
    if changed {
        std::fs::write(&seeded_path, serde_json::to_string_pretty(&seeded)?)?;
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

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
}

pub fn create_skill(
    app_data_dir: &Path,
    name: &str,
    description: &str,
    language: SkillLanguage,
) -> Result<PathBuf> {
    let slug = slugify(name);
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

/// Fase 6 (skill store, Degrau 1 do roteiro — "importar skill de URL", sem
/// registry/hospedagem nova nenhuma): busca o conteúdo cru de uma URL
/// (assume um `SKILL.md` com frontmatter `name`/`description`) pro
/// `SkillImportModal.vue` mostrar em preview ANTES de salvar — nunca
/// carregado/usado direto sem o usuário ver o texto primeiro, já que skill
/// é texto lido pelo LLM e o risco real de importar de terceiro é prompt
/// injection, não "permissão" (a skill não ganha nenhuma capacidade nova).
///
/// Reaproveita a mesma validação de URL pública que `websearch::fetch` já
/// usa (bloqueia localhost/rede interna) — a URL aqui vem de input livre do
/// usuário no composer, mesmo nível de confiança.
pub async fn fetch_skill_from_url(url: &str) -> Result<String> {
    let parsed = crate::agent::websearch::validate_public_url(url).await?;
    let client = reqwest::Client::new();
    let resp = client
        .get(parsed)
        .header("User-Agent", "Mozilla/5.0 (compatible; Cerne/0.1)")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| anyhow!("falha ao buscar {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("{url} respondeu {}", resp.status()));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| anyhow!("resposta invalida de {url}: {e}"))?;
    // Teto de tamanho — uma URL apontando pra algo gigante nao deveria travar
    // a tela nem inflar o preview a toa.
    Ok(text.chars().take(200_000).collect())
}

/// Cria uma skill nova a partir de conteúdo já pronto (frontmatter + corpo),
/// em vez do template em branco que `create_skill` gera — usado depois do
/// usuário revisar o preview de `fetch_skill_from_url` e confirmar o
/// import. Exige um `name` válido no frontmatter; falha se já existir uma
/// skill com esse slug (mesma regra de `create_skill`).
pub fn import_skill(app_data_dir: &Path, content: &str) -> Result<PathBuf> {
    let (frontmatter, _) = split_frontmatter(content);
    let name = frontmatter.get("name").cloned().ok_or_else(|| {
        anyhow!(
            "o conteudo importado nao tem um campo 'name' no frontmatter (---\\nname: ...\\n---) \
             - confira se a URL aponta pra um SKILL.md valido"
        )
    })?;
    let slug = slugify(&name);
    let dir = ensure_global_skills_dir(app_data_dir)?.join(&slug);
    if dir.exists() {
        return Err(anyhow!(
            "ja existe uma skill '{slug}' - edite a existente em vez de importar de novo"
        ));
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("SKILL.md"), content)?;
    Ok(dir)
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

        // Nao afirma o total (create_skill chama ensure_global_skills_dir por
        // baixo, que semeia as skills de exemplo na 1a chamada - ver
        // ensure_global_skills_dir_seeds_example_skills_once) - so confirma
        // que a skill criada esta la, com os campos certos.
        let found = list_skills(&app_data_dir, None).unwrap();
        let revisar_pr = found.iter().find(|s| s.name == "revisar-pr").unwrap();
        assert_eq!(revisar_pr.scope, "global");

        let body = load_skill_body(&app_data_dir, None, "revisar-pr").unwrap();
        assert!(body.contains("Passo a passo"));

        std::fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn ensure_global_skills_dir_seeds_example_skills_once() {
        let app_data_dir = scratch_dir();

        ensure_global_skills_dir(&app_data_dir).unwrap();
        let found = list_skills(&app_data_dir, None).unwrap();
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"weather"));
        assert!(names.contains(&"summarize"));
        assert!(names.contains(&"skill-creator"));
        assert!(names.contains(&"file-organizer"));
        assert!(names.contains(&"email-triage"));
        assert!(names.contains(&"project-manager"));

        // Usuario deleta um exemplo - chamar de novo (ex: proxima abertura
        // do app) nao deve trazer ele de volta, so semeia na 1a vez mesmo.
        std::fs::remove_dir_all(global_skills_dir(&app_data_dir).join("weather")).unwrap();
        ensure_global_skills_dir(&app_data_dir).unwrap();
        let found_again = list_skills(&app_data_dir, None).unwrap();
        assert!(!found_again.iter().any(|s| s.name == "weather"));

        std::fs::remove_dir_all(&app_data_dir).ok();
    }

    /// Bug real corrigido em 2026-08-17: instalacao de antes de
    /// `file-organizer`/`email-triage`/`project-manager` existirem em
    /// `EXAMPLE_SKILLS` (so tem `_README.md`, sem `_seeded.json`) precisa
    /// ganhar os exemplos novos na proxima abertura do app, sem re-semear
    /// os antigos que o usuario ja pode ter deletado.
    #[test]
    fn ensure_global_skills_dir_backfills_new_examples_for_pre_existing_install() {
        let app_data_dir = scratch_dir();
        let dir = global_skills_dir(&app_data_dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Simula uma instalacao antiga: so o README (marcador de "ja rodou
        // antes"), sem _seeded.json, e so os 3 exemplos originais no disco
        // (o usuario ja deletou "weather" de proposito).
        std::fs::write(dir.join("_README.md"), "old readme").unwrap();
        std::fs::create_dir_all(dir.join("summarize")).unwrap();
        std::fs::write(dir.join("summarize").join("SKILL.md"), "---\nname: summarize\n---\n").unwrap();

        ensure_global_skills_dir(&app_data_dir).unwrap();

        let found = list_skills(&app_data_dir, None).unwrap();
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        // Exemplos novos: backfilled.
        assert!(names.contains(&"file-organizer"));
        assert!(names.contains(&"email-triage"));
        assert!(names.contains(&"project-manager"));
        // "weather", que o usuario tinha deletado antes desse fix existir,
        // NAO deve voltar sozinho.
        assert!(!names.contains(&"weather"));

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

    #[test]
    fn import_skill_roundtrip() {
        let app_data_dir = scratch_dir();
        let content = "---\nname: Importada de Teste\ndescription: veio de uma URL\n---\n\nCorpo importado.\n";
        let dir = import_skill(&app_data_dir, content).unwrap();
        assert!(dir.join("SKILL.md").exists());

        // Nao afirma o total (import_skill tambem chama
        // ensure_global_skills_dir por baixo, que semeia as skills de
        // exemplo na 1a chamada) - so confirma que a skill importada esta
        // la, com os campos certos. O `name` listado e o valor CRU do
        // frontmatter importado ("Importada de Teste"), nao o slug do
        // diretorio ("importada-de-teste") - import_skill preserva o
        // conteudo exatamente como veio, so o nome da PASTA e slugificado.
        let found = list_skills(&app_data_dir, None).unwrap();
        let imported = found.iter().find(|s| s.name == "Importada de Teste").unwrap();
        assert_eq!(imported.description, "veio de uma URL");

        // Conteudo salvo deve ser exatamente o que foi importado, sem
        // reescrever/normalizar nada.
        let saved = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        assert_eq!(saved, content);

        std::fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn import_skill_without_name_frontmatter_errors() {
        let app_data_dir = scratch_dir();
        let content = "so um texto sem frontmatter nenhum";
        let result = import_skill(&app_data_dir, content);
        assert!(result.is_err(), "sem 'name' no frontmatter deveria falhar com erro claro");

        std::fs::remove_dir_all(&app_data_dir).ok();
    }

    #[test]
    fn import_skill_duplicate_slug_errors() {
        let app_data_dir = scratch_dir();
        let content = "---\nname: Duplicada\ndescription: x\n---\ncorpo\n";
        import_skill(&app_data_dir, content).unwrap();
        let result = import_skill(&app_data_dir, content);
        assert!(result.is_err(), "importar a mesma skill 2x deveria falhar, nao sobrescrever");

        std::fs::remove_dir_all(&app_data_dir).ok();
    }
}
