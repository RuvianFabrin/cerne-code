use crate::models::AppConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "cerne";
const KEYRING_USER: &str = "openrouter_api_key";

fn config_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("config.json")
}

/// Mapa provider_key -> lista de ids de modelos favoritos. A `provider_key`
/// identifica a conexão, não só o kind: "openrouter"/"ollama"/"lm_studio"
/// pros embutidos, "llama_cpp:{fork}" e "custom:{id}" pros que têm múltiplas
/// conexões — mesma ideia do cache de modelos no frontend.
type ModelFavorites = HashMap<String, Vec<String>>;

fn model_favorites_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("model_favorites.json")
}

pub fn load_model_favorites(app_data_dir: &PathBuf) -> ModelFavorites {
    let path = model_favorites_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => ModelFavorites::default(),
    }
}

pub fn save_model_favorites(app_data_dir: &PathBuf, favorites: &ModelFavorites) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let path = model_favorites_path(app_data_dir);
    let text = serde_json::to_string_pretty(favorites)?;
    std::fs::write(path, text)?;
    Ok(())
}

pub fn load_config(app_data_dir: &PathBuf) -> AppConfig {
    let path = config_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(app_data_dir: &PathBuf, config: &AppConfig) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let path = config_path(app_data_dir);
    let text = serde_json::to_string_pretty(config)?;
    std::fs::write(path, text)?;
    Ok(())
}

pub fn set_openrouter_key(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
    entry.set_password(key)?;
    Ok(())
}

pub fn get_openrouter_key() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    entry.get_password().ok()
}

pub fn has_openrouter_key() -> bool {
    get_openrouter_key().is_some()
}
