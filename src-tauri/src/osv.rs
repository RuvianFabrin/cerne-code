// Item 6 da lista de ideias do Hermes Agent (14_backlog_pendente.md): checagem
// de vulnerabilidade de dependência via a API pública do OSV.dev
// (https://osv.dev), sem precisar de conta/chave. Cobre os 3 formatos de
// manifesto mais comuns achados num projeto (npm, cargo, pip) — não tenta
// resolver a árvore de dependências transitivas (isso exigiria um resolver
// de verdade por ecossistema), só as dependências DIRETAS declaradas no
// manifesto, que já é o que mais importa pra um aviso rápido no chat.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ManifestDep {
    pub ecosystem: &'static str,
    pub name: String,
    pub version: String,
}

/// Máximo de dependências consultadas numa chamada — projetos com manifesto
/// gigante (ex: `package.json` de monorepo) não devem disparar centenas de
/// requisições HTTP sequenciais.
const MAX_DEPS_QUERIED: usize = 60;

pub fn collect_dependencies(project_root: &Path) -> Vec<ManifestDep> {
    let mut deps = Vec::new();
    if let Ok(content) = std::fs::read_to_string(project_root.join("package.json")) {
        deps.extend(parse_package_json(&content));
    }
    if let Ok(content) = std::fs::read_to_string(project_root.join("Cargo.toml")) {
        deps.extend(parse_cargo_toml(&content));
    }
    if let Ok(content) = std::fs::read_to_string(project_root.join("requirements.txt")) {
        deps.extend(parse_requirements_txt(&content));
    }
    deps
}

/// Tira prefixo de range semver (`^`, `~`, `>=`, etc.) — a API do OSV quer
/// uma versão exata pra checar contra os intervalos afetados de cada
/// vulnerabilidade; não dá pra resolver "qual versão exata o `^1.2.0`
/// realmente instalou" sem ler o lockfile (fora de escopo aqui), então usamos
/// o número declarado no manifesto mesmo — falso-negativo é possível (a
/// versão real instalada pode ser mais nova que corrigiu o problema), mas
/// ainda é um sinal útil sem precisar de lockfile.
fn strip_version_prefix(v: &str) -> String {
    v.trim()
        .trim_start_matches(['^', '~', '=', '>', '<', ' '])
        .trim()
        .to_string()
}

fn parse_package_json(content: &str) -> Vec<ManifestDep> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for field in ["dependencies", "devDependencies"] {
        if let Some(obj) = json.get(field).and_then(|v| v.as_object()) {
            for (name, version) in obj {
                let Some(version) = version.as_str() else { continue };
                // Alias tipo "workspace:*" ou "file:../foo" não tem versão
                // publicada resolvível pelo OSV — pula.
                if version.contains(':') {
                    continue;
                }
                deps.push(ManifestDep {
                    ecosystem: "npm",
                    name: name.clone(),
                    version: strip_version_prefix(version),
                });
            }
        }
    }
    deps
}

fn parse_cargo_toml(content: &str) -> Vec<ManifestDep> {
    let Ok(toml) = content.parse::<toml::Value>() else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = toml.get(section).and_then(|v| v.as_table()) {
            for (name, spec) in table {
                let version = match spec {
                    toml::Value::String(v) => Some(v.clone()),
                    toml::Value::Table(t) => {
                        // Dependência de caminho local ou git não tem versão
                        // publicada no crates.io — pula.
                        if t.contains_key("path") || t.contains_key("git") {
                            None
                        } else {
                            t.get("version").and_then(|v| v.as_str()).map(String::from)
                        }
                    }
                    _ => None,
                };
                if let Some(version) = version {
                    deps.push(ManifestDep {
                        ecosystem: "crates.io",
                        name: name.clone(),
                        version: strip_version_prefix(&version),
                    });
                }
            }
        }
    }
    deps
}

fn parse_requirements_txt(content: &str) -> Vec<ManifestDep> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        // Formatos aceitos: "nome==1.2.3", "nome>=1.2.3" (usa a versão do
        // range mesmo). Ignora linhas sem versão exata (ex: "nome" sozinho,
        // "-r outro.txt") — não dá pra checar sem saber a versão.
        for sep in ["==", ">=", "~="] {
            if let Some((name, version)) = line.split_once(sep) {
                let version = version.split(&[';', ' ', '#'][..]).next().unwrap_or(version);
                deps.push(ManifestDep {
                    ecosystem: "PyPI",
                    name: name.trim().to_string(),
                    version: version.trim().to_string(),
                });
                break;
            }
        }
    }
    deps
}

#[derive(Deserialize)]
struct OsvQueryResponse {
    #[serde(default)]
    vulns: Vec<OsvVuln>,
}

#[derive(Deserialize)]
struct OsvVuln {
    id: String,
    #[serde(default)]
    summary: Option<String>,
}

async fn query_one(client: &reqwest::Client, dep: &ManifestDep) -> Result<Vec<OsvVuln>> {
    let body = serde_json::json!({
        "package": { "name": dep.name, "ecosystem": dep.ecosystem },
        "version": dep.version,
    });
    let resp = client
        .post("https://api.osv.dev/v1/query")
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json::<OsvQueryResponse>()
        .await?;
    Ok(resp.vulns)
}

/// Consulta o OSV.dev pra cada dependência achada no projeto e devolve um
/// resumo em texto pronto pra mostrar no chat. Erro de rede numa dependência
/// individual não derruba a checagem inteira — só é reportado como "não foi
/// possível checar" pra aquele pacote.
pub async fn check_project(project_root: &Path) -> Result<String> {
    let mut deps = collect_dependencies(project_root);
    if deps.is_empty() {
        return Ok(
            "Nenhum manifesto de dependências reconhecido (package.json/Cargo.toml/\
             requirements.txt) encontrado na raiz do projeto."
                .to_string(),
        );
    }
    let truncated = deps.len() > MAX_DEPS_QUERIED;
    deps.truncate(MAX_DEPS_QUERIED);

    let client = reqwest::Client::new();
    let queries = deps.iter().map(|dep| query_one(&client, dep));
    let results = futures_util::future::join_all(queries).await;

    let mut vulnerable = Vec::new();
    let mut failed = Vec::new();
    for (dep, result) in deps.iter().zip(results) {
        match result {
            Ok(vulns) if !vulns.is_empty() => {
                let ids: Vec<String> = vulns
                    .iter()
                    .map(|v| {
                        let summary = v.summary.as_deref().unwrap_or("sem resumo");
                        format!("    - {} — {}", v.id, summary)
                    })
                    .collect();
                vulnerable.push(format!(
                    "  {} {}@{}:\n{}",
                    dep.ecosystem,
                    dep.name,
                    dep.version,
                    ids.join("\n")
                ));
            }
            Ok(_) => {}
            Err(_) => failed.push(format!("{}@{}", dep.name, dep.version)),
        }
    }

    let mut out = String::new();
    if vulnerable.is_empty() {
        out.push_str(&format!(
            "Nenhuma vulnerabilidade conhecida encontrada em {} dependência(s) checada(s).\n",
            deps.len()
        ));
    } else {
        out.push_str(&format!(
            "Vulnerabilidades conhecidas encontradas ({} de {} dependências afetadas):\n{}\n",
            vulnerable.len(),
            deps.len(),
            vulnerable.join("\n")
        ));
    }
    if truncated {
        out.push_str(&format!(
            "\nAviso: manifesto tem mais de {MAX_DEPS_QUERIED} dependências diretas — só as \
             primeiras {MAX_DEPS_QUERIED} foram checadas.\n"
        ));
    }
    if !failed.is_empty() {
        out.push_str(&format!(
            "\nNão foi possível checar (falha de rede): {}\n",
            failed.join(", ")
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_json_deps_and_dev_deps() {
        let content = r#"{
            "dependencies": { "vue": "^3.4.0", "pinia": "~2.1.0" },
            "devDependencies": { "typescript": "5.4.0" }
        }"#;
        let deps = parse_package_json(content);
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&ManifestDep {
            ecosystem: "npm",
            name: "vue".to_string(),
            version: "3.4.0".to_string(),
        }));
        assert!(deps.contains(&ManifestDep {
            ecosystem: "npm",
            name: "typescript".to_string(),
            version: "5.4.0".to_string(),
        }));
    }

    #[test]
    fn package_json_skips_workspace_and_file_aliases() {
        let content = r#"{
            "dependencies": { "local-pkg": "workspace:*", "other": "file:../other" }
        }"#;
        assert!(parse_package_json(content).is_empty());
    }

    #[test]
    fn parses_cargo_toml_simple_and_table_deps() {
        let content = r#"
[dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }
local-crate = { path = "../local-crate" }
"#;
        let deps = parse_cargo_toml(content);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&ManifestDep {
            ecosystem: "crates.io",
            name: "serde".to_string(),
            version: "1.0".to_string(),
        }));
        assert!(deps.contains(&ManifestDep {
            ecosystem: "crates.io",
            name: "tokio".to_string(),
            version: "1.35".to_string(),
        }));
    }

    #[test]
    fn parses_requirements_txt_pinned_versions() {
        let content = "requests==2.31.0\nflask>=3.0.0\n# comment\n-r other.txt\nunpinned-pkg\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&ManifestDep {
            ecosystem: "PyPI",
            name: "requests".to_string(),
            version: "2.31.0".to_string(),
        }));
        assert!(deps.contains(&ManifestDep {
            ecosystem: "PyPI",
            name: "flask".to_string(),
            version: "3.0.0".to_string(),
        }));
    }

    #[test]
    fn collect_dependencies_returns_empty_without_manifests() {
        let dir = std::env::temp_dir().join(format!("osv_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(collect_dependencies(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
