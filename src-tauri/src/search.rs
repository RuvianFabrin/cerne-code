//! Configuração de qual provider de busca na web o agente usa. Por padrão
//! (`Auto`) a busca sai via DuckDuckGo sem precisar de conta nem instalar
//! nada localmente — resolve o problema de antes, onde `web_search`
//! dependia de rodar um container SearXNG à parte. As opções de API (Brave
//! Search, Tavily) dão resultados melhores pra quem já tem uma chave, e
//! `Searxng` cobre quem já roda uma instância própria e quer continuar
//! usando ela.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "cerne";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchProviderKind {
    #[default]
    Auto,
    Brave,
    Tavily,
    Searxng,
    Serper,
    Exa,
    GoogleCse,
    Bing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default)]
    pub provider: SearchProviderKind,
    #[serde(default = "default_searxng_url")]
    pub searxng_url: String,
    /// Search Engine ID ("cx") do Google Programmable Search — não é
    /// segredo (é so um identificador do motor configurado no console do
    /// Google), então fica junto do resto do config, não no keyring.
    #[serde(default)]
    pub google_cse_id: String,
    /// Endpoint da Bing/Azure AI Search — configuravel porque recursos do
    /// Azure ganham um endpoint proprio por regiao/recurso, diferente do
    /// dominio fixo `api.bing.microsoft.com` da API classica.
    #[serde(default = "default_bing_endpoint")]
    pub bing_endpoint: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            provider: SearchProviderKind::default(),
            searxng_url: default_searxng_url(),
            google_cse_id: String::new(),
            bing_endpoint: default_bing_endpoint(),
        }
    }
}

fn default_searxng_url() -> String {
    "http://127.0.0.1:8888".to_string()
}

fn default_bing_endpoint() -> String {
    "https://api.bing.microsoft.com/v7.0/search".to_string()
}

/// Um resultado de busca já normalizado — cada provider (`websearch.rs`)
/// devolve os campos do jeito que a API dele expõe; isso aqui é o formato
/// comum que a formatação final (texto pro modelo) consome.
#[derive(Debug, Clone)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Visão do config exposta pra tela — nunca inclui a chave de API em si,
/// só se uma está guardada ou não (mesmo padrão de `CustomProviderConfig`).
#[derive(Debug, Clone, Serialize)]
pub struct SearchConfigView {
    pub provider: SearchProviderKind,
    pub searxng_url: String,
    pub google_cse_id: String,
    pub bing_endpoint: String,
    pub has_brave_key: bool,
    pub has_tavily_key: bool,
    pub has_serper_key: bool,
    pub has_exa_key: bool,
    pub has_google_key: bool,
    pub has_bing_key: bool,
}

pub fn view(app_data_dir: &Path) -> SearchConfigView {
    let cfg = load_config(app_data_dir);
    SearchConfigView {
        provider: cfg.provider,
        searxng_url: cfg.searxng_url,
        google_cse_id: cfg.google_cse_id,
        bing_endpoint: cfg.bing_endpoint,
        has_brave_key: has_key(SearchProviderKind::Brave),
        has_tavily_key: has_key(SearchProviderKind::Tavily),
        has_serper_key: has_key(SearchProviderKind::Serper),
        has_exa_key: has_key(SearchProviderKind::Exa),
        has_google_key: has_key(SearchProviderKind::GoogleCse),
        has_bing_key: has_key(SearchProviderKind::Bing),
    }
}

fn search_config_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("search_config.json")
}

pub fn load_config(app_data_dir: &Path) -> SearchConfig {
    match std::fs::read_to_string(search_config_path(app_data_dir)) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => SearchConfig::default(),
    }
}

pub fn save_config(app_data_dir: &Path, config: &SearchConfig) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let text = serde_json::to_string_pretty(config)?;
    std::fs::write(search_config_path(app_data_dir), text)?;
    Ok(())
}

fn keyring_user(provider: SearchProviderKind) -> &'static str {
    match provider {
        SearchProviderKind::Brave => "search_brave_api_key",
        SearchProviderKind::Tavily => "search_tavily_api_key",
        SearchProviderKind::Serper => "search_serper_api_key",
        SearchProviderKind::Exa => "search_exa_api_key",
        SearchProviderKind::GoogleCse => "search_google_api_key",
        SearchProviderKind::Bing => "search_bing_api_key",
        SearchProviderKind::Auto | SearchProviderKind::Searxng => "search_unused",
    }
}

pub fn set_key(provider: SearchProviderKind, key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, keyring_user(provider))?;
    entry.set_password(key)?;
    Ok(())
}

pub fn get_key(provider: SearchProviderKind) -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, keyring_user(provider)).ok()?;
    entry.get_password().ok()
}

pub fn has_key(provider: SearchProviderKind) -> bool {
    get_key(provider).is_some()
}

pub fn clear_key(provider: SearchProviderKind) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, keyring_user(provider))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-search-config-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_config_is_auto_with_searxng_fallback_url() {
        let dir = scratch_dir();
        let cfg = load_config(&dir);
        assert_eq!(cfg.provider, SearchProviderKind::Auto);
        assert_eq!(cfg.searxng_url, "http://127.0.0.1:8888");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = scratch_dir();
        save_config(
            &dir,
            &SearchConfig {
                provider: SearchProviderKind::Brave,
                searxng_url: "http://example:9999".to_string(),
                google_cse_id: String::new(),
                bing_endpoint: default_bing_endpoint(),
            },
        )
        .unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.provider, SearchProviderKind::Brave);
        assert_eq!(loaded.searxng_url, "http://example:9999");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_config_without_searxng_url_field_still_deserializes() {
        let dir = scratch_dir();
        std::fs::write(search_config_path(&dir), r#"{"provider":"tavily"}"#).unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.provider, SearchProviderKind::Tavily);
        assert_eq!(loaded.searxng_url, "http://127.0.0.1:8888");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_config_without_new_provider_fields_still_deserializes() {
        let dir = scratch_dir();
        std::fs::write(
            search_config_path(&dir),
            r#"{"provider":"brave","searxng_url":"http://127.0.0.1:8888"}"#,
        )
        .unwrap();
        let loaded = load_config(&dir);
        assert_eq!(loaded.google_cse_id, "");
        assert_eq!(loaded.bing_endpoint, "https://api.bing.microsoft.com/v7.0/search");
        std::fs::remove_dir_all(&dir).ok();
    }
}
