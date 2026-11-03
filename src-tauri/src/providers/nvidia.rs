//! Ajustes específicos para o endpoint NVIDIA NIM compatível com OpenAI.

use crate::models::ReasoningEffort;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelMetadata {
    pub context_length: u32,
    pub supports_vision: bool,
}

pub const RESPONSE_TIMEOUT_SECS: u64 = 180;

pub fn is_endpoint(base_url: &str) -> bool {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| host == "integrate.api.nvidia.com")
}

/// Metadados publicados pela NVIDIA para modelos cujo /models não descreve
/// modalidades e contexto em um formato legível pelo Cerne.
pub fn model_metadata(model: &str) -> Option<ModelMetadata> {
    match model {
        "deepseek-ai/deepseek-v4.1-flash" => Some(ModelMetadata {
            context_length: 1_048_576,
            supports_vision: true,
        }),
        "deepseek-ai/deepseek-v4-flash" | "deepseek-ai/deepseek-v4-flash-0731" => {
            Some(ModelMetadata {
                context_length: 1_048_576,
                supports_vision: false,
            })
        }
        "moonshotai/kimi-k3" => Some(ModelMetadata {
            context_length: 1_048_576,
            supports_vision: true,
        }),
        "moonshotai/kimi-k2.5" => Some(ModelMetadata {
            context_length: 262_144,
            supports_vision: true,
        }),
        _ => None,
    }
}

/// Os três modelos usam controles diferentes, conforme a API NVIDIA:
/// DeepSeek V4.1 usa números; Kimi K3 aceita low/high/max; Kimi K2.5 só
/// documenta `thinking: {type: disabled}` para o modo Instant.
pub fn apply_reasoning(body: &mut Value, model: &str, effort: Option<ReasoningEffort>) {
    match model {
        "deepseek-ai/deepseek-v4.1-flash" => {
            let value = match effort {
                Some(ReasoningEffort::Off | ReasoningEffort::Low) => Some(json!(1)),
                Some(ReasoningEffort::Medium) => Some(json!(50)),
                Some(ReasoningEffort::High) => Some(json!(100)),
                Some(ReasoningEffort::On) | None => None,
            };
            if let Some(value) = value {
                body["reasoning_effort"] = value;
            }
        }
        "moonshotai/kimi-k3" => {
            let value = match effort {
                Some(ReasoningEffort::Off | ReasoningEffort::Low | ReasoningEffort::Medium) => {
                    Some("low")
                }
                Some(ReasoningEffort::High) => Some("high"),
                Some(ReasoningEffort::On) | None => None,
            };
            if let Some(value) = value {
                body["reasoning_effort"] = json!(value);
            }
        }
        "moonshotai/kimi-k2.5" => {
            if effort == Some(ReasoningEffort::Off) {
                body["thinking"] = json!({ "type": "disabled" });
            }
        }
        _ => {}
    }
}

pub fn rejected_reasoning_parameter(status: reqwest::StatusCode, body: &str) -> bool {
    if status != reqwest::StatusCode::BAD_REQUEST
        && status != reqwest::StatusCode::UNPROCESSABLE_ENTITY
    {
        return false;
    }
    let body = body.to_ascii_lowercase();
    (body.contains("reasoning_effort")
        || body.contains("reasoning effort")
        || body.contains("thinking"))
        && [
            "unsupported",
            "unknown",
            "invalid",
            "not allowed",
            "not permitted",
            "unrecognized",
            "not supported",
            "extra input",
            "extra field",
            "must be",
            "expected",
            "between",
            "integer",
            "range",
        ]
        .iter()
        .any(|word| body.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_the_nvidia_api_host() {
        assert!(is_endpoint("https://integrate.api.nvidia.com/v1/"));
        assert!(!is_endpoint("https://api.openai.com/v1"));
        assert!(!is_endpoint(
            "https://not-nvidia.example/integrate.api.nvidia.com"
        ));
    }

    #[test]
    fn has_published_deepseek_v41_metadata() {
        let metadata = model_metadata("deepseek-ai/deepseek-v4.1-flash").unwrap();
        assert_eq!(metadata.context_length, 1_048_576);
        assert!(metadata.supports_vision);
    }

    #[test]
    fn metadata_has_context_and_image_support_for_selected_large_models() {
        assert_eq!(
            model_metadata("moonshotai/kimi-k3").unwrap().context_length,
            1_048_576
        );
        assert!(
            model_metadata("moonshotai/kimi-k3")
                .unwrap()
                .supports_vision
        );
        assert_eq!(
            model_metadata("moonshotai/kimi-k2.5")
                .unwrap()
                .context_length,
            262_144
        );
        assert!(
            model_metadata("moonshotai/kimi-k2.5")
                .unwrap()
                .supports_vision
        );
    }

    #[test]
    fn uses_model_specific_nvidia_reasoning_parameters() {
        let mut deepseek = json!({});
        apply_reasoning(
            &mut deepseek,
            "deepseek-ai/deepseek-v4.1-flash",
            Some(ReasoningEffort::High),
        );
        assert_eq!(deepseek["reasoning_effort"], json!(100));

        let mut kimi3 = json!({});
        apply_reasoning(
            &mut kimi3,
            "moonshotai/kimi-k3",
            Some(ReasoningEffort::High),
        );
        assert_eq!(kimi3["reasoning_effort"], json!("high"));

        let mut kimi25 = json!({});
        apply_reasoning(
            &mut kimi25,
            "moonshotai/kimi-k2.5",
            Some(ReasoningEffort::Off),
        );
        assert_eq!(kimi25["thinking"]["type"], json!("disabled"));

        let mut other = json!({});
        apply_reasoning(&mut other, "vendor/unknown", Some(ReasoningEffort::High));
        assert!(other.as_object().unwrap().is_empty());
    }

    #[test]
    fn retries_only_when_nvidia_rejects_reasoning_parameter() {
        assert!(rejected_reasoning_parameter(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "reasoning_effort must be an integer between 1 and 100"
        ));
        assert!(!rejected_reasoning_parameter(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "reasoning_effort invalid"
        ));
        assert!(!rejected_reasoning_parameter(
            reqwest::StatusCode::BAD_REQUEST,
            "invalid image format"
        ));
    }
}
