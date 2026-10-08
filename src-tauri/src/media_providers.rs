//! Vendor-specific media adapters. Credentials remain on the backend.
use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use crate::{agent::image_gen::{self, GeneratedImage}, models::AppConfig};

#[derive(Clone)]
pub struct Connection { pub provider: String, pub base: String, pub key: Option<String> }
#[derive(Serialize)]
pub struct ConnectionView { pub id: String, pub provider: String, pub label: String, pub has_key: bool }
#[derive(Serialize)]
pub struct MediaModel { pub id: String, pub label: String }

pub fn default_base(provider: &str) -> &'static str {
    match provider {
        "openrouter" => "https://openrouter.ai/api/v1", "openai" => "https://api.openai.com/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta", "xai" => "https://api.x.ai/v1",
        "fal" => "https://queue.fal.run", _ => "",
    }
}
pub fn detect_provider(base: &str) -> Option<&'static str> {
    let url = reqwest::Url::parse(base).ok()?;
    match url.host_str()? {
        "openrouter.ai" => Some("openrouter"), "api.openai.com" => Some("openai"),
        "generativelanguage.googleapis.com" => Some("gemini"), "api.x.ai" => Some("xai"),
        "queue.fal.run" | "fal.run" => Some("fal"), _ => None,
    }
}
pub fn connections(cfg: &AppConfig, dir: &Path) -> Result<Vec<ConnectionView>> {
    let mut result = vec![ConnectionView { id: "openrouter".into(), provider: "openrouter".into(), label: "OpenRouter".into(), has_key: crate::config::has_openrouter_key() }];
    for p in crate::providers::custom::load_providers(dir)? {
        if let Some(provider) = detect_provider(&p.base_url) {
            result.push(ConnectionView { id: format!("custom:{}", p.id), provider: provider.into(), label: p.label, has_key: crate::providers::custom::has_key(&p.id) });
        }
    }
    let _ = cfg;
    Ok(result)
}
pub fn resolve(cfg: &AppConfig, dir: &Path, video: bool) -> Result<Connection> {
    let (provider, id, base) = if video { (&cfg.video_gen.provider, &cfg.video_gen.connection_id, &cfg.video_gen.base_url) }
        else { (&cfg.image_gen.provider, &cfg.image_gen.connection_id, &cfg.image_gen.base_url) };
    if !matches!(provider.as_str(), "" | "compatible" | "openrouter" | "openai" | "gemini" | "xai" | "fal") { bail!("Provedor de mídia desconhecido"); }
    if video && provider == "openai" { bail!("A API de vídeo Sora/OpenAI foi encerrada. Escolha OpenRouter, Gemini, xAI ou fal.ai."); }
    if let Some(custom_id) = id.strip_prefix("custom:") {
        let p = crate::providers::custom::load_providers(dir)?.into_iter().find(|p| p.id == custom_id).ok_or_else(|| anyhow!("Conexão não encontrada"))?;
        if detect_provider(&p.base_url) != Some(provider.as_str()) { bail!("A conexão escolhida não pertence a este provedor"); }
        let key = crate::providers::custom::get_key(&p.id).ok_or_else(|| anyhow!("Configure a chave na conexão existente"))?;
        let base = if provider == "gemini" { default_base("gemini").to_owned() } else if provider == "fal" { default_base("fal").to_owned() } else { p.base_url.trim_end_matches('/').to_owned() };
        return Ok(Connection { provider: provider.clone(), base, key: Some(key) });
    }
    if provider == "openrouter" {
        return Ok(Connection { provider: provider.clone(), base: cfg.openrouter_base_url.trim_end_matches('/').into(),
            key: Some(crate::config::get_openrouter_key().ok_or_else(|| anyhow!("Configure o OpenRouter em Configurações → OpenRouter"))?) });
    }
    let key = if provider.is_empty() || provider == "compatible" {
        if video { crate::config::get_video_gen_key() } else { crate::config::get_image_gen_key() }
    } else { Some(crate::config::get_media_provider_key(provider).ok_or_else(|| anyhow!("Configure uma chave ou selecione uma conexão existente"))?) };
    let base = if base.trim().is_empty() { default_base(provider) } else { base.trim_end_matches('/') };
    if base.is_empty() { bail!("Configure a URL da API"); }
    Ok(Connection { provider: provider.clone(), base: base.to_owned(), key })
}
pub fn auth(req: reqwest::RequestBuilder, c: &Connection) -> reqwest::RequestBuilder {
    match c.key.as_deref() {
        Some(key) if c.provider == "gemini" => req.header("x-goog-api-key", key),
        Some(key) if c.provider == "fal" => req.header("Authorization", format!("Key {key}")),
        Some(key) => req.bearer_auth(key), None => req,
    }
}
async fn response(req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let r = req.send().await?;
    if !r.status().is_success() { bail!("API de mídia ({}) — {}", r.status(), r.text().await?); }
    Ok(r)
}
async fn json_response(req: reqwest::RequestBuilder) -> Result<Value> { Ok(response(req).await?.json().await?) }
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().timeout(std::time::Duration::from_secs(1800)).redirect(reqwest::redirect::Policy::none()).build()?)
}
pub fn merge_parameters(body: &mut Value, parameters: &Value) -> Result<()> {
    if parameters.is_null() { return Ok(()); }
    let object = parameters.as_object().ok_or_else(|| anyhow!("Parâmetros adicionais precisam ser um objeto JSON"))?;
    for (key, value) in object {
        if matches!(key.as_str(), "prompt" | "model" | "messages" | "contents" | "instances" | "image" | "images" | "image_url" | "image_urls" | "input_references") { bail!("O parâmetro {key} é controlado pelo composer"); }
        body[key] = value.clone();
    }
    Ok(())
}
fn safe_model(model: &str) -> Result<&str> {
    if model.is_empty() || model.split('/').any(|p| p.is_empty() || p == "." || p == "..") || !model.chars().all(|c| c.is_ascii_alphanumeric() || "-_/.".contains(c)) { bail!("Selecione um modelo válido"); }
    Ok(model.strip_prefix("models/").unwrap_or(model))
}
fn inline_image(uri: &str) -> Result<Value> {
    let (head, data) = uri.split_once(',').ok_or_else(|| anyhow!("Imagem inválida"))?;
    let mime = head.strip_prefix("data:").and_then(|h| h.strip_suffix(";base64")).filter(|m| m.starts_with("image/")).ok_or_else(|| anyhow!("Imagem inválida"))?;
    Ok(json!({"mimeType":mime,"data":data}))
}

pub async fn generate_image(c: &Connection, model: &str, prompt: &str, images: &[String], n: u32, parameters: &Value) -> Result<Vec<GeneratedImage>> {
    let client = client()?;
    if c.provider.is_empty() || c.provider == "compatible" {
        return if images.is_empty() { image_gen::generate(&c.base, c.key.as_deref(), model, prompt, n).await }
            else { crate::media::edit_image(&c.base, c.key.as_deref(), model, prompt, images).await };
    }
    let model = safe_model(model)?;
    let value = match c.provider.as_str() {
        "openai" if !images.is_empty() => {
            let mut form = reqwest::multipart::Form::new().text("prompt", prompt.to_owned()).text("model", model.to_owned()).text("n", n.max(1).to_string());
            for image in images { form = form.part("image[]", crate::media::image_part(image)?); }
            if let Some(params) = parameters.as_object() {
                let mut check = json!({}); merge_parameters(&mut check, parameters)?;
                for (key, value) in params { form = form.text(key.clone(), value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string())); }
            }
            json_response(auth(client.post(format!("{}/images/edits", c.base)).multipart(form), c)).await?
        }
        "gemini" if model.starts_with("imagen-") => {
            if !images.is_empty() { bail!("Para editar imagens no Google, escolha um modelo Gemini de imagem"); }
            let mut params = json!({"sampleCount":n.max(1)}); merge_parameters(&mut params, parameters)?;
            let v = json_response(auth(client.post(format!("{}/models/{model}:predict", c.base)).json(&json!({"instances":[{"prompt":prompt}],"parameters":params})), c)).await?;
            json!({"data":v.get("predictions").and_then(Value::as_array).unwrap_or(&vec![]).iter().map(|p| json!({"b64_json":p["bytesBase64Encoded"]})).collect::<Vec<_>>()})
        }
        "gemini" => {
            let mut parts = vec![json!({"text":prompt})];
            for uri in images { parts.push(json!({"inlineData":inline_image(uri)?})); }
            let mut gen = json!({"responseModalities":["TEXT","IMAGE"]}); merge_parameters(&mut gen, parameters)?;
            let v = json_response(auth(client.post(format!("{}/models/{model}:generateContent", c.base)).json(&json!({"contents":[{"role":"user","parts":parts}],"generationConfig":gen})), c)).await?;
            let mut data = vec![];
            if let Some(candidates) = v["candidates"].as_array() { for candidate in candidates {
                if let Some(parts) = candidate["content"]["parts"].as_array() { for part in parts {
                    if part.get("thought").and_then(Value::as_bool) == Some(true) { continue; }
                    if let Some(blob) = part.get("inlineData").or_else(|| part.get("inline_data")) { if blob["data"].is_string() { data.push(json!({"b64_json":blob["data"]})); } }
                } }
            } }
            json!({"data":data})
        }
        "fal" => {
            if !images.is_empty() { bail!("A edição fal.ai usa campos próprios de cada modelo. Use OpenRouter, Gemini, OpenAI ou xAI para edição direta."); }
            let mut body = json!({"prompt":prompt}); merge_parameters(&mut body, parameters)?;
            let v = fal_job(&client, c, model, &body).await?;
            json!({"data":v["images"].as_array().unwrap_or(&vec![]).iter().map(|i| json!({"url":i["url"]})).collect::<Vec<_>>()})
        }
        provider => {
            let mut body = json!({"model":model,"prompt":prompt,"n":n.max(1)});
            let route = if provider == "openrouter" {
                if !images.is_empty() { body["input_references"] = json!(images.iter().map(|uri| json!({"type":"image_url","image_url":{"url":uri}})).collect::<Vec<_>>()); }
                "images"
            } else if provider == "xai" {
                body["response_format"] = json!("b64_json");
                if images.len() > 1 { bail!("Este adaptador xAI aceita uma imagem de referência por edição"); }
                if let Some(uri) = images.first() { body["image"] = json!({"url":uri,"type":"image_url"}); "images/edits" } else { "images/generations" }
            } else { "images/generations" };
            merge_parameters(&mut body, parameters)?;
            json_response(auth(client.post(format!("{}/{route}", c.base)).json(&body), c)).await?
        }
    };
    image_gen::decode_response(&reqwest::Client::new(), &value).await
}

pub fn video_payload(c: &Connection, cfg: &crate::media::VideoConfig, prompt: &str) -> Result<Value> {
    let mut body = json!({"model":safe_model(&cfg.model)?,"prompt":prompt});
    if !cfg.seconds.is_empty() {
        let duration: u32 = cfg.seconds.parse().map_err(|_| anyhow!("Duração precisa ser um número inteiro positivo"))?;
        if duration == 0 { bail!("Duração precisa ser maior que zero"); }
        body["duration"] = json!(duration);
    }
    if c.provider == "gemini" {
        let mut parameters = json!({});
        if let Some(duration) = body.get("duration") { parameters["durationSeconds"] = duration.clone(); }
        if !cfg.aspect_ratio.is_empty() { parameters["aspectRatio"] = json!(cfg.aspect_ratio); }
        if !cfg.resolution.is_empty() { parameters["resolution"] = json!(cfg.resolution); }
        if !cfg.size.is_empty() { bail!("No Veo use proporção e resolução, deixando tamanho em pixels vazio"); }
        merge_parameters(&mut parameters, &cfg.parameters)?;
        Ok(json!({"instances":[{"prompt":prompt}],"parameters":parameters}))
    } else {
        if c.provider == "fal" { body.as_object_mut().unwrap().remove("model"); if !cfg.seconds.is_empty() { body["duration"] = json!(cfg.seconds); } }
        if !cfg.size.is_empty() { if c.provider == "xai" { bail!("No xAI use proporção e resolução, deixando tamanho em pixels vazio"); } body["size"] = json!(cfg.size); }
        if !cfg.aspect_ratio.is_empty() { body["aspect_ratio"] = json!(cfg.aspect_ratio); }
        if !cfg.resolution.is_empty() { body["resolution"] = json!(cfg.resolution); }
        merge_parameters(&mut body, &cfg.parameters)?;
        Ok(body)
    }
}
async fn pause(deadline: tokio::time::Instant) -> Result<()> {
    if tokio::time::Instant::now() >= deadline { bail!("Tempo de espera da geração esgotado"); }
    tokio::time::sleep(std::time::Duration::from_secs(if cfg!(test) { 0 } else { 10 })).await;
    Ok(())
}
fn same_origin(base: &str, target: &str) -> Result<reqwest::Url> {
    let origin = reqwest::Url::parse(base)?;
    let url = origin.join(target)?;
    if origin.origin() != url.origin() { bail!("A API retornou uma URL de acompanhamento fora do seu domínio"); }
    Ok(url)
}
async fn fal_job(client: &reqwest::Client, c: &Connection, model: &str, body: &Value) -> Result<Value> {
    let job = json_response(auth(client.post(format!("{}/{}", c.base, safe_model(model)?)).json(body), c)).await?;
    let status_url = same_origin(&c.base, job["status_url"].as_str().ok_or_else(|| anyhow!("fal.ai sem status_url"))?)?;
    let response_url = same_origin(&c.base, job["response_url"].as_str().ok_or_else(|| anyhow!("fal.ai sem response_url"))?)?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1800);
    loop {
        pause(deadline).await?;
        let status = json_response(auth(client.get(status_url.clone()), c)).await?;
        if let Some(error) = status.get("error").filter(|e| !e.is_null()) { bail!("fal.ai: {error}"); }
        match status["status"].as_str() {
            Some("COMPLETED") => return json_response(auth(client.get(response_url), c)).await,
            Some("IN_QUEUE" | "IN_PROGRESS") => (), _ => bail!("Status fal.ai desconhecido: {}", status["status"]),
        }
    }
}
async fn download(client: &reqwest::Client, c: &Connection, target: &str, authenticated: bool) -> Result<Vec<u8>> {
    let mut url = reqwest::Url::parse(target)?;
    if !matches!(url.scheme(), "http" | "https") { bail!("URL de download inválida"); }
    for _ in 0..6 {
        // Only authenticated service URLs receive credentials, never CDN redirects.
        let own = same_origin(&c.base, url.as_str()).is_ok();
        let req = client.get(url.clone());
        let resp = if authenticated && own { auth(req, c) } else { req }.send().await?;
        if resp.status().is_redirection() {
            url = url.join(resp.headers().get("location").and_then(|h| h.to_str().ok()).ok_or_else(|| anyhow!("Redirect sem destino"))?)?;
            continue;
        }
        if !resp.status().is_success() { bail!("Download de mídia falhou ({})", resp.status()); }
        return Ok(resp.bytes().await?.to_vec());
    }
    bail!("Redirecionamentos demais ao baixar mídia")
}
pub async fn generate_video(c: &Connection, cfg: &crate::media::VideoConfig, prompt: &str) -> Result<Vec<u8>> {
    let client = client()?;
    let body = video_payload(c, cfg, prompt)?;
    let model = safe_model(&cfg.model)?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1800);
    if c.provider == "fal" {
        let value = fal_job(&client, c, model, &body).await?;
        return download(&client, c, value["video"]["url"].as_str().ok_or_else(|| anyhow!("fal.ai sem vídeo no resultado"))?, false).await;
    }
    let route = match c.provider.as_str() { "gemini" => format!("models/{model}:predictLongRunning"), "xai" => "videos/generations".into(), "openrouter" => "videos".into(), _ => bail!("Provedor de vídeo sem adaptador") };
    let mut value = json_response(auth(client.post(format!("{}/{route}", c.base)).json(&body), c)).await?;
    if c.provider == "gemini" {
        let name = value["name"].as_str().ok_or_else(|| anyhow!("Google sem nome da operação"))?.to_owned();
        if name.starts_with('/') || name.contains("..") || name.contains(['?', '#']) { bail!("Operação Google inválida"); }
        let endpoint = same_origin(&format!("{}/", c.base), &name)?;
        while value["done"].as_bool() != Some(true) { pause(deadline).await?; value = json_response(auth(client.get(endpoint.clone()), c)).await?; }
        if let Some(error) = value.get("error") { bail!("Google: {error}"); }
        let uri = value.pointer("/response/generateVideoResponse/generatedSamples/0/video/uri")
            .or_else(|| value.pointer("/response/generatedVideos/0/video/uri")).and_then(Value::as_str).ok_or_else(|| anyhow!("Google não devolveu vídeo (verifique bloqueio ou filtros)"))?;
        if same_origin(&c.base, uri).is_err() { bail!("Google retornou download autenticado fora do domínio da API"); }
        return download(&client, c, uri, true).await;
    }
    let id = value.get(if c.provider == "xai" { "request_id" } else { "id" }).and_then(Value::as_str).ok_or_else(|| anyhow!("API sem ID de geração"))?.to_owned();
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)) { bail!("ID de geração inválido"); }
    let endpoint = format!("{}/videos/{id}", c.base);
    loop {
        if let Some(error) = value.get("error").filter(|e| !e.is_null()) { bail!("API de vídeo: {error}"); }
        match value["status"].as_str() {
            Some("completed") if c.provider == "openrouter" => return download(&client, c, &format!("{endpoint}/content?index=0"), true).await,
            Some("done") if c.provider == "xai" => return download(&client, c, value["video"]["url"].as_str().ok_or_else(|| anyhow!("xAI sem URL de vídeo"))?, false).await,
            Some("failed" | "cancelled" | "expired") => bail!("Geração interrompida: {}", value["status"]),
            None if c.provider == "xai" => (),
            Some("pending" | "queued" | "in_progress" | "processing") => (),
            _ => bail!("Status de geração desconhecido: {}", value["status"]),
        }
        pause(deadline).await?; value = json_response(auth(client.get(&endpoint), c)).await?;
    }
}
pub async fn list_models(c: &Connection, video: bool) -> Result<Vec<MediaModel>> {
    if c.provider == "fal" || c.provider.is_empty() || c.provider == "compatible" { bail!("Informe o ID do modelo/endpoint conforme a documentação do servidor"); }
    let route = if c.provider == "openrouter" { if video { "videos/models" } else { "images/models" } } else { "models" };
    let client = client()?;
    let mut result = vec![];
    let mut page_token: Option<String> = None;
    for _ in 0..10 {
        let mut req = client.get(format!("{}/{route}", c.base));
        if let Some(token) = &page_token { req = req.query(&[("pageToken", token)]); }
        let value = json_response(auth(req, c)).await?;
        let rows = value.get("data").or_else(|| value.get("models")).and_then(Value::as_array).ok_or_else(|| anyhow!("Catálogo de modelos inválido"))?;
        for row in rows {
            let id = row.get("id").or_else(|| row.get("name")).and_then(Value::as_str).unwrap_or("").trim_start_matches("models/");
            let include = match c.provider.as_str() {
                "openrouter" => true, "openai" => !video && (id.contains("gpt-image") || id == "chatgpt-image-latest"),
                "gemini" => if video { id.contains("veo") } else { id.contains("image") || id.contains("imagen") || id.contains("nano-banana") },
                "xai" => id.contains(if video { "imagine-video" } else { "imagine-image" }), _ => false,
            };
            if include { result.push(MediaModel { id: id.into(), label: row.get("displayName").or_else(|| row.get("name")).and_then(Value::as_str).unwrap_or(id).into() }); }
        }
        page_token = value["nextPageToken"].as_str().map(str::to_owned);
        if c.provider != "gemini" || page_token.is_none() { break; }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use crate::media::tests::fake_api;
    fn connection(provider: &str, base: String) -> Connection { Connection { provider: provider.into(), base, key: Some("test-secret".into()) } }
    fn png() -> String {
        let mut bytes = std::io::Cursor::new(vec![]);
        image::DynamicImage::new_rgb8(2, 2).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
    }
    fn reply(v: Value) -> (&'static str, Vec<u8>) { ("application/json", serde_json::to_vec(&v).unwrap()) }
    fn payload(request: &str) -> Value { serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap() }
    #[tokio::test]
    async fn image_vendor_routes_and_edit_formats() {
        for provider in ["openrouter", "openai", "gemini", "xai"] {
            for edit in [false, true] {
                let b64 = png();
                let result = if provider == "gemini" { json!({"candidates":[{"content":{"parts":[{"inlineData":{"data":b64,"mimeType":"image/png"}}]}}]}) } else { json!({"data":[{"b64_json":b64}]}) };
                let (base, task) = fake_api(vec![reply(result)]).await;
                let c = connection(provider, base);
                let images = if edit { vec![format!("data:image/png;base64,{b64}")] } else { vec![] };
                assert_eq!(generate_image(&c, "image-model", "only current prompt", &images, 1, &Value::Null).await.unwrap().len(), 1);
                let requests = task.await.unwrap(); let request = &requests[0];
                assert!(!request.contains("messages")); assert!(request.contains("only current prompt"));
                if provider == "gemini" {
                    assert!(request.starts_with("POST /v1/models/image-model:generateContent "));
                    assert!(request.to_lowercase().contains("x-goog-api-key: test-secret"));
                    assert_eq!(payload(request)["contents"][0]["parts"].as_array().unwrap().len(), if edit {2} else {1});
                } else {
                    assert!(request.to_lowercase().contains("authorization: bearer test-secret"));
                    if provider == "openrouter" { assert!(request.starts_with("POST /v1/images ")); if edit { assert!(payload(request)["input_references"].is_array()); } }
                    else if edit { assert!(request.starts_with("POST /v1/images/edits ")); }
                    else { assert!(request.starts_with("POST /v1/images/generations ")); }
                    if provider == "openai" { assert!(!request.contains("response_format")); if edit { assert!(request.contains("multipart/form-data")); assert!(request.contains("image[]")); } }
                    if provider == "xai" && edit { assert_eq!(payload(request)["image"]["type"], "image_url"); }
                }
            }
        }
    }
    #[tokio::test]
    async fn video_vendors_poll_and_download_with_correct_auth() {
        let mp4 = b"\0\0\0\x18ftypisomtest".to_vec();
        for provider in ["openrouter", "xai", "gemini", "fal"] {
            let replies = match provider {
                "openrouter" => vec![reply(json!({"id":"job-1","status":"pending"})), reply(json!({"id":"job-1","status":"completed"})), ("video/mp4", mp4.clone())],
                "xai" => vec![reply(json!({"request_id":"job-1"})), reply(json!({"status":"done","video":{"url":"__BASE__/download"}})), ("video/mp4", mp4.clone())],
                "gemini" => vec![reply(json!({"name":"models/video-model/operations/job-1"})), reply(json!({"done":true,"response":{"generateVideoResponse":{"generatedSamples":[{"video":{"uri":"__BASE__/download"}}]}}})), ("video/mp4",mp4.clone())],
                _ => vec![reply(json!({"status_url":"__BASE__/status","response_url":"__BASE__/result"})),reply(json!({"status":"COMPLETED"})),reply(json!({"video":{"url":"__BASE__/download"}})),("video/mp4",mp4.clone())],
            };
            let (base, task) = fake_api(replies).await;
            let cfg = crate::media::VideoConfig { model:"video-model".into(), seconds:"4".into(), aspect_ratio:"16:9".into(), ..Default::default() };
            assert_eq!(generate_video(&connection(provider, base), &cfg, "video prompt only").await.unwrap(), mp4);
            let requests = task.await.unwrap(); let body = payload(&requests[0]);
            assert!(!body.to_string().contains("messages"));
            if provider == "gemini" { assert_eq!(body["parameters"]["durationSeconds"],4); assert_eq!(body["instances"][0]["prompt"],"video prompt only"); }
            else { assert_eq!(body["prompt"], "video prompt only"); assert_eq!(body["duration"], if provider == "fal" {json!("4")} else {json!(4)}); assert!(body.get("seconds").is_none()); }
            let final_request = requests.last().unwrap().to_lowercase();
            assert_eq!(final_request.contains("test-secret"), matches!(provider,"openrouter"|"gemini"));
            if provider == "fal" { assert!(requests[0].to_lowercase().contains("authorization: key test-secret")); }
            if provider == "openrouter" { assert!(final_request.starts_with("get /v1/videos/job-1/content?index=0 ")); }
        }
    }
    #[tokio::test]
    async fn image_catalog_filters_and_google_imagen() {
        let (base, task) = fake_api(vec![reply(json!({"data":[{"id":"gpt-image-test"},{"id":"text-model"}]}))]).await;
        assert_eq!(list_models(&connection("openai",base),false).await.unwrap().len(),1); task.await.unwrap();
        let (base,task) = fake_api(vec![reply(json!({"predictions":[{"bytesBase64Encoded":png()}]}))]).await;
        generate_image(&connection("gemini",base),"imagen-test","prompt",&[],1,&Value::Null).await.unwrap();
        let requests = task.await.unwrap(); assert_eq!(payload(&requests[0])["parameters"]["sampleCount"],1);
    }
    #[tokio::test]
    async fn fal_image_queue_and_openrouter_video_catalog() {
        let bytes = base64::engine::general_purpose::STANDARD.decode(png()).unwrap();
        let (base,task) = fake_api(vec![reply(json!({"status_url":"__BASE__/status","response_url":"__BASE__/result"})),reply(json!({"status":"IN_PROGRESS"})),reply(json!({"status":"COMPLETED"})),reply(json!({"images":[{"url":"__BASE__/image"}]})),("image/png",bytes)]).await;
        let c = connection("fal",base);
        assert_eq!(generate_image(&c,"fal-ai/model","image prompt",&[],1,&json!({"num_images":1})).await.unwrap().len(),1);
        let requests = task.await.unwrap();
        assert!(requests[0].starts_with("POST /v1/fal-ai/model "));
        assert_eq!(payload(&requests[0]),json!({"prompt":"image prompt","num_images":1}));
        assert!(!requests.last().unwrap().contains("test-secret"));
        let (base,task) = fake_api(vec![reply(json!({"data":[{"id":"google/veo-test","name":"Google: Veo test"}]}))]).await;
        let models = list_models(&connection("openrouter",base),true).await.unwrap();
        assert_eq!(models[0].id,"google/veo-test");
        assert!(task.await.unwrap()[0].starts_with("GET /v1/videos/models "));
    }
    #[test]
    fn reject_history_overrides_and_untrusted_job_urls() {
        for key in ["messages","prompt","contents","instances","input_references"] { assert!(merge_parameters(&mut json!({}), &json!({key:[]})).is_err()); }
        assert!(same_origin("https://api.x.ai/v1", "https://other.test/secret").is_err());
        assert_eq!(detect_provider("https://api.openai.com.attacker.test/v1"),None);
        assert!(safe_model("../secret").is_err());
        let cfg = AppConfig::default(); assert!(serde_json::to_value(cfg).unwrap()["image_gen"]["parameters"].is_null());
    }
}
