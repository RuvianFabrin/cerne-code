pub mod custom;
pub mod llama_cpp;

use crate::models::{
    ChatMessage, ModelInfo, ProviderConfig, ProviderKind, ToolCall, ToolCallFunction, ToolSpec,
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
) -> Result<ChatMessage> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));

    let mut body = json!({
        "model": model,
        "messages": to_wire_messages(messages),
        "stream": true,
    });
    if !tools.is_empty() {
        body["tools"] = serde_json::to_value(tools)?;
    }

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

    Ok(ChatMessage {
        role: "assistant".to_string(),
        content,
        tool_calls: final_tool_calls,
        tool_call_id: None,
        name: None,
        images: Vec::new(),
        display_content: None,
    })
}

pub async fn list_models(cfg: &ProviderConfig, api_key: Option<String>) -> Result<Vec<ModelInfo>> {
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
                    let context_length = m["context_length"]
                        .as_u64()
                        .or_else(|| m["top_provider"]["context_length"].as_u64())
                        .map(|v| v as u32);
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
) -> Option<u32> {
    let client = reqwest::Client::new();

    match cfg.kind {
        ProviderKind::Custom if cfg.context_length_override.is_some() => cfg.context_length_override,
        ProviderKind::Openrouter | ProviderKind::Custom => {
            let models = list_models(cfg, api_key).await.ok()?;
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
pub async fn supports_vision(cfg: &ProviderConfig, api_key: Option<String>, model: &str) -> bool {
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
}
