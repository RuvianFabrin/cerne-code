use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Distingue as duas formas de uso de um `Persona` na UI (pedido explícito
/// do usuário, 2026-08-18): mesmo dado/mecanismo por baixo
/// (`system_prompt_override` + filtro de tools/skills), mas o PROPÓSITO do
/// conteúdo é diferente o suficiente pra merecer abas separadas — "Agente"
/// é orquestrador (passos explícitos: "use a ferramenta X, depois Y, leia
/// o arquivo da pasta W e mova pra pasta R"), "Persona" é mais
/// especialista/professor (tom e conhecimento, ex: "Especialista em inglês,
/// ensina pra profissional de TI, com muitos anos de experiência"). Default
/// `Persona` — todo registro salvo antes desse campo existir era exatamente
/// esse uso (a aba se chamava só "Personas").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PersonaKind {
    Agent,
    #[default]
    Persona,
}

/// Um "prompt pronto" cadastrado pelo usuário para entrar como system prompt
/// (ex: "Você é um revisor de código Rust sênior, focado em segurança").
/// Diferente de uma skill (que o LLM carrega sob demanda via `load_skill`
/// quando decide que é relevante), uma persona é escolhida pelo usuário no
/// composer e vale pra sessão inteira desde o próximo turno — mesmo
/// mecanismo de "flag de sessão injetada no prompt" que o Método Fable já
/// usa (`Session.fable_method`), mas com conteúdo livre e múltiplas opções
/// em vez de um único texto fixo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub id: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub kind: PersonaKind,
    /// Allowlist de ferramentas — Fase A3 do roteiro de Agentes/Skills
    /// (`PLANOS/13_roteiro_agentes_skills_fases.md`), completando o gap
    /// documentado em T24 ("falta tools/model/skills por agente"). Vazio
    /// (default, inclusive pra personas salvas antes desse campo existir —
    /// `serde(default)` cobre isso) = sem restrição, comportamento IDÊNTICO
    /// a antes. Só filtra o toolset da sessão quando o usuário preenche essa
    /// lista explicitamente na edição da persona.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Allowlist de skills que essa persona pode carregar via `load_skill` —
    /// completa o gap "skills por agente" da Fase A3/B3 (roteiro de
    /// Agentes/Skills). Mesma semântica de `tools`: vazio (default,
    /// retrocompatível) = sem restrição, qualquer skill do catálogo pode ser
    /// carregada normalmente.
    #[serde(default)]
    pub skills: Vec<String>,
}

fn personas_file(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("personas.json")
}

pub fn list_personas(app_data_dir: &Path) -> Result<Vec<Persona>> {
    let file = personas_file(app_data_dir);
    if !file.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&file)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&text)?)
}

fn save_personas(app_data_dir: &Path, personas: &[Persona]) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    std::fs::write(
        personas_file(app_data_dir),
        serde_json::to_string_pretty(personas)?,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn create_persona(
    app_data_dir: &Path,
    name: &str,
    content: &str,
    tools: Vec<String>,
    skills: Vec<String>,
    kind: PersonaKind,
) -> Result<Persona> {
    let mut personas = list_personas(app_data_dir)?;
    let persona = Persona {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        content: content.trim().to_string(),
        tools,
        skills,
        kind,
    };
    personas.push(persona.clone());
    save_personas(app_data_dir, &personas)?;
    Ok(persona)
}

#[allow(clippy::too_many_arguments)]
pub fn update_persona(
    app_data_dir: &Path,
    id: &str,
    name: &str,
    content: &str,
    tools: Vec<String>,
    skills: Vec<String>,
    kind: PersonaKind,
) -> Result<Persona> {
    let mut personas = list_personas(app_data_dir)?;
    let persona = personas
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| anyhow!("persona '{id}' nao encontrada"))?;
    persona.name = name.trim().to_string();
    persona.content = content.trim().to_string();
    persona.tools = tools;
    persona.skills = skills;
    persona.kind = kind;
    let updated = persona.clone();
    save_personas(app_data_dir, &personas)?;
    Ok(updated)
}

pub fn delete_persona(app_data_dir: &Path, id: &str) -> Result<()> {
    let mut personas = list_personas(app_data_dir)?;
    let before = personas.len();
    personas.retain(|p| p.id != id);
    if personas.len() == before {
        return Err(anyhow!("persona '{id}' nao encontrada"));
    }
    save_personas(app_data_dir, &personas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-personas-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_then_list_roundtrip() {
        let dir = scratch_dir();
        let p = create_persona(&dir, "Revisor Rust", "Voce e um revisor de codigo Rust senior.", vec![], vec![], PersonaKind::Persona).unwrap();
        let found = list_personas(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, p.id);
        assert_eq!(found[0].name, "Revisor Rust");
        assert!(found[0].tools.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_changes_name_and_content() {
        let dir = scratch_dir();
        let p = create_persona(&dir, "Nome", "Conteudo", vec![], vec![], PersonaKind::Persona).unwrap();
        let updated = update_persona(&dir, &p.id, "Novo nome", "Novo conteudo", vec![], vec![], PersonaKind::Persona).unwrap();
        assert_eq!(updated.name, "Novo nome");
        let found = list_personas(&dir).unwrap();
        assert_eq!(found[0].content, "Novo conteudo");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_removes_persona() {
        let dir = scratch_dir();
        let p = create_persona(&dir, "Nome", "Conteudo", vec![], vec![], PersonaKind::Persona).unwrap();
        delete_persona(&dir, &p.id).unwrap();
        assert!(list_personas(&dir).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_missing_persona_errors() {
        let dir = scratch_dir();
        assert!(update_persona(&dir, "nao-existe", "x", "y", vec![], vec![], PersonaKind::Persona).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn kind_defaults_to_persona_for_old_data_and_roundtrips_agent() {
        let dir = scratch_dir();
        let agent = create_persona(
            &dir,
            "Organizador",
            "Use list_dir na pasta W, depois move cada arquivo pra pasta R.",
            vec![],
            vec![],
            PersonaKind::Agent,
        )
        .unwrap();
        assert_eq!(agent.kind, PersonaKind::Agent);
        let found = list_personas(&dir).unwrap();
        assert_eq!(found[0].kind, PersonaKind::Agent);

        // Registro salvo antes do campo `kind` existir (JSON sem essa chave)
        // precisa carregar como Persona (era o unico uso que existia antes).
        std::fs::write(
            personas_file(&dir),
            format!(r#"[{{"id":"{}","name":"Antiga","content":"sem campo kind"}}]"#, agent.id),
        )
        .unwrap();
        let found_old = list_personas(&dir).unwrap();
        assert_eq!(found_old[0].kind, PersonaKind::Persona);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tools_allowlist_roundtrips_and_defaults_empty_for_old_data() {
        let dir = scratch_dir();
        let p = create_persona(
            &dir,
            "Pesquisador",
            "Voce so pesquisa, nunca edita nada.",
            vec!["web_search".to_string(), "web_fetch".to_string()],
            vec![],
            PersonaKind::Persona,
        )
        .unwrap();
        let found = list_personas(&dir).unwrap();
        assert_eq!(found[0].tools, vec!["web_search", "web_fetch"]);

        // Persona salva antes do campo `tools` existir (JSON sem essa chave)
        // precisa continuar carregando normalmente, com tools vazio.
        std::fs::write(
            personas_file(&dir),
            format!(
                r#"[{{"id":"{}","name":"Antiga","content":"sem campo tools"}}]"#,
                p.id
            ),
        )
        .unwrap();
        let found_old = list_personas(&dir).unwrap();
        assert_eq!(found_old.len(), 1);
        assert!(found_old[0].tools.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skills_allowlist_roundtrips_and_defaults_empty_for_old_data() {
        let dir = scratch_dir();
        let p = create_persona(
            &dir,
            "Redator",
            "Voce so escreve textos, nunca mexe em codigo.",
            vec![],
            vec!["summarize".to_string()],
            PersonaKind::Persona,
        )
        .unwrap();
        let found = list_personas(&dir).unwrap();
        assert_eq!(found[0].skills, vec!["summarize"]);

        // Persona salva antes do campo `skills` existir (JSON sem essa chave,
        // mesmo teste do `tools` acima) precisa continuar carregando normalmente.
        std::fs::write(
            personas_file(&dir),
            format!(
                r#"[{{"id":"{}","name":"Antiga","content":"sem campo skills"}}]"#,
                p.id
            ),
        )
        .unwrap();
        let found_old = list_personas(&dir).unwrap();
        assert_eq!(found_old.len(), 1);
        assert!(found_old[0].skills.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }
}
