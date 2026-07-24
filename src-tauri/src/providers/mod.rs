pub mod custom;
pub mod llama_cpp;

use crate::models::{
    ChatMessage, ModelInfo, ProviderConfig, ProviderKind, ReasoningEffort, ToolCall,
    ToolCallFunction, ToolSpec,
};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde_json::json;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamTokenEvent {
    pub session_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct StreamUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

pub struct StreamResult {
    pub message: ChatMessage,
    pub usage: StreamUsage,
}

/// Tabela hardcoded de context lengths conhecidos por modelo.
/// A API da Qwen Cloud (e a maioria dos providers OpenAI-compatible)
/// não retorna context_length no /v1/models, então usamos isso como fallback.
const KNOWN_CONTEXT_LENGTHS: &[(&str, u32)] = &[
    ("qwen3.8-max-preview", 1_000_000),
    ("qwen3.7-max", 1_000_000),
    ("qwen3.7-plus", 1_000_000),
    ("qwen3.6-flash", 1_000_000),
    ("qwen3-max", 262_144),
    ("qwen3-plus", 131_072),
    ("qwen3-flash", 131_072),
    ("qwen3-235b-a22b", 131_072),
    ("qwen3-30b-a3b", 131_072),
    ("qwen3-32b", 131_072),
    ("qwen3-14b", 131_072),
    ("qwen3-8b", 131_072),
    ("qwen3-4b", 131_072),
    ("qwen3-1.7b", 32_768),
    ("qwen3-0.6b", 32_768),
    ("qwen-max", 32_768),
    ("qwen-plus", 131_072),
    ("qwen-turbo", 131_072),
    ("qwen-long", 10_000_000),
    ("qwen2.5-max", 32_768),
    ("qwen2.5-plus", 131_072),
    ("qwen2.5-72b-instruct", 131_072),
    ("qwen2.5-32b-instruct", 131_072),
    ("qwen2.5-14b-instruct", 131_072),
    ("qwen2.5-7b-instruct", 131_072),
    ("deepseek-v3", 65_536),
    ("deepseek-r1", 65_536),
    ("deepseek-v4-pro", 131_072),
    ("deepseek-v4-flash", 131_072),
];

fn known_context_length(model_id: &str) -> Option<u32> {
    let lower = model_id.to_lowercase();
    KNOWN_CONTEXT_LENGTHS
        .iter()
        .find(|(id, _)| lower.contains(id))
        .map(|(_, len)| *len)
}

fn context_cache_path(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    app_data_dir.join("model_context_cache.json")
}

pub fn load_context_cache(app_data_dir: &std::path::Path) -> std::collections::HashMap<String, u32> {
    let path = context_cache_path(app_data_dir);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_context_length(app_data_dir: &std::path::Path, model_id: &str, context_length: u32) {
    let mut cache = load_context_cache(app_data_dir);
    cache.insert(model_id.to_string(), context_length);
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(context_cache_path(app_data_dir), json);
    }
}

pub fn resolve_context_length(app_data_dir: &std::path::Path, model_id: &str, provider_override: Option<u32>) -> u32 {
    let cache = load_context_cache(app_data_dir);
    if let Some(&len) = cache.get(model_id) {
        return len;
    }
    if let Some(len) = known_context_length(model_id) {
        save_context_length(app_data_dir, model_id, len);
        return len;
    }
    provider_override.unwrap_or(crate::models::DEFAULT_CONTEXT_LENGTH)
}

/// Converts `ChatMessage`s to the wire format the OpenAI-compatible
/// `/chat/completions` endpoint expects. Plain messages serialize exactly as
/// before; messages carrying `images` get their `content` rewritten into the
/// standard OpenAI multi-part array (`{"type":"text",...}` +
/// `{"type":"image_url",...}` per image) — vision-capable models across all
/// 4 providers understand this shape once the message reaches them (see
/// README "Pesquisa: suporte real a imagem/áudio/vídeo por provider").
fn to_wire_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            let mut value = serde_json::to_value(m).unwrap_or_else(|_| json!({}));
            if let Some(obj) = value.as_object_mut() {
                // `display_content` e so pra UI (ver campo em models.rs) — nunca
                // deveria chegar no provider, mesmo quando presente.
                obj.remove("display_content");
                if !m.images.is_empty() {
                    obj.remove("images");
                    let mut blocks = vec![json!({ "type": "text", "text": m.content })];
                    blocks.extend(
                        m.images
                            .iter()
                            .map(|url| json!({ "type": "image_url", "image_url": { "url": url } })),
                    );
                    obj.insert("content".to_string(), serde_json::Value::Array(blocks));
                }
            }
            value
        })
        .collect()
}

/// Escreve no `body` do `/chat/completions` o controle de raciocínio pedido,
/// traduzindo o esforço genérico pro campo que cada provider entende.
///
/// - `None` ("Auto"): não escreve nada — o modelo usa o default dele.
/// - `Some(Off)`: desliga o reasoning de forma explícita. Cada provider
///   desliga de um jeito (pesquisa de APIs verificada, ver testes abaixo):
///   Ollama /v1 mapeia `reasoning_effort:"none"` → `Think=false`; llama.cpp e
///   LM Studio aceitam `reasoning_effort:"none"` e ainda honram
///   `chat_template_kwargs.enable_thinking=false` no template do Qwen3;
///   OpenRouter usa `reasoning.effort:"none"`; em Custom não existe "off"
///   universal, então mandamos `enable_thinking:false` + `chat_template_kwargs`
///   (cobre vLLM/sglang/Qwen/DeepSeek/GLM self-hosted — backends OpenAI
///   estritos não têm como desligar, só baixar a força).
/// - `Some(Low/Medium/High)`: `reasoning_effort` (padrão OpenAI-compat).
fn apply_reasoning(
    body: &mut serde_json::Value,
    kind: ProviderKind,
    effort: Option<ReasoningEffort>,
) {
    match effort {
        None => {}
        Some(ReasoningEffort::Off) => match kind {
            ProviderKind::Ollama => {
                body["reasoning_effort"] = json!("none");
            }
            ProviderKind::LlamaCpp | ProviderKind::LmStudio => {
                body["reasoning_effort"] = json!("none");
                body["chat_template_kwargs"] = json!({ "enable_thinking": false });
            }
            ProviderKind::Openrouter => {
                body["reasoning"] = json!({ "effort": "none" });
            }
            ProviderKind::Custom => {
                body["enable_thinking"] = json!(false);
                body["chat_template_kwargs"] = json!({ "enable_thinking": false });
            }
        },
        Some(effort) => {
            body["reasoning_effort"] = json!(match effort {
                ReasoningEffort::Low => "low",
                ReasoningEffort::Medium => "medium",
                ReasoningEffort::High => "high",
                ReasoningEffort::Off => unreachable!(),
            });
        }
    }
}

/// Result of a full (non-streaming-to-caller) assistant turn: the final
/// assembled message (content + any tool_calls the model requested).
pub async fn chat_stream(
    app: &AppHandle,
    session_id: &str,
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
    reasoning_effort: Option<ReasoningEffort>,
    tool_choice: Option<&str>,
) -> Result<StreamResult> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));

    let mut body = json!({
        "model": model,
        "messages": to_wire_messages(messages),
        "stream": true,
    });
    if !tools.is_empty() {
        body["tools"] = serde_json::to_value(tools)?;
        if let Some(tc) = tool_choice {
            body["tool_choice"] = json!(tc);
        }
    }
    // Controle de raciocínio — ver `apply_reasoning` pra o que cada provider
    // recebe no wire (e por quê).
    apply_reasoning(&mut body, cfg.kind, reasoning_effort);

    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("provider request failed ({status}): {text}"));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut content = String::new();
    // index -> (id, name, arguments) accumulator for streamed tool calls
    let mut tool_calls: Vec<(String, String, String)> = Vec::new();
    let mut usage = StreamUsage::default();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim_end_matches('\r').to_string();
            buf.drain(..=pos);
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data:") {
                continue;
            }
            let payload = line["data:".len()..].trim();
            if payload == "[DONE]" {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(payload) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let delta = &parsed["choices"][0]["delta"];

            if let Some(text) = delta["content"].as_str() {
                if !text.is_empty() {
                    content.push_str(text);
                    let _ = app.emit(
                        "chat:token",
                        StreamTokenEvent {
                            session_id: session_id.to_string(),
                            delta: text.to_string(),
                        },
                    );
                }
            }

            if let Some(thinking) = delta["reasoning_content"].as_str() {
                if !thinking.is_empty() {
                    let _ = app.emit(
                        "chat:thinking_token",
                        StreamTokenEvent {
                            session_id: session_id.to_string(),
                            delta: thinking.to_string(),
                        },
                    );
                }
            }

            if let Some(calls) = delta["tool_calls"].as_array() {
                for call in calls {
                    let idx = call["index"].as_u64().unwrap_or(0) as usize;
                    while tool_calls.len() <= idx {
                        tool_calls.push((String::new(), String::new(), String::new()));
                    }
                    if let Some(id) = call["id"].as_str() {
                        tool_calls[idx].0 = id.to_string();
                    }
                    if let Some(name) = call["function"]["name"].as_str() {
                        tool_calls[idx].1.push_str(name);
                    }
                    if let Some(args) = call["function"]["arguments"].as_str() {
                        tool_calls[idx].2.push_str(args);
                    }
                }
            }

            // Usage vem num chunk separado (choices vazio ou ausente) ou no
            // último chunk antes de [DONE] — providers OpenAI-compatible
            // variam onde colocam, então checamos em todo chunk.
            if let Some(u) = parsed["usage"].as_object() {
                if let Some(pt) = u["prompt_tokens"].as_u64() {
                    usage.prompt_tokens = pt as u32;
                }
                if let Some(ct) = u["completion_tokens"].as_u64() {
                    usage.completion_tokens = ct as u32;
                }
            }
        }
    }

    let final_tool_calls: Option<Vec<ToolCall>> = if tool_calls.is_empty() {
        None
    } else {
        Some(
            tool_calls
                .into_iter()
                .enumerate()
                .map(|(i, (id, name, arguments))| ToolCall {
                    id: if id.is_empty() {
                        format!("call_{i}")
                    } else {
                        id
                    },
                    kind: "function".to_string(),
                    function: ToolCallFunction { name, arguments },
                })
                .collect(),
        )
    };

    Ok(StreamResult {
        message: ChatMessage {
            role: "assistant".to_string(),
            content,
            tool_calls: final_tool_calls,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        },
        usage,
    })
}

pub async fn list_models(cfg: &ProviderConfig, api_key: Option<String>, app_data_dir: &std::path::Path) -> Result<Vec<ModelInfo>> {
    let client = reqwest::Client::new();

    match cfg.kind {
        ProviderKind::Ollama => {
            let base = cfg.base_url.trim_end_matches("/v1").trim_end_matches('/');
            let url = format!("{base}/api/tags");
            let resp = client.get(&url).send().await?;
            let json: serde_json::Value = resp.json().await?;
            let models = json["models"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|m| {
                    let id = m["name"].as_str()?.to_string();
                    Some(ModelInfo {
                        label: id.clone(),
                        id,
                        context_length: None,
                    })
                })
                .collect();
            Ok(models)
        }
        _ => {
            let url = format!("{}/models", cfg.base_url.trim_end_matches('/'));
            let mut req = client.get(&url);
            if let Some(key) = api_key {
                req = req.bearer_auth(key);
            }
            let resp = req.send().await?;
            if !resp.status().is_success() {
                let status = resp.status();
                return Err(anyhow!("failed to list models ({status})"));
            }
            let json: serde_json::Value = resp.json().await?;
            let models = json["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|m| {
                    let id = m["id"].as_str()?.to_string();
                    let api_ctx = m["context_length"]
                        .as_u64()
                        .or_else(|| m["top_provider"]["context_length"].as_u64())
                        .map(|v| v as u32);
                    let context_length = Some(resolve_context_length(app_data_dir, &id, api_ctx));
                    Some(ModelInfo {
                        label: id.clone(),
                        id,
                        context_length,
                    })
                })
                .collect();
            Ok(models)
        }
    }
}

/// Best-effort lookup of a single model's context window, used when
/// starting/switching a session so the context-usage indicator has
/// something real to divide by. Falls back to `None` (caller applies
/// `DEFAULT_CONTEXT_LENGTH`) when the provider doesn't expose it.
pub async fn get_context_length(
    cfg: &ProviderConfig,
    api_key: Option<String>,
    model: &str,
    app_data_dir: &std::path::Path,
) -> Option<u32> {
    let client = reqwest::Client::new();

    match cfg.kind {
        ProviderKind::Custom if cfg.context_length_override.is_some() => cfg.context_length_override,
        ProviderKind::Openrouter | ProviderKind::Custom => {
            let models = list_models(cfg, api_key, app_data_dir).await.ok()?;
            models.into_iter().find(|m| m.id == model)?.context_length
        }
        ProviderKind::Ollama => {
            let base = cfg.base_url.trim_end_matches("/v1").trim_end_matches('/');
            let url = format!("{base}/api/show");
            let resp = client
                .post(&url)
                .json(&json!({ "model": model }))
                .send()
                .await
                .ok()?;
            let json: serde_json::Value = resp.json().await.ok()?;
            let info = json["model_info"].as_object()?;
            info.iter()
                .find(|(k, _)| k.ends_with(".context_length"))
                .and_then(|(_, v)| v.as_u64())
                .map(|v| v as u32)
        }
        ProviderKind::LmStudio => {
            let base = cfg.base_url.trim_end_matches("/v1").trim_end_matches('/');
            let url = format!("{base}/api/v0/models");
            let resp = client.get(&url).send().await.ok()?;
            let json: serde_json::Value = resp.json().await.ok()?;
            json["data"]
                .as_array()?
                .iter()
                .find(|m| m["id"].as_str() == Some(model))
                .and_then(|m| {
                    m["loaded_context_length"]
                        .as_u64()
                        .or_else(|| m["max_context_length"].as_u64())
                })
                .map(|v| v as u32)
        }
        ProviderKind::LlamaCpp => None, // resolved separately from the preset's .ini (see llama_cpp::preset_context_length)
    }
}

/// Best-effort check of whether `model` actually has vision support on this
/// provider — never assume from the model's name, always ask the provider
/// (see README "Pesquisa: suporte real a imagem/áudio/vídeo por provider").
/// `LlamaCpp` isn't handled here on purpose: vision there depends on whether
/// the resolved preset in the fork's `models.ini` references an `mmproj` file,
/// not on anything queryable over HTTP — see
/// `llama_cpp::preset_supports_vision`, called directly by the caller instead.
pub async fn supports_vision(cfg: &ProviderConfig, api_key: Option<String>, model: &str, _app_data_dir: &std::path::Path) -> bool {
    let client = reqwest::Client::new();

    match cfg.kind {
        ProviderKind::Ollama => {
            let base = cfg.base_url.trim_end_matches("/v1").trim_end_matches('/');
            let url = format!("{base}/api/show");
            let Ok(resp) = client
                .post(&url)
                .json(&json!({ "model": model }))
                .send()
                .await
            else {
                return false;
            };
            let Ok(json) = resp.json::<serde_json::Value>().await else {
                return false;
            };
            json["capabilities"]
                .as_array()
                .map(|caps| caps.iter().any(|c| c.as_str() == Some("vision")))
                .unwrap_or(false)
        }
        ProviderKind::Openrouter => {
            let url = format!("{}/models", cfg.base_url.trim_end_matches('/'));
            let mut req = client.get(&url);
            if let Some(key) = api_key {
                req = req.bearer_auth(key);
            }
            let Ok(resp) = req.send().await else {
                return false;
            };
            let Ok(json) = resp.json::<serde_json::Value>().await else {
                return false;
            };
            json["data"]
                .as_array()
                .and_then(|models| models.iter().find(|m| m["id"].as_str() == Some(model)))
                .and_then(|m| m["architecture"]["input_modalities"].as_array())
                .map(|mods| mods.iter().any(|v| v.as_str() == Some("image")))
                .unwrap_or(false)
        }
        ProviderKind::LmStudio => {
            let base = cfg.base_url.trim_end_matches("/v1").trim_end_matches('/');
            let url = format!("{base}/api/v0/models");
            let Ok(resp) = client.get(&url).send().await else {
                return false;
            };
            let Ok(json) = resp.json::<serde_json::Value>().await else {
                return false;
            };
            json["data"]
                .as_array()
                .and_then(|models| models.iter().find(|m| m["id"].as_str() == Some(model)))
                .map(|m| m["type"].as_str() == Some("vlm"))
                .unwrap_or(false)
        }
        ProviderKind::LlamaCpp => false,
        // Provider customizado generico - `/models` no formato OpenAI padrao
        // nao tem um campo comum de modalidade (o `architecture.input_modalities`
        // usado acima e especifico do OpenRouter), entao nao ha como confirmar
        // vision automaticamente sem hardcodar o formato de resposta de um
        // vendor especifico. Em vez de assumir "nao suporta" sempre, confia
        // na confirmacao manual do usuario (`CustomProviderConfig.supports_vision`,
        // ver tela de Configuracoes) - continua false por padrao ate o
        // usuario marcar explicitamente que aquela conexao aceita imagem.
        ProviderKind::Custom => cfg.supports_vision_override,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_message(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: Vec::new(),
            display_content: None,
        }
    }

    #[test]
    fn to_wire_messages_leaves_plain_messages_as_a_string_content() {
        let messages = vec![text_message("user", "oi")];
        let wire = to_wire_messages(&messages);
        assert_eq!(wire[0]["content"], json!("oi"));
        assert!(wire[0].get("images").is_none());
    }

    #[test]
    fn to_wire_messages_turns_images_into_openai_content_blocks() {
        let mut m = text_message("user", "o que tem nessa imagem?");
        m.images = vec!["data:image/png;base64,AAAA".to_string()];
        let wire = to_wire_messages(&[m]);

        let content = wire[0]["content"]
            .as_array()
            .expect("content deveria virar array");
        assert_eq!(
            content.len(),
            2,
            "esperava 1 bloco de texto + 1 de imagem: {content:?}"
        );
        assert_eq!(content[0]["type"], json!("text"));
        assert_eq!(content[0]["text"], json!("o que tem nessa imagem?"));
        assert_eq!(content[1]["type"], json!("image_url"));
        assert_eq!(
            content[1]["image_url"]["url"],
            json!("data:image/png;base64,AAAA")
        );
        assert!(
            wire[0].get("images").is_none(),
            "campo images interno nao deveria vazar pro wire format"
        );
    }

    #[test]
    fn to_wire_messages_handles_multiple_images_in_order() {
        let mut m = text_message("user", "compare essas duas");
        m.images = vec![
            "data:image/png;base64,AAA".to_string(),
            "data:image/jpeg;base64,BBB".to_string(),
        ];
        let wire = to_wire_messages(&[m]);
        let content = wire[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 3);
        assert_eq!(
            content[1]["image_url"]["url"],
            json!("data:image/png;base64,AAA")
        );
        assert_eq!(
            content[2]["image_url"]["url"],
            json!("data:image/jpeg;base64,BBB")
        );
    }

    #[test]
    fn to_wire_messages_never_leaks_display_content_to_the_provider() {
        // display_content e so pra UI (mensagens com anexo de documento
        // mostram so o texto digitado + nome do anexo, mas o modelo precisa
        // do texto extraido inteiro em `content`) - nunca deveria aparecer
        // no payload que sai pro provider, com ou sem imagem na mensagem.
        let mut plain = text_message(
            "user",
            "### Anexo: relatorio.pdf\n\ntexto extraido inteiro...",
        );
        plain.display_content = Some("📎 relatorio.pdf".to_string());
        let wire = to_wire_messages(&[plain]);
        assert!(wire[0].get("display_content").is_none());
        assert_eq!(
            wire[0]["content"],
            json!("### Anexo: relatorio.pdf\n\ntexto extraido inteiro...")
        );

        let mut with_image = text_message("user", "descreva essa imagem");
        with_image.images = vec!["data:image/png;base64,AAAA".to_string()];
        with_image.display_content = Some("descreva essa imagem\n\n🖼️ foto.png".to_string());
        let wire = to_wire_messages(&[with_image]);
        assert!(wire[0].get("display_content").is_none());
    }

    fn empty_body() -> serde_json::Value {
        json!({ "model": "m", "messages": [], "stream": true })
    }

    #[test]
    fn apply_reasoning_auto_sends_nothing() {
        // Auto = não envia campo nenhum (modelo usa o default dele).
        let mut body = empty_body();
        apply_reasoning(&mut body, ProviderKind::LlamaCpp, None);
        assert!(body.get("reasoning_effort").is_none());
        assert!(body.get("chat_template_kwargs").is_none());
        assert!(body.get("enable_thinking").is_none());
        assert!(body.get("reasoning").is_none());
    }

    #[test]
    fn apply_reasoning_strength_uses_reasoning_effort() {
        let mut body = empty_body();
        apply_reasoning(&mut body, ProviderKind::Custom, Some(ReasoningEffort::Medium));
        assert_eq!(body["reasoning_effort"], json!("medium"));
    }

    #[test]
    fn apply_reasoning_off_ollama_sends_reasoning_effort_none() {
        // Ollama /v1 mapeia reasoning_effort:"none" -> Think=false.
        let mut body = empty_body();
        apply_reasoning(&mut body, ProviderKind::Ollama, Some(ReasoningEffort::Off));
        assert_eq!(body["reasoning_effort"], json!("none"));
        assert!(body.get("chat_template_kwargs").is_none());
    }

    #[test]
    fn apply_reasoning_off_llamacpp_and_lmstudio_add_template_kwarg() {
        for kind in [ProviderKind::LlamaCpp, ProviderKind::LmStudio] {
            let mut body = empty_body();
            apply_reasoning(&mut body, kind, Some(ReasoningEffort::Off));
            assert_eq!(body["reasoning_effort"], json!("none"));
            assert_eq!(
                body["chat_template_kwargs"]["enable_thinking"],
                json!(false)
            );
        }
    }

    #[test]
    fn apply_reasoning_off_openrouter_uses_reasoning_object() {
        let mut body = empty_body();
        apply_reasoning(&mut body, ProviderKind::Openrouter, Some(ReasoningEffort::Off));
        assert_eq!(body["reasoning"]["effort"], json!("none"));
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn apply_reasoning_off_custom_uses_enable_thinking() {
        // Custom não tem "off" universal; mandamos os campos que os servidores
        // OpenAI-compat de modelos thinking (vLLM/sglang/Qwen/DeepSeek) honram.
        let mut body = empty_body();
        apply_reasoning(&mut body, ProviderKind::Custom, Some(ReasoningEffort::Off));
        assert_eq!(body["enable_thinking"], json!(false));
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"], json!(false));
        assert!(body.get("reasoning_effort").is_none());
    }
}
