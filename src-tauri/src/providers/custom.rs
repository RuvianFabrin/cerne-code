//! Providers customizados: qualquer endpoint compatível com a API de chat
//! completions da OpenAI que o usuário configura na tela de Configurações —
//! nome, URL base, chave de API. Cobre Claude (via seu shim OpenAI-compatible
//! em `https://api.anthropic.com/v1/`), Grok/xAI (`https://api.x.ai/v1`),
//! ChatGPT/OpenAI (`https://api.openai.com/v1`), Kimi/Moonshot
//! (`https://api.moonshot.ai/v1`), Qwen/DashScope
//! (`https://dashscope-intl.aliyuncs.com/compatible-mode/v1`), ou qualquer
//! outro — sem hardcodar nenhum desses por nome, já que o Cerne é
//! distribuído open source e não pode assumir qual provider terceiro o
//! usuário vai usar. `providers::chat_stream`/`list_models` já falam esse
//! formato genericamente (mesmo código usado por OpenRouter/Ollama/LM
//! Studio), então nenhum código específico de vendor é necessário aqui.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "cerne";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomProviderConfig {
    pub id: String,
    pub label: String,
    pub base_url: String,
    /// Confirmação manual do usuário de que os modelos desta conexão aceitam
    /// imagem — provider customizado genérico não tem um jeito padrão de
    /// perguntar isso pra API (ver `providers::supports_vision`), então por
    /// padrão (`false`) o Cerne Code bloqueia envio de imagem pra qualquer
    /// provider customizado, mesmo que o modelo real aceite. Marcar isso é o
    /// usuário assumindo que sabe que o modelo escolhido tem visão.
    #[serde(default)]
    pub supports_vision: bool,
    /// Override manual do tamanho de contexto (em tokens), usado quando o
    /// endpoint `/models` desta conexão não devolve um campo de contexto
    /// utilizável — a maioria dos providers OpenAI-compatible genéricos
    /// (Claude via shim, Qwen/DashScope, Kimi/Moonshot) não expõe isso no
    /// formato padrão `/models` (só o `/models` do OpenRouter tem esse campo
    /// a mais). Sem isso, `DEFAULT_CONTEXT_LENGTH` (8192) é usado como
    /// fallback, o que fica bem menor que o real pra modelos modernos.
    #[serde(default)]
    pub context_length: Option<u32>,
}

fn custom_providers_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("custom_providers.json")
}

#[derive(Serialize, Deserialize, Default)]
struct StoredConfig {
    #[serde(default)]
    providers: Vec<CustomProviderConfig>,
}

pub fn load_providers(app_data_dir: &Path) -> Result<Vec<CustomProviderConfig>> {
    let path = custom_providers_path(app_data_dir);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let stored: StoredConfig =
        serde_json::from_str(&text).map_err(|e| anyhow!("custom_providers.json invalido: {e}"))?;
    Ok(stored.providers)
}

pub fn save_providers(app_data_dir: &Path, providers: &[CustomProviderConfig]) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let text = serde_json::to_string_pretty(&StoredConfig {
        providers: providers.to_vec(),
    })?;
    std::fs::write(custom_providers_path(app_data_dir), text)?;
    Ok(())
}

/// Adiciona (ou atualiza, se o `id` já existir) um provider customizado.
pub fn add_provider(
    app_data_dir: &Path,
    provider: CustomProviderConfig,
) -> Result<Vec<CustomProviderConfig>> {
    let mut providers = load_providers(app_data_dir)?;
    providers.retain(|p| p.id != provider.id);
    providers.push(provider);
    save_providers(app_data_dir, &providers)?;
    Ok(providers)
}

/// Remove o provider E a chave de API dele do cofre de credenciais — senão a
/// chave fica órfã no keyring do SO pra sempre, sem nenhuma UI que a alcance
/// de novo (nem `list_providers` mostra chaves de provider já removido).
pub fn remove_provider(app_data_dir: &Path, id: &str) -> Result<Vec<CustomProviderConfig>> {
    let mut providers = load_providers(app_data_dir)?;
    providers.retain(|p| p.id != id);
    save_providers(app_data_dir, &providers)?;
    let _ = clear_key(id);
    Ok(providers)
}

fn keyring_user(id: &str) -> String {
    format!("custom_provider_{id}")
}

pub fn set_key(id: &str, key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_user(id))?;
    entry.set_password(key)?;
    Ok(())
}

pub fn get_key(id: &str) -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_user(id)).ok()?;
    entry.get_password().ok()
}

pub fn has_key(id: &str) -> bool {
    get_key(id).is_some()
}

fn clear_key(id: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_user(id))?;
    // `delete_credential` erra se a entrada nao existe - nao ha chave
    // configurada pra remover e o resultado (nao ha chave sobrando) e o
    // mesmo, entao ignora esse caso especifico.
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cerne-custom-provider-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(id: &str) -> CustomProviderConfig {
        CustomProviderConfig {
            id: id.to_string(),
            label: format!("Provider {id}"),
            base_url: format!("https://api.{id}.example/v1"),
            supports_vision: false,
            context_length: None,
        }
    }

    #[test]
    fn load_providers_is_empty_when_file_missing() {
        let dir = scratch_dir();
        assert!(load_providers(&dir).unwrap().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_provider_then_load_roundtrips() {
        let dir = scratch_dir();
        add_provider(&dir, sample("claude")).unwrap();
        let loaded = load_providers(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "claude");
        assert_eq!(loaded[0].base_url, "https://api.claude.example/v1");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_provider_upserts_by_id_instead_of_duplicating() {
        let dir = scratch_dir();
        add_provider(&dir, sample("grok")).unwrap();
        let mut updated = sample("grok");
        updated.label = "Renomeado".to_string();
        let providers = add_provider(&dir, updated).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].label, "Renomeado");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_provider_deletes_only_the_matching_id() {
        let dir = scratch_dir();
        add_provider(&dir, sample("keep")).unwrap();
        add_provider(&dir, sample("drop")).unwrap();
        let remaining = remove_provider(&dir, "drop").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "keep");
        fs::remove_dir_all(&dir).ok();
    }
}
