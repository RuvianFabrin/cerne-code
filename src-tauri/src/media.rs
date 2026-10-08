//! Direct media requests. This journal is deliberately separate from chat_log.json.
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::Mutex;

pub struct GenerationGuard<'a> { active: &'a Mutex<HashSet<String>>, session: String }
impl<'a> GenerationGuard<'a> {
    pub fn acquire(active: &'a Mutex<HashSet<String>>, session: &str) -> Result<Self> {
        if !active.lock().unwrap().insert(session.to_owned()) { bail!("Já existe uma geração nesta sessão"); }
        Ok(Self { active, session: session.to_owned() })
    }
}
impl Drop for GenerationGuard<'_> {
    fn drop(&mut self) { self.active.lock().unwrap().remove(&self.session); }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    pub provider: String,
    pub connection_id: String,
    pub aspect_ratio: String,
    pub resolution: String,
    pub parameters: Value,
    pub base_url: String,
    pub model: String,
    /// json: synchronous compatible wrapper; multipart: /videos job protocol.
    pub protocol: String,
    pub seconds: String,
    pub size: String,
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRecord {
    pub id: String,
    pub kind: String,
    pub prompt: String,
    pub model: String,
    pub created_at: String,
    pub after_text_message: usize,
    pub files: Vec<String>,
    pub error: Option<String>,
}

fn journal(dir: &Path, session: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(session)?;
    Ok(dir.join("sessions").join(session).join("media_history.json"))
}
pub fn load(dir: &Path, session: &str) -> Result<Vec<MediaRecord>> {
    let path = journal(dir, session)?;
    if !path.exists() { return Ok(vec![]); }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
pub fn append(dir: &Path, session: &str, record: MediaRecord) -> Result<()> {
    let path = journal(dir, session)?;
    let mut records = load(dir, session)?;
    records.push(record);
    std::fs::write(path, serde_json::to_vec_pretty(&records)?)?;
    Ok(())
}

fn auth(req: reqwest::RequestBuilder, key: Option<&str>) -> reqwest::RequestBuilder {
    match key { Some(k) if !k.is_empty() => req.bearer_auth(k), _ => req }
}
pub(crate) fn image_part(uri: &str) -> Result<reqwest::multipart::Part> {
    let (prefix, encoded) = uri.split_once(",").ok_or_else(|| anyhow!("Imagem inválida"))?;
    let mime = prefix.strip_prefix("data:").and_then(|p| p.strip_suffix(";base64"))
        .filter(|m| matches!(*m, "image/png" | "image/jpeg" | "image/webp"))
        .ok_or_else(|| anyhow!("Use imagem PNG, JPEG ou WebP"))?;
    let ext = match mime { "image/jpeg" => "jpg", "image/webp" => "webp", _ => "png" };
    Ok(reqwest::multipart::Part::bytes(base64::engine::general_purpose::STANDARD.decode(encoded)?)
        .file_name(format!("reference.{ext}")).mime_str(mime)?)
}
pub async fn edit_image(base: &str, key: Option<&str>, model: &str, prompt: &str, images: &[String]) -> Result<Vec<crate::agent::image_gen::GeneratedImage>> {
    let client = reqwest::Client::new();
    let mut form = reqwest::multipart::Form::new().text("prompt", prompt.to_owned()).text("response_format", "b64_json");
    if !model.is_empty() { form = form.text("model", model.to_owned()); }
    for image in images { form = form.part(if images.len() == 1 { "image" } else { "image[]" }, image_part(image)?); }
    let value: Value = auth(client.post(format!("{}/images/edits", base.trim_end_matches('/'))).multipart(form), key)
        .send().await?.error_for_status()?.json().await?;
    crate::agent::image_gen::decode_response(&client, &value).await
}

pub fn video_body(cfg: &VideoConfig, prompt: &str) -> Value {
    let mut body = json!({"prompt": prompt});
    for (field, value) in [("model", &cfg.model), ("seconds", &cfg.seconds), ("size", &cfg.size)] {
        if !value.is_empty() { body[field] = json!(value); }
    }
    body
}
pub async fn generate_video(cfg: &VideoConfig, key: Option<&str>, prompt: &str, destination: &Path) -> Result<Vec<String>> {
    // Validate destination before any paid/external request.
    if !destination.is_dir() { bail!("Escolha uma pasta existente para salvar o vídeo"); }
    let dir = destination.join("generated_videos");
    std::fs::create_dir_all(&dir)?;
    let probe = dir.join(format!(".write-check-{}", uuid::Uuid::new_v4()));
    std::fs::write(&probe, [])?;
    std::fs::remove_file(probe)?;
    if cfg.base_url.trim().is_empty() { bail!("Configure a API de vídeo"); }
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(1800)).build()?;
    let endpoint = format!("{}/videos", cfg.base_url.trim_end_matches('/'));
    let req = if cfg.protocol == "multipart" {
        let mut form = reqwest::multipart::Form::new();
        for (k, v) in video_body(cfg, prompt).as_object().unwrap() { form = form.text(k.clone(), v.as_str().unwrap().to_owned()); }
        client.post(&endpoint).multipart(form)
    } else { client.post(&endpoint).json(&video_body(cfg, prompt)) };
    let response = auth(req, key).send().await?.error_for_status()?;
    let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let bytes = if content_type.starts_with("video/") { response.bytes().await?.to_vec() } else {
        let mut value: Value = response.json().await?;
        if let Some(id) = value.get("id").and_then(Value::as_str).map(str::to_owned) {
            if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { bail!("ID de vídeo inválido"); }
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1800);
            while matches!(value.get("status").and_then(Value::as_str), Some("queued" | "in_progress" | "processing" | "pending")) {
                if tokio::time::Instant::now() >= deadline { bail!("Tempo de espera esgotado. ID do vídeo: {id}"); }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                value = auth(client.get(format!("{endpoint}/{id}")), key).send().await?.error_for_status()?.json().await?;
            }
            if matches!(value.get("status").and_then(Value::as_str), Some("failed" | "cancelled")) { bail!("API não concluiu o vídeo: {}", value.get("error").unwrap_or(&Value::Null)); }
            if value.get("status").and_then(Value::as_str) == Some("completed") {
                auth(client.get(format!("{endpoint}/{id}/content")), key).send().await?.error_for_status()?.bytes().await?.to_vec()
            } else { download_video(&client, &value).await? }
        } else { download_video(&client, &value).await? }
    };
    if bytes.len() < 12 || &bytes[4..8] != b"ftyp" { bail!("A API precisa devolver um vídeo MP4 válido"); }
    let path = dir.join(format!("video-{}.mp4", uuid::Uuid::new_v4()));
    std::fs::write(&path, bytes)?;
    Ok(vec![path.to_string_lossy().into_owned()])
}
async fn download_video(client: &reqwest::Client, value: &Value) -> Result<Vec<u8>> {
    let item = value.get("data").and_then(Value::as_array).and_then(|a| a.first()).unwrap_or(value);
    if let Some(b64) = item.get("b64_json").and_then(Value::as_str) { return Ok(base64::engine::general_purpose::STANDARD.decode(b64)?); }
    let url = item.get("url").or_else(|| item.get("video_url")).and_then(Value::as_str).ok_or_else(|| anyhow!("Resposta sem URL ou b64_json do vídeo"))?;
    let parsed = reqwest::Url::parse(url)?;
    if !matches!(parsed.scheme(), "http" | "https") { bail!("URL do vídeo inválida"); }
    // Signed/CDN URL: do not leak the API credential to another origin.
    Ok(client.get(parsed).send().await?.error_for_status()?.bytes().await?.to_vec())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) async fn fake_api(replies: Vec<(&'static str, Vec<u8>)>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let reply_base = base.clone();
        let task = tokio::spawn(async move {
            let base = reply_base;
            let mut requests = vec![];
            for (mime, body) in replies {
                let body = if mime == "application/json" { String::from_utf8(body).unwrap().replace("__BASE__", &base).into_bytes() } else { body };
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![];
                let header_end;
                loop {
                    let mut buffer = [0; 4096];
                    let size = stream.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    request.extend_from_slice(&buffer[..size]);
                    if let Some(index) = request.windows(4).position(|w| w == b"\r\n\r\n") { header_end = index + 4; break; }
                }
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers.lines().find_map(|line| line.to_lowercase().strip_prefix("content-length: ").and_then(|s| s.trim().parse::<usize>().ok())).unwrap_or(0);
                while request.len() < header_end + length {
                    let mut buffer = [0; 4096];
                    let size = stream.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    request.extend_from_slice(&buffer[..size]);
                }
                requests.push(String::from_utf8_lossy(&request).into_owned());
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
                stream.write_all(&body).await.unwrap();
            }
            requests
        });
        (base, task)
    }
    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("cerne-media-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap(); path
    }
    #[tokio::test]
    async fn image_generation_and_edit_send_only_prompt_and_current_images() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        let response = serde_json::to_vec(&json!({"data":[{"b64_json":b64}]})).unwrap();
        let (base, task) = fake_api(vec![("application/json", response.clone()), ("application/json", response)]).await;
        assert_eq!(crate::agent::image_gen::generate(&base, Some("test-key"), "image-model", "current image prompt", 1).await.unwrap().len(), 1);
        assert_eq!(edit_image(&base, Some("test-key"), "image-model", "edit current", &[format!("data:image/png;base64,{b64}")]).await.unwrap().len(), 1);
        let requests = task.await.unwrap();
        let body: Value = serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["prompt"], "current image prompt");
        assert!(body.get("messages").is_none());
        assert!(requests[1].starts_with("POST /v1/images/edits "));
        assert!(requests[1].contains("name=\"image\""));
        assert!(requests[1].contains("edit current"));
        assert!(!requests[1].contains("current image prompt"));
    }
    #[tokio::test]
    async fn video_saves_file_and_polls_jobs() {
        let mp4 = b"\0\0\0\x18ftypisom\0\0\0\0isommp42".to_vec();
        let (base, task) = fake_api(vec![
            ("application/json", br#"{"id":"job_123","status":"queued"}"#.to_vec()),
            ("application/json", br#"{"id":"job_123","status":"completed"}"#.to_vec()),
            ("video/mp4", mp4.clone()),
        ]).await;
        let dir = temp_dir();
        let cfg = VideoConfig { base_url: base, model: "video-model".into(), protocol: "multipart".into(), ..Default::default() };
        let files = generate_video(&cfg, Some("test-key"), "current video prompt", &dir).await.unwrap();
        assert_eq!(std::fs::read(&files[0]).unwrap(), mp4);
        let requests = task.await.unwrap();
        assert!(requests[0].contains("current video prompt"));
        assert!(!requests[0].contains("messages"));
        assert!(requests[1].starts_with("GET /v1/videos/job_123 "));
        assert!(requests[2].starts_with("GET /v1/videos/job_123/content "));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn missing_video_destination_prevents_network_request() {
        let cfg = VideoConfig { base_url: "http://127.0.0.1:1".into(), ..Default::default() };
        let error = generate_video(&cfg, None, "prompt", &std::env::temp_dir().join(uuid::Uuid::new_v4().to_string())).await.unwrap_err();
        assert!(error.to_string().contains("Escolha uma pasta"));
    }
    #[test]
    fn payload_contains_only_current_prompt() {
        let cfg = VideoConfig { model: "local-video".into(), ..Default::default() };
        assert_eq!(video_body(&cfg, "current"), json!({"model":"local-video", "prompt":"current"}));
    }
    #[test]
    fn media_never_enters_text_history() {
        let dir = std::env::temp_dir().join(format!("cerne-media-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let session = dir.join("sessions").join(&id);
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(session.join("chat_log.json"), b"[]").unwrap();
        append(&dir, &id, MediaRecord { id: "test".into(), kind: "image".into(), prompt: "private media prompt".into(), model: "image".into(), created_at: "now".into(), after_text_message: 0, files: vec![], error: None }).unwrap();
        assert_eq!(load(&dir, &id).unwrap().len(), 1);
        assert_eq!(std::fs::read(session.join("chat_log.json")).unwrap(), b"[]");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

pub async fn generate_vendor_video(c: &crate::media_providers::Connection, cfg: &VideoConfig, prompt: &str, destination: &Path) -> Result<Vec<String>> {
    if c.provider.is_empty() || c.provider == "compatible" { return generate_video(cfg, c.key.as_deref(), prompt, destination).await; }
    if !destination.is_dir() { bail!("Escolha uma pasta existente para salvar o vídeo"); }
    let dir = destination.join("generated_videos");
    std::fs::create_dir_all(&dir)?;
    let probe = dir.join(format!(".write-check-{}", uuid::Uuid::new_v4()));
    std::fs::write(&probe, [])?; std::fs::remove_file(probe)?;
    let bytes = crate::media_providers::generate_video(c, cfg, prompt).await?;
    if bytes.len() < 12 || &bytes[4..8] != b"ftyp" { bail!("A API precisa devolver um vídeo MP4 válido"); }
    let path = dir.join(format!("video-{}.mp4", uuid::Uuid::new_v4()));
    std::fs::write(&path, bytes)?;
    Ok(vec![path.to_string_lossy().into_owned()])
}
