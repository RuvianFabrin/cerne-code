use crate::models::AppConfig;
use anyhow::Result;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "cerne";
const KEYRING_USER: &str = "openrouter_api_key";

fn config_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("config.json")
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
