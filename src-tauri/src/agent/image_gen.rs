//! Geração de imagem via uma API local OpenAI-compatible
//! (`POST {base_url}/images/generations`, resposta `{"data": [{"b64_json": "..."}]}`) —
//! o mesmo contrato que ComfyUI/LocalAI e outros wrappers já falam, pensado
//! pra rodar contra um script Python próprio do usuário na frente de um
//! modelo local (Qwen-Image, Krea, etc — pedido do usuário, 2026-09-24).
//!
//! Devolve os PNGs como bytes crus; quem chama decide se salva em disco
//! (`save_images`) e/ou embute como `data:` URL pro chat (mesmo mecanismo
//! que `computer_use_screenshot` já usa pra imagem em resultado de
//! ferramenta, ver `agent::mod::tool_images`).

use anyhow::{anyhow, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};

pub struct GeneratedImage {
    pub bytes: Vec<u8>,
}

/// Um servidor de geração de imagem configurado pelo usuário (pasta do
/// projeto Python + qual modelo carregar) — mesma ideia do `LlamaForkConfig`
/// pro llama.cpp: o usuário cadastra uma vez em Configurações, escolhe na
/// hora de usar, e o Cerne sobe o processo sozinho (`uv run uvicorn`),
/// espera responder, e aponta `AppConfig.image_gen.base_url` pra ele.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenPreset {
    pub id: String,
    pub label: String,
    /// Pasta do projeto Python (onde ficam `server.py`/`pyproject.toml`) —
    /// `uv run uvicorn server:app` roda com esta pasta como cwd.
    pub working_dir: String,
    /// Valor de `MODEL_PIPELINE` pro `server.py` (ex: "zimage", "krea2",
    /// "qwen-image") — string livre, o Cerne não valida contra uma lista
    /// fixa, já que é o servidor do usuário que decide o que aceita.
    pub model_pipeline: String,
    /// Valor de `MODEL_PATH` — repo do Hugging Face ou caminho local.
    /// Vazio = deixa o `server.py` usar o próprio default dele.
    #[serde(default)]
    pub model_path: String,
    pub port: u16,
}

fn presets_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("image_gen_presets.json")
}

#[derive(Serialize, Deserialize, Default)]
struct StoredPresets {
    #[serde(default)]
    presets: Vec<ImageGenPreset>,
}

pub fn load_presets(app_data_dir: &Path) -> Result<Vec<ImageGenPreset>> {
    let path = presets_path(app_data_dir);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let stored: StoredPresets =
        serde_json::from_str(&text).map_err(|e| anyhow!("image_gen_presets.json invalido: {e}"))?;
    Ok(stored.presets)
}

pub fn save_presets(app_data_dir: &Path, presets: &[ImageGenPreset]) -> Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let text = serde_json::to_string_pretty(&StoredPresets { presets: presets.to_vec() })?;
    std::fs::write(presets_path(app_data_dir), text)?;
    Ok(())
}

/// Adiciona (ou atualiza, se o `id` já existir) um preset.
pub fn add_preset(app_data_dir: &Path, preset: ImageGenPreset) -> Result<Vec<ImageGenPreset>> {
    let mut presets = load_presets(app_data_dir)?;
    presets.retain(|p| p.id != preset.id);
    presets.push(preset);
    save_presets(app_data_dir, &presets)?;
    Ok(presets)
}

pub fn remove_preset(app_data_dir: &Path, id: &str) -> Result<Vec<ImageGenPreset>> {
    let mut presets = load_presets(app_data_dir)?;
    presets.retain(|p| p.id != id);
    save_presets(app_data_dir, &presets)?;
    Ok(presets)
}

const HEALTH_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const HEALTH_CHECK_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(400);

/// Sobe `uv run uvicorn server:app` na pasta do preset, com `MODEL_PIPELINE`/
/// `MODEL_PATH` passados por variável de ambiente — mesmo padrão de
/// `llama_cpp::start_server` (spawn + espera `/health` responder antes de
/// devolver). O processo em si sobe rápido (segundos); o MODELO em si só
/// carrega de verdade na primeira geração (lazy, ver `server.py`), então
/// esse timeout curto cobre só o boot do servidor HTTP, não o carregamento
/// do modelo.
pub async fn start_server(preset: &ImageGenPreset) -> Result<Child> {
    let working_dir = PathBuf::from(&preset.working_dir);
    if !working_dir.is_dir() {
        return Err(anyhow!("pasta do projeto nao encontrada: {}", preset.working_dir));
    }
    let mut child = Command::new("uv")
        .args(["run", "uvicorn", "server:app", "--host", "127.0.0.1", "--port"])
        .arg(preset.port.to_string())
        .current_dir(&working_dir)
        .env("MODEL_PIPELINE", &preset.model_pipeline)
        .env("MODEL_PATH", &preset.model_path)
        .env("PYTHONUNBUFFERED", "1")
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow!("nao foi possivel iniciar 'uv' — confira se esta instalado e no PATH: {e}"))?;

    if let Err(e) = wait_for_health(&mut child, preset.port).await {
        let _ = child.start_kill();
        return Err(e);
    }

    Ok(child)
}

async fn wait_for_health(child: &mut Child, port: u16) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}/health");
    let deadline = tokio::time::Instant::now() + HEALTH_CHECK_TIMEOUT;

    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(anyhow!(
                "o servidor encerrou sozinho logo depois de iniciar (codigo {}) — confira se a porta {port} ja esta em uso, se 'uv sync' ja rodou na pasta do projeto, ou os logs do processo",
                status.code().map(|c| c.to_string()).unwrap_or_else(|| "desconhecido".to_string())
            ));
        }

        if let Ok(resp) = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok(());
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow!(
                "o servidor nao respondeu em /health na porta {port} depois de {}s — pode estar travado, ou a porta ja esta ocupada por outro processo",
                HEALTH_CHECK_TIMEOUT.as_secs()
            ));
        }

        tokio::time::sleep(HEALTH_CHECK_POLL_INTERVAL).await;
    }
}

/// `n`: quantas imagens pedir na mesma chamada (a maioria dos servidores
/// aceita, mas ignora acima de algum teto próprio — não validado aqui).
pub async fn generate(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    prompt: &str,
    n: u32,
) -> Result<Vec<GeneratedImage>> {
    let url = format!("{}/images/generations", base_url.trim_end_matches('/'));
    let mut body = json!({
        "prompt": prompt,
        "n": n.max(1),
        "response_format": "b64_json",
    });
    if !model.trim().is_empty() {
        body["model"] = json!(model);
    }

    let client = reqwest::Client::new();
    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| anyhow!("nao foi possivel conectar em {url}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("geracao de imagem falhou ({status}): {text}"));
    }

    let value: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta nao e JSON valido: {e}"))?;

    let data = value
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("resposta sem o campo \"data\" (array) esperado"))?;

    if data.is_empty() {
        return Err(anyhow!("servidor nao devolveu nenhuma imagem"));
    }

    data.iter()
        .map(|item| {
            let b64 = item
                .get("b64_json")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("item da resposta sem \"b64_json\""))?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| anyhow!("b64_json invalido: {e}"))?;
            Ok(GeneratedImage { bytes })
        })
        .collect()
}

/// Grava as imagens em `<project_root>/generated_images/`, um arquivo por
/// imagem, nome derivado do prompt (sanitizado) + timestamp + índice —
/// nunca sobrescreve (o timestamp garante nome único mesmo pro mesmo prompt
/// gerado duas vezes). Devolve os caminhos absolutos, na mesma ordem.
pub fn save_images(project_root: &Path, prompt: &str, images: &[GeneratedImage]) -> Result<Vec<PathBuf>> {
    let dir = project_root.join("generated_images");
    std::fs::create_dir_all(&dir)?;
    let slug = slugify(prompt);
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    images
        .iter()
        .enumerate()
        .map(|(i, img)| {
            let filename = if images.len() > 1 {
                format!("{slug}-{timestamp}-{i}.png")
            } else {
                format!("{slug}-{timestamp}.png")
            };
            let path = dir.join(filename);
            std::fs::write(&path, &img.bytes)?;
            Ok(path)
        })
        .collect()
}

/// Nome de arquivo seguro a partir do prompt: minúsculas, só
/// alfanumérico/hífen, truncado — nunca deixa o prompt cru virar caminho
/// (evita separador de diretório, caracteres de controle, etc).
fn slugify(prompt: &str) -> String {
    let mut slug: String = prompt
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let truncated: String = slug.chars().take(40).collect();
    if truncated.is_empty() {
        "imagem".to_string()
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-image-gen-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_preset(id: &str) -> ImageGenPreset {
        ImageGenPreset {
            id: id.to_string(),
            label: "Z-Image-Turbo".to_string(),
            working_dir: r"F:\geraImagemLocal".to_string(),
            model_pipeline: "zimage".to_string(),
            model_path: r"F:\geraImagemLocal\z-image-turbo".to_string(),
            port: 8000,
        }
    }

    #[test]
    fn load_presets_vazio_sem_arquivo() {
        let dir = scratch_dir();
        assert!(load_presets(&dir).unwrap().is_empty());
    }

    #[test]
    fn add_preset_depois_load_faz_roundtrip() {
        let dir = scratch_dir();
        add_preset(&dir, sample_preset("zimage")).unwrap();
        let loaded = load_presets(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "zimage");
        assert_eq!(loaded[0].port, 8000);
    }

    #[test]
    fn add_preset_com_mesmo_id_substitui() {
        let dir = scratch_dir();
        add_preset(&dir, sample_preset("zimage")).unwrap();
        let mut updated = sample_preset("zimage");
        updated.port = 9000;
        add_preset(&dir, updated).unwrap();
        let loaded = load_presets(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].port, 9000);
    }

    #[test]
    fn remove_preset_tira_so_o_id_pedido() {
        let dir = scratch_dir();
        add_preset(&dir, sample_preset("zimage")).unwrap();
        add_preset(&dir, sample_preset("krea2")).unwrap();
        let remaining = remove_preset(&dir, "zimage").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "krea2");
    }

    #[test]
    fn slugify_letras_numeros_e_espacos() {
        assert_eq!(slugify("Um Gato Laranja"), "um-gato-laranja");
    }

    #[test]
    fn slugify_colapsa_hifens_repetidos() {
        assert_eq!(slugify("a!!!b   c"), "a-b-c");
    }

    #[test]
    fn slugify_trunca_prompts_longos() {
        let prompt = "a".repeat(200);
        assert_eq!(slugify(&prompt).chars().count(), 40);
    }

    #[test]
    fn slugify_nunca_fica_vazio() {
        assert_eq!(slugify("!!!???"), "imagem");
    }

    #[test]
    fn slugify_remove_separador_de_diretorio() {
        let slug = slugify("../../etc/passwd");
        assert!(!slug.contains('/'));
        assert!(!slug.contains(".."));
    }
}
