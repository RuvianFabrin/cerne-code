// Voz: ler
// resposta em voz alta (TTS) e microfone no composer (STT). Escopo reduzido
// a pedido explícito do usuário ("focar no ler texto, e no falar texto") —
// SEM redesenho do "+", SEM modo de conversa em tempo real, SEM configurar
// modelo de STT/TTS na UI (usa um default fixo documentado abaixo). Ambos
// via OpenRouter (mesma chave que o resto do app já usa) — únicos endpoints
// de áudio dedicados que o app tinha à disposição sem inventar mais um
// provider/chave nova pro usuário configurar.
//
// Endpoints confirmados na documentação oficial da OpenRouter (2026-08-17,
// consultada ao vivo — não testados contra uma chave real ainda, é a
// primeira vez que o app fala com esses dois endpoints):
// - TTS: POST /api/v1/audio/speech — body {model, input, response_format},
//   resposta é o áudio cru (bytes), não JSON.
// - STT: POST /api/v1/audio/transcriptions — body
//   {model, input_audio: {data: <base64>, format}}, resposta JSON {text, usage}.

use anyhow::{anyhow, Result};
use base64::Engine;
use serde::Deserialize;
use std::error::Error as StdError;

/// `reqwest::Error` Display sozinho mostra só a camada mais externa (ex:
/// "error sending request for url (...)"), sem a causa raiz (DNS, TLS,
/// timeout, conexão recusada) — inútil pra diagnosticar. Percorre a cadeia
/// de `source()` inteira e concatena, igual o usuário viu faltando no
/// DevTools (2026-08-19: erro cortado, só "error sending request for url").
fn describe_reqwest_error(e: &reqwest::Error) -> String {
    let mut msg = e.to_string();
    let mut source = (e as &dyn StdError).source();
    while let Some(s) = source {
        msg.push_str(" -> ");
        msg.push_str(&s.to_string());
        source = s.source();
    }
    msg
}

/// Modelo TTS default — trocado em 2026-08-19 (usuário reportou, com print
/// da própria página de modelos da OpenRouter, que `gpt-4o-mini-tts` não
/// existe mais na categoria "Speech"). `hexgrad/kokoro-82m` confirmado ao
/// vivo na doc da OpenRouter: existe, é o mais barato listado ($0.62/M
/// caracteres) com qualidade boa (open-weight, usado amplamente). Ver
/// `DEFAULT_TTS_VOICE` — a OpenRouter exige `voice` explícito pra provider
/// sem default próprio (Kokoro é um desses).
pub const DEFAULT_TTS_MODEL: &str = "hexgrad/kokoro-82m";
/// Voz default do Kokoro (americana feminina, a mais citada como padrão nas
/// vozes do projeto open-source original) — configurável em Configurações
/// caso não seja a ideal para quem usa outro provider/modelo de TTS.
pub const DEFAULT_TTS_VOICE: &str = "af_heart";
pub const DEFAULT_STT_MODEL: &str = "openai/whisper-1";

/// Sintetiza `text` em áudio via um endpoint OpenAI-compatible
/// (`{base_url}/audio/speech`) — generalizado a partir da OpenRouter
/// original pra permitir qualquer conexão (local ou com chave) já
/// configurada no app, a pedido do usuário. Devolve os bytes de áudio (mp3)
/// já em base64 — mais simples de mandar pro frontend via Tauri (JSON) do
/// que um array de bytes cru, e o frontend só precisa montar uma data URI
/// `data:audio/mpeg;base64,<...>` pra tocar direto num `<audio>`.
pub async fn synthesize_speech(
    base_url: &str,
    api_key: Option<&str>,
    text: &str,
    model: &str,
    voice: &str,
) -> Result<String> {
    if text.trim().is_empty() {
        return Err(anyhow!("texto vazio"));
    }
    let client = reqwest::Client::new();
    let mut body = serde_json::json!({
        "model": model,
        "input": text,
        "response_format": "mp3",
    });
    // Muitos providers (Kokoro incluso) não têm voz default no lado deles —
    // a OpenRouter rejeita a requisição com erro de validação se `voice`
    // vier vazio. Só omite quando o usuário deixa em branco de propósito
    // (provider que já tem default próprio).
    if !voice.trim().is_empty() {
        body["voice"] = serde_json::json!(voice);
    }
    let url = format!("{}/audio/speech", base_url.trim_end_matches('/'));
    // 90s (não 30s) — Kokoro é hospedado via DeepInfra (serverless), que
    // pode ter cold start de dezenas de segundos na primeira chamada depois
    // de um tempo ocioso, fora o tempo real de síntese pra textos longos.
    // Achado testando ao vivo, 2026-08-19: "operation timed out" em 30s.
    let mut req = client.post(url).json(&body).timeout(std::time::Duration::from_secs(90));
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| anyhow!("nao foi possivel contatar o provider: {}", describe_reqwest_error(&e)))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("TTS falhou ({status}): {err_text}"));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| anyhow!("falha lendo o audio devolvido: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Detecta o idioma provável de `text` por heurística leve (contagem de
/// palavras comuns + faixa Unicode pra CJK) — sem baixar nenhum modelo de
/// detecção de idioma só pra isso. Cobre só os 4 idiomas que o próprio app
/// já suporta na UI (pt/en/es/zh, ver `i18n.ts::SUPPORTED_LOCALES`), já que
/// é só isso que `kokoro_voice_for_language` sabe mapear pra uma voz.
/// Pedido do usuário, 2026-08-19: "está em inglês, tem como identificar o
/// texto e falar na lingua correta".
pub fn detect_language(text: &str) -> &'static str {
    if text.chars().any(|c| {
        let cp = c as u32;
        (0x4E00..=0x9FFF).contains(&cp) // CJK Unified Ideographs
            || (0x3400..=0x4DBF).contains(&cp) // CJK extension A
    }) {
        return "zh";
    }
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return "en";
    }
    // Palavras de alto sinal (raras de aparecer só por coincidência num
    // texto de outro idioma) pesam mais que as genéricas/compartilhadas
    // (ex: "que"/"para" existem em pt E es).
    const PT_STRONG: &[&str] = &["não", "você", "também", "então", "está", "são", "isso"];
    const PT_WEAK: &[&str] = &["de", "que", "para", "com", "uma", "os", "as", "muito", "como"];
    const ES_STRONG: &[&str] = &["no", "muy", "también", "está", "son", "esto", "usted"];
    const ES_WEAK: &[&str] = &["de", "que", "para", "con", "una", "los", "las", "como"];
    const EN_STRONG: &[&str] = &["the", "you", "this", "that", "with", "have", "will", "would"];
    const EN_WEAK: &[&str] = &["and", "is", "for", "are", "was", "not", "but", "they"];

    let score = |strong: &[&str], weak: &[&str]| -> u32 {
        words
            .iter()
            .map(|w| {
                if strong.contains(w) {
                    3
                } else if weak.contains(w) {
                    1
                } else {
                    0
                }
            })
            .sum()
    };
    let pt_score = score(PT_STRONG, PT_WEAK);
    let es_score = score(ES_STRONG, ES_WEAK);
    let en_score = score(EN_STRONG, EN_WEAK);

    if pt_score == 0 && es_score == 0 && en_score == 0 {
        return "en"; // sem sinal nenhum (texto curto, números, código) — default
    }
    if pt_score >= es_score && pt_score >= en_score {
        "pt"
    } else if es_score >= en_score {
        "es"
    } else {
        "en"
    }
}

/// Mapeia um idioma detectado (`pt`/`en`/`es`/`zh`) pra uma voz do Kokoro
/// (ver `DEFAULT_TTS_VOICE`) — vozes femininas por padrão, mesma escolha
/// que `DEFAULT_TTS_VOICE` já fazia. IDs confirmados na doc/comunidade do
/// Kokoro-82M (2026-08-19): prefixo de idioma + gênero + nome da voz.
fn kokoro_voice_for_language(lang: &str) -> &'static str {
    match lang {
        "pt" => "pf_dora",
        "es" => "ef_dora",
        "zh" => "zf_xiaobei",
        _ => DEFAULT_TTS_VOICE, // en (e qualquer outro caso não mapeado)
    }
}

/// Resolve qual voz usar pra `text` — se `auto_language` estiver ligado E o
/// modelo for algum Kokoro (único que o app sabe mapear idioma→voz),
/// detecta o idioma e escolhe a voz correspondente; senão usa a voz
/// configurada manualmente como está.
pub fn resolve_voice(model: &str, configured_voice: &str, text: &str, auto_language: bool) -> String {
    if auto_language && model.starts_with("hexgrad/kokoro") {
        kokoro_voice_for_language(detect_language(text)).to_string()
    } else {
        configured_voice.to_string()
    }
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Transcreve áudio (já em base64, formato indicado em `format` — ex:
/// "webm", "wav") via um endpoint OpenAI-compatible
/// (`{base_url}/audio/transcriptions`). Devolve o texto transcrito.
pub async fn transcribe_audio(
    base_url: &str,
    api_key: Option<&str>,
    audio_base64: &str,
    format: &str,
    model: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "input_audio": {
            "data": audio_base64,
            "format": format,
        },
    });
    let url = format!("{}/audio/transcriptions", base_url.trim_end_matches('/'));
    let mut req = client.post(url).json(&body).timeout(std::time::Duration::from_secs(30));
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| anyhow!("nao foi possivel contatar o provider: {}", describe_reqwest_error(&e)))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("STT falhou ({status}): {err_text}"));
    }
    let parsed: TranscriptionResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta inesperada do provider: {e}"))?;
    Ok(parsed.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_language_recognizes_english() {
        assert_eq!(detect_language("The quick brown fox jumps over the lazy dog, and you will see this."), "en");
    }

    #[test]
    fn detect_language_recognizes_portuguese() {
        assert_eq!(detect_language("Você não está entendendo, isso também é muito importante."), "pt");
    }

    #[test]
    fn detect_language_recognizes_spanish() {
        assert_eq!(detect_language("Usted no está entendiendo, esto también es muy importante."), "es");
    }

    #[test]
    fn detect_language_recognizes_chinese_by_script() {
        assert_eq!(detect_language("你好，今天天气怎么样"), "zh");
    }

    #[test]
    fn detect_language_defaults_to_english_for_no_signal() {
        assert_eq!(detect_language("xyz 123 foo bar"), "en");
    }

    #[test]
    fn kokoro_voice_map_covers_all_four_locales() {
        assert_eq!(kokoro_voice_for_language("pt"), "pf_dora");
        assert_eq!(kokoro_voice_for_language("es"), "ef_dora");
        assert_eq!(kokoro_voice_for_language("zh"), "zf_xiaobei");
        assert_eq!(kokoro_voice_for_language("en"), DEFAULT_TTS_VOICE);
    }

    #[test]
    fn resolve_voice_uses_configured_voice_when_auto_language_off() {
        assert_eq!(resolve_voice("hexgrad/kokoro-82m", "am_adam", "Você está bem?", false), "am_adam");
    }

    #[test]
    fn resolve_voice_uses_configured_voice_for_non_kokoro_models() {
        assert_eq!(resolve_voice("openai/gpt-4o-mini-tts", "alloy", "Você está bem?", true), "alloy");
    }

    #[test]
    fn resolve_voice_auto_detects_for_kokoro() {
        assert_eq!(resolve_voice("hexgrad/kokoro-82m", "am_adam", "Você não está entendendo isso.", true), "pf_dora");
    }
}
