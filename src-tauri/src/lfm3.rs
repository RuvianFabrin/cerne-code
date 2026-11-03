//! LiquidAI LFM3: suporte aos 5 modelos do blog lfm3 via o router llama-server
//! (mesmo fork que o resto do llama.cpp local, ver `providers::llama_cpp`).
//!
//! Os modelos são identificados pelo nome da seção no `models.ini` do fork
//! (ex: `[lfm3-vl-2b]`), que é também o campo `model` da requisição. Os
//! caminhos de `model`/`mmproj` vêm desse mesmo arquivo — o Cerne não assume
//! onde o usuário instalou nada.
//!
//! Papel de cada um:
//! - `lfm3-1.7b-text`: chat só com texto.
//! - `lfm3-vl-2b`: chat com imagem.
//! - `lfm3-audio-2b`: chat com áudio (ASR) e fala (via `llama-tts`).
//! - `lfm3-omni-2b`: chat com imagem, áudio e vídeo (sem raciocínio).
//! - `d1-omni-2b`: NÃO é chat. Responde em `/v1/systemone` com decisões
//!   calibradas (sim/não, escolha, nota). Não gera tokens; serve pra
//!   classificar/roteirizar antes de chamar um modelo de chat.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Nome da seção do modelo de decisão (não aparece na lista de chat).
pub const DECISION_MODEL: &str = "d1-omni-2b";
/// Modelo usado para fala (TTS) e áudio de entrada.
pub const AUDIO_MODEL: &str = "lfm3-audio-2b";

/// Seções reconhecidas, na ordem de exibição na UI.
pub const SECTIONS: [&str; 5] = [
    "lfm3-1.7b-text",
    "lfm3-vl-2b",
    "lfm3-audio-2b",
    "lfm3-omni-2b",
    DECISION_MODEL,
];

/// Capacidades de entrada/saída de cada seção. Usado pra marcar `supports_vision`
/// e `supports_audio` na lista de modelos, já que o `mmproj` do Audio é um
/// projetor de áudio e não de imagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modalities {
    pub image: bool,
    pub audio: bool,
    pub speech_out: bool,
    pub chat: bool,
}

pub fn modalities(section: &str) -> Option<Modalities> {
    let m = |image, audio, speech_out, chat| Modalities { image, audio, speech_out, chat };
    Some(match section {
        "lfm3-1.7b-text" => m(false, false, false, true),
        "lfm3-vl-2b" => m(true, false, false, true),
        "lfm3-audio-2b" => m(false, true, true, true),
        "lfm3-omni-2b" => m(true, true, true, true),
        DECISION_MODEL => m(true, true, false, false),
        _ => return None,
    })
}

pub fn is_decision_model(section: &str) -> bool {
    section == DECISION_MODEL
}

/// Idiomas que os model cards do LFM3 listam (interseção do Audio e do Omni),
/// com o nome que vai no prompt de ASR. `auto` deixa o modelo inferir.
pub const LANGUAGES: [(&str, &str); 12] = [
    ("pt", "Portuguese"),
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("zh", "Chinese"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ru", "Russian"),
    ("ar", "Arabic"),
    ("nl", "Dutch"),
];

pub fn language_name(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

/// Prompt de sistema para transcrever áudio. Idioma desconhecido ou `auto`
/// cai no `Perform ASR.` sem idioma, como o próprio model card sugere.
pub fn asr_prompt(lang: &str) -> String {
    match language_name(lang) {
        Some(name) => format!("Perform ASR in {name}."),
        None => "Perform ASR.".to_string(),
    }
}

/// Código de idioma para `llama-tts --tts-lang`. Só aceita os códigos da
/// lista; qualquer outra coisa volta pro padrão `en` do próprio llama-tts.
pub fn tts_lang(code: &str) -> &'static str {
    LANGUAGES.iter().find(|(c, _)| *c == code).map(|(c, _)| *c).unwrap_or("en")
}

/// Voz do `llama-tts`: `female` (padrão) ou `male`.
pub fn tts_voice(voice: &str) -> &'static str {
    if voice == "male" { "male" } else { "female" }
}

/// Caminhos `model` e `mmproj` de uma seção do `models.ini`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SectionPaths {
    pub model: Option<String>,
    pub mmproj: Option<String>,
}

pub fn section_paths(models_ini: &Path, section: &str) -> Result<Option<SectionPaths>> {
    let mut ini = configparser::ini::Ini::new();
    ini.load(models_ini)
        .map_err(|e| anyhow!("falha ao ler {}: {e}", models_ini.display()))?;
    let has_section = ini.sections().iter().any(|s| s == section);
    if !has_section {
        return Ok(None);
    }
    Ok(Some(SectionPaths {
        model: ini.get(section, "model"),
        mmproj: ini.get(section, "mmproj"),
    }))
}

/// Resolve o `llama-tts` ao lado do `llama-server` do fork (mesma build).
pub fn llama_tts_exe(server_exe: &str) -> PathBuf {
    Path::new(server_exe)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(if cfg!(windows) { "llama-tts.exe" } else { "llama-tts" })
}

/// Monta o corpo de `/v1/systemone`. `questions` segue o formato do endpoint
/// (ver model card do d1-Omni-2B). `model` é obrigatório: sem ele o router
/// responde 400.
pub fn classify_body(state: &str, questions: Value, images: Vec<String>, audios: Vec<String>) -> Value {
    let mut body = json!({
        "model": DECISION_MODEL,
        "state": state,
        "questions": questions,
    });
    if !images.is_empty() {
        body["images"] = json!(images);
    }
    if !audios.is_empty() {
        body["files"] = json!(audios);
    }
    body
}

/// Chama `/v1/systemone` no router local.
pub async fn classify(port: u16, body: &Value) -> Result<Value> {
    let url = format!("http://127.0.0.1:{port}/v1/systemone");
    let response = reqwest::Client::new()
        .post(&url)
        .json(body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .with_context(|| format!("falha ao chamar {url}"))?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(anyhow!("d1-Omni respondeu {status}: {text}"));
    }
    Ok(serde_json::from_str(&text)?)
}

/// Chama `/v1/chat/completions` do router local, sem ferramentas. Usado pelos
/// testes e pela conversa por voz da tela LFM3, que precisam do áudio
/// direto no modelo sem passar pela sessão de chat.
pub async fn chat(port: u16, model: &str, messages: Vec<Value>, max_tokens: u32) -> Result<String> {
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let body = json!({
        "model": model,
        "messages": messages,
        "temperature": 0,
        "max_tokens": max_tokens,
    });
    let response = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .with_context(|| format!("falha ao chamar {url}"))?;
    let status = response.status();
    let value: Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow!("{model} respondeu {status}: {value}"));
    }
    let message = &value["choices"][0]["message"];
    let content = message["content"].as_str().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(anyhow!(
            "{model} não devolveu texto (aumente max_tokens: o modelo raciocina antes de responder)"
        ));
    }
    Ok(content)
}

/// Gera fala com `llama-tts` (modelo de áudio do LFM3) e devolve os bytes do
/// WAV. O arquivo sai numa pasta temporária que é apagada logo depois.
pub async fn speak(
    llama_tts: &Path,
    model: &Path,
    mmproj: &Path,
    text: &str,
    lang: &str,
    voice: &str,
) -> Result<Vec<u8>> {
    if !llama_tts.is_file() {
        return Err(anyhow!("llama-tts não encontrado em {}", llama_tts.display()));
    }
    let dir = std::env::temp_dir().join(format!("cerne-lfm3-tts-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("out.wav");
    let result = async {
        let output = tokio::process::Command::new(llama_tts)
            .arg("-m").arg(model)
            .arg("-mm").arg(mmproj)
            .arg("-p").arg(text)
            .arg("--tts-lang").arg(tts_lang(lang))
            .arg("--tts-voice").arg(tts_voice(voice))
            .arg("-o").arg(&out)
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(Duration::from_secs(600), output)
            .await
            .map_err(|_| anyhow!("llama-tts passou de 10 minutos"))??;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr.lines().rev().take(5).collect::<Vec<_>>().join("\n");
            return Err(anyhow!("llama-tts falhou: {tail}"));
        }
        Ok(std::fs::read(&out)?)
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_modalities_and_decision_is_not_chat() {
        for s in SECTIONS {
            assert!(modalities(s).is_some(), "{s} sem modalidades");
        }
        assert!(!modalities(DECISION_MODEL).unwrap().chat);
        assert!(modalities("lfm3-omni-2b").unwrap().audio);
        assert!(!modalities("lfm3-vl-2b").unwrap().audio, "VL não ouve áudio");
        assert!(modalities("lfm3-audio-2b").unwrap().speech_out);
        assert!(modalities("qwen").is_none());
    }

    #[test]
    fn asr_prompt_uses_language_or_falls_back_to_auto() {
        assert_eq!(asr_prompt("pt"), "Perform ASR in Portuguese.");
        assert_eq!(asr_prompt("auto"), "Perform ASR.");
        assert_eq!(asr_prompt("xx"), "Perform ASR.");
    }

    #[test]
    fn tts_lang_and_voice_fall_back_to_llama_defaults() {
        assert_eq!(tts_lang("pt"), "pt");
        assert_eq!(tts_lang("klingon"), "en");
        assert_eq!(tts_voice("male"), "male");
        assert_eq!(tts_voice("qualquer"), "female");
    }

    #[test]
    fn classify_body_always_names_the_decision_model() {
        let body = classify_body("texto", json!({"r": {"type": "noul", "instructions": "?"}}), vec![], vec![]);
        assert_eq!(body["model"], DECISION_MODEL);
        assert!(body.get("images").is_none());
        assert!(body.get("files").is_none());
    }

    #[test]
    fn section_paths_reads_model_and_mmproj_from_ini() {
        let dir = std::env::temp_dir().join(format!("cerne-lfm3-ini-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let ini = dir.join("models.ini");
        std::fs::write(
            &ini,
            "version = 1\n[lfm3-audio-2b]\nmodel = C:/m/audio.gguf\nmmproj = C:/m/mm.gguf\n",
        )
        .unwrap();
        let found = section_paths(&ini, "lfm3-audio-2b").unwrap().unwrap();
        assert_eq!(found.model.as_deref(), Some("C:/m/audio.gguf"));
        assert_eq!(found.mmproj.as_deref(), Some("C:/m/mm.gguf"));
        assert!(section_paths(&ini, "lfm3-vl-2b").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn llama_tts_sits_next_to_server() {
        let p = llama_tts_exe("C:/fork/bin/llama-server.exe");
        assert_eq!(p.file_name().unwrap().to_str().unwrap().starts_with("llama-tts"), true);
        assert!(p.starts_with("C:/fork/bin"));
    }

}
