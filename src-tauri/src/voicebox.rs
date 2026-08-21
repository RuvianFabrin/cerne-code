//! Integração com o Voicebox (https://github.com/jamiepine/voicebox) — app
//! open source separado que o usuário já roda na própria máquina (Tauri+Rust
//! por fora, sidecar Python/PyTorch por dentro pra inferência real de TTS/STT
//! em vários engines: Kokoro, Qwen TTS, Chatterbox, TADA, LuxTTS...). Cerne
//! Code NÃO reimplementa nada disso — só fala a API REST local dele
//! (`http://127.0.0.1:17493` por padrão), confirmada ao vivo lendo o
//! `/openapi.json` do servidor rodando na máquina do usuário, 2026-08-19.
//!
//! Diferente de `audio.rs` (endpoints OpenAI-compatible `/audio/speech` e
//! `/audio/transcriptions`), o wire format do Voicebox é próprio:
//! - TTS: `POST /speak {text, profile, language?}` devolve um
//!   `GenerationResponse` JSON (não o áudio direto) com `status` — se não
//!   vier `"completed"` na resposta inicial, `GET
//!   /generate/{id}/status` é um stream SSE que fica aberto até a geração
//!   terminar (ou falhar); só DEPOIS disso `GET /audio/{id}` devolve os
//!   bytes de verdade (WAV, não MP3 — por isso `synthesize_speech` também
//!   devolve o `content-type` real, pro frontend não montar uma data URI
//!   mentindo o formato).
//! - STT: `POST /transcribe` é multipart/form-data (campo `file`), não JSON
//!   com base64 como a OpenAI usa.

use anyhow::{anyhow, Result};
use serde::Deserialize;

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:17493";

#[derive(Deserialize)]
struct SpeakResponse {
    id: String,
    status: String,
    error: Option<String>,
}

#[derive(Deserialize)]
struct StatusEvent {
    status: String,
    error: Option<String>,
}

/// Sintetiza `text` em áudio via um Voicebox local já rodando. `profile` é o
/// nome ou id do perfil de voz configurado lá (vazio deixa o Voicebox cair
/// no binding padrão dele, se existir um). `language`, quando informado,
/// só tem efeito em engines multilíngues — perfis "preset" de voz única
/// (ex: Kokoro af_heart) não trocam de sotaque só por isso, então a troca
/// automática de idioma (`audio::detect_language`) é best-effort aqui, ao
/// contrário do Kokoro direto via OpenRouter (`audio::resolve_voice`), que
/// troca a voz de verdade.
/// Dispara a geração e espera terminar, sem baixar os bytes do áudio —
/// usado quando o Voicebox já toca o áudio sozinho (opção "Autoplay on
/// generate" dele, ligada por padrão no app). Pedido do usuário,
/// 2026-08-20: com autoplay ligado no Voicebox, o Cerne baixar E tocar o
/// mesmo áudio de novo soava como fala duplicada/eco — melhor deixar só o
/// Voicebox tocar e o Cerne só "puxar o gatilho" e mostrar o estado (carinha
/// carregando → ocioso), sem áudio nenhum no lado do Cerne.
pub async fn speak(base_url: &str, text: &str, profile: &str, language: Option<&str>) -> Result<()> {
    if text.trim().is_empty() {
        return Err(anyhow!("texto vazio"));
    }
    let client = reqwest::Client::new();
    let base = base_url.trim_end_matches('/');
    let speak = trigger_generation(&client, base, text, profile, language).await?;
    if speak.status != "completed" {
        wait_for_generation(&client, base, &speak.id).await?;
    }
    Ok(())
}

async fn trigger_generation(
    client: &reqwest::Client,
    base: &str,
    text: &str,
    profile: &str,
    language: Option<&str>,
) -> Result<SpeakResponse> {
    let mut body = serde_json::json!({ "text": text });
    if !profile.trim().is_empty() {
        body["profile"] = serde_json::json!(profile);
    }
    if let Some(lang) = language {
        body["language"] = serde_json::json!(lang);
    }
    let resp = client
        .post(format!("{base}/speak"))
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| anyhow!("nao foi possivel contatar o Voicebox local: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Voicebox recusou o pedido de fala ({status}): {err_text}"));
    }
    let speak: SpeakResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta inesperada do Voicebox: {e}"))?;
    if let Some(err) = speak.error {
        return Err(anyhow!("Voicebox falhou ao gerar o audio: {err}"));
    }
    Ok(speak)
}

/// `GET /generate/{id}/status` é um stream SSE que fica aberto até a geração
/// terminar — lê o corpo inteiro (a conexão já bloqueia até acabar) e usa só
/// o último evento (`data: {...}`) pra saber o resultado final. 120s de
/// limite: geração local costuma ser rápida (segundos), mas o primeiro uso
/// depois de um tempo parado pode envolver carregar o modelo pra
/// memória/VRAM de novo.
async fn wait_for_generation(client: &reqwest::Client, base: &str, id: &str) -> Result<()> {
    let resp = client
        .get(format!("{base}/generate/{id}/status"))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| anyhow!("nao foi possivel acompanhar a geracao no Voicebox: {e}"))?;
    let text = resp
        .text()
        .await
        .map_err(|e| anyhow!("falha lendo o progresso da geracao: {e}"))?;
    let last_event = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|json| serde_json::from_str::<StatusEvent>(json.trim()).ok())
        .last()
        .ok_or_else(|| anyhow!("Voicebox nao reportou o progresso da geracao"))?;
    if let Some(err) = last_event.error {
        return Err(anyhow!("Voicebox falhou ao gerar o audio: {err}"));
    }
    if last_event.status != "completed" {
        return Err(anyhow!("geracao no Voicebox terminou com status inesperado: {}", last_event.status));
    }
    Ok(())
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Transcreve `audio_bytes` via `POST /transcribe` (multipart, campos
/// `file`/`language`/`model`) — formato diferente do STT OpenAI-compatible
/// (`audio.rs`, JSON com base64). `extension` vira o nome do arquivo
/// enviado (ex: "clip.webm") só pro Voicebox reconhecer o formato pelo
/// nome. `language` (código ISO 639-1, ex: "pt") é OPCIONAL no schema do
/// Voicebox — sem ele o Whisper tenta auto-detectar, o que funciona mas é
/// menos confiável (e mais lento) que dar a dica explícita quando o
/// usuário já sabe o idioma que vai falar. Pedido do usuário, 2026-08-20:
/// "consegue ver se pelo voicebox é possível fazer STT em português" — sim,
/// qualquer Whisper SEM sufixo ".en" (Base/Small/Medium/Large/Turbo, as
/// opções que o próprio Voicebox oferece pra download) é multilíngue e já
/// suporta português nativamente; só faltava o Cerne deixar configurar a
/// dica de idioma.
pub async fn transcribe_audio(
    base_url: &str,
    audio_bytes: Vec<u8>,
    extension: &str,
    language: Option<&str>,
) -> Result<String> {
    let client = reqwest::Client::new();
    let base = base_url.trim_end_matches('/');
    let filename = format!("clip.{extension}");
    let part = reqwest::multipart::Part::bytes(audio_bytes).file_name(filename);
    let mut form = reqwest::multipart::Form::new().part("file", part);
    if let Some(lang) = language.filter(|l| !l.trim().is_empty()) {
        form = form.text("language", lang.to_string());
    }
    let resp = client
        .post(format!("{base}/transcribe"))
        .multipart(form)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| anyhow!("nao foi possivel contatar o Voicebox local: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Voicebox recusou a transcricao ({status}): {err_text}"));
    }
    let parsed: TranscriptionResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("resposta inesperada do Voicebox: {e}"))?;
    Ok(parsed.text)
}
