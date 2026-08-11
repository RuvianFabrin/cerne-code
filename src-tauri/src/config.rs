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

/// Prévia mascarada da chave salva (início + fim, meio escondido) — a UI usa
/// isso pra confirmar visualmente "essa é a chave que eu colei" sem expor o
/// valor inteiro de volta pro frontend, já que o cofre de credenciais existe
/// justamente pra não guardar a chave em texto puro fora dele.
pub fn openrouter_key_preview() -> Option<String> {
    get_openrouter_key().map(|k| mask_key(&k))
}

pub fn clear_openrouter_key() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Mostra os 6 primeiros e os 4 últimos caracteres, com "…" no meio — dá pra
/// reconhecer a chave (prefixo do provider + sufixo distintivo) sem revelar
/// o suficiente pra alguém copiar e usar.
fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 12 {
        return "•".repeat(chars.len().max(4));
    }
    let prefix: String = chars[..6].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}…{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_key_shows_prefix_and_suffix_for_long_keys() {
        assert_eq!(
            mask_key("sk-or-v1-abcdef1234567890"),
            "sk-or-…7890"
        );
    }

    #[test]
    fn mask_key_fully_hides_short_keys() {
        // Chave curta demais pra sobrar meio escondido de verdade - esconde
        // tudo em vez de vazar quase o valor inteiro.
        assert_eq!(mask_key("short-key"), "•••••••••");
    }
}
