use crate::models::ModelInfo;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::{Child, Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaForkConfig {
    pub id: String,
    pub label: String,
    pub server_exe: String,
    pub models_ini: String,
    pub port: u16,
}

fn providers_toml_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("providers.toml")
}

/// Loads the configured llama.cpp forks; empty list (not an error) on a
/// fresh install with no `providers.toml` yet — Cerne is distributed open
/// source, so it can't assume any particular machine's fork layout (the
/// previous default hardcoded a specific dev machine's `C:\ai-turboquant`
/// paths, which is exactly what a real user wouldn't have). The user adds
/// their own fork(s) via Settings (`add_fork`/`remove_fork`), which persists
/// them here for next launch.
pub fn load_forks(app_data_dir: &PathBuf) -> Result<Vec<LlamaForkConfig>> {
    let path = providers_toml_path(app_data_dir);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    #[derive(Deserialize, Default)]
    struct Wrapper {
        #[serde(default)]
        fork: Vec<LlamaForkConfig>,
    }
    let wrapper: Wrapper =
        toml::from_str(&text).map_err(|e| anyhow!("providers.toml invalido: {e}"))?;
    Ok(wrapper.fork)
}

pub fn save_forks(app_data_dir: &PathBuf, forks: &[LlamaForkConfig]) -> Result<()> {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        fork: &'a [LlamaForkConfig],
    }
    std::fs::create_dir_all(app_data_dir)?;
    let text = toml::to_string_pretty(&Wrapper { fork: forks })?;
    std::fs::write(providers_toml_path(app_data_dir), text)?;
    Ok(())
}

/// Adiciona (ou atualiza, se o `id` já existir) um fork configurado pelo
/// usuário na tela de Configurações.
pub fn add_fork(app_data_dir: &PathBuf, fork: LlamaForkConfig) -> Result<Vec<LlamaForkConfig>> {
    let mut forks = load_forks(app_data_dir)?;
    forks.retain(|f| f.id != fork.id);
    forks.push(fork);
    save_forks(app_data_dir, &forks)?;
    Ok(forks)
}

pub fn remove_fork(app_data_dir: &PathBuf, id: &str) -> Result<Vec<LlamaForkConfig>> {
    let mut forks = load_forks(app_data_dir)?;
    forks.retain(|f| f.id != id);
    save_forks(app_data_dir, &forks)?;
    Ok(forks)
}

/// Parses a llama-server router `.ini` (models.ini format) and returns the
/// presets available (every section except the `[*]` global-defaults one),
/// each carrying its `ctx-size` (falling back to the `[*]` default) so the
/// context-usage indicator has a real number to work with.
pub fn list_presets(models_ini_path: &str) -> Result<Vec<ModelInfo>> {
    let parser = load_ini(models_ini_path)?;
    let global_ctx = parser.getuint("*", "ctx-size").ok().flatten();
    let mut presets: Vec<ModelInfo> = parser
        .sections()
        .into_iter()
        .filter(|s| s != "*" && s.to_lowercase() != "default")
        .map(|s| {
            let ctx = parser
                .getuint(&s, "ctx-size")
                .ok()
                .flatten()
                .or(global_ctx)
                .map(|v| v as u32);
            let has_vision = preset_supports_vision(models_ini_path, &s);
            let vision_hint = if has_vision {
                None
            } else {
                vision_family_hint(&parser, &s)
            };
            ModelInfo {
                id: s.clone(),
                label: s,
                context_length: ctx,
                name: None,
                description: None,
                size_bytes: None,
                parameter_size: None,
                price_prompt: None,
                price_completion: None,
                supports_vision: Some(has_vision),
                vision_hint,
                supports_tools: None,
                supports_audio: None,
            }
        })
        .collect();
    presets.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(presets)
}

/// Context window for a single preset, used when a session is created
/// against the llama.cpp provider (cheaper than re-listing every preset).
pub fn preset_context_length(models_ini_path: &str, preset: &str) -> Option<u32> {
    let parser = load_ini(models_ini_path).ok()?;
    let global_ctx = parser.getuint("*", "ctx-size").ok().flatten();
    parser
        .getuint(preset, "ctx-size")
        .ok()
        .flatten()
        .or(global_ctx)
        .map(|v| v as u32)
}

/// Best-effort check of whether a preset actually has vision configured —
/// unlike Ollama/OpenRouter/LM Studio, llama.cpp has no HTTP endpoint to ask
/// this, so it's inferred from whether the preset's `.ini` section points at
/// a multimodal projector (`--mmproj`/`clip` file) at all. This is
/// deliberately conservative: a preset for a vision-capable base model
/// (gemma3, qwen-vl, etc.) with no `mmproj`/`clip` key configured comes back
/// `false`, matching the exact gap flagged before implementing this ("posso
/// colocar o gemma4 com llama.cpp, mas esquecer o .mmproj de visão").
pub fn preset_supports_vision(models_ini_path: &str, preset: &str) -> bool {
    let Ok(parser) = load_ini(models_ini_path) else {
        return false;
    };
    let Some(map) = parser
        .get_map_ref()
        .iter()
        .find(|(s, _)| s.eq_ignore_ascii_case(preset))
        .map(|(_, m)| m)
    else {
        return false;
    };
    map.iter().any(|(k, v)| {
        let k = k.to_lowercase();
        (k.contains("mmproj") || k.contains("clip"))
            && v.as_deref().map(|p| !p.trim().is_empty()).unwrap_or(false)
    })
}

/// Famílias de modelo conhecidas por terem uma variante multimodal (visão)
/// oficial, junto com o nome amigável usado no hint. Cobre os casos reais
/// vistos em `models.ini` (gemma3/gemma4) e as famílias mais comuns fora
/// dele — não é uma lista exaustiva, é best-effort: um preset de família
/// desconhecida simplesmente não recebe hint nenhum (fica `None`, igual a
/// hoje), em vez de arriscar um falso positivo.
const VISION_CAPABLE_FAMILIES: &[(&str, &str)] = &[
    ("gemma3", "Gemma 3"),
    ("gemma-3", "Gemma 3"),
    ("gemma4", "Gemma 4"),
    ("gemma-4", "Gemma 4"),
    ("qwen2-vl", "Qwen2-VL"),
    ("qwen2.5-vl", "Qwen2.5-VL"),
    ("qwen-vl", "Qwen-VL"),
    ("qwenvl", "Qwen-VL"),
    ("llava", "LLaVA"),
    ("bakllava", "LLaVA"),
    ("moondream", "Moondream"),
    ("pixtral", "Pixtral"),
    ("internvl", "InternVL"),
    ("minicpm-v", "MiniCPM-V"),
    ("phi-3-vision", "Phi-3 Vision"),
    ("phi3v", "Phi-3 Vision"),
    ("idefics", "Idefics"),
    ("cogvlm", "CogVLM"),
    ("yi-vl", "Yi-VL"),
    ("deepseek-vl", "DeepSeek-VL"),
];

/// Quando um preset NÃO tem `mmproj` configurado (`preset_supports_vision`
/// deu `false`), checa se o nome do preset ou o caminho do `model` bate com
/// uma família conhecida por ter variante multimodal — se bater, devolve o
/// nome da família (ex: "Gemma 4") pra UI montar a frase de aviso já
/// traduzida ("arquitetura suporta visão, mas falta apontar o mmproj nesse
/// preset"), em vez de simplesmente dizer "sem visão", que seria enganoso
/// pra quem só esqueceu de configurar o mmproj (o cenário exato que motivou
/// a pergunta: "as vezes o modelo tem visão mas sem mmproj, como fica nesses
/// casos?"). Devolve só o nome da família (não uma frase pronta) pra não
/// hardcodar português numa app com 4 idiomas — quem monta a frase final é o
/// frontend, via i18n.
fn vision_family_hint(parser: &configparser::ini::Ini, preset: &str) -> Option<String> {
    let model_path = parser.get(preset, "model").unwrap_or_default();
    let haystack = format!("{preset} {model_path}").to_lowercase();
    VISION_CAPABLE_FAMILIES
        .iter()
        .find(|(needle, _)| haystack.contains(needle))
        .map(|(_, label)| label.to_string())
}

fn load_ini(models_ini_path: &str) -> Result<configparser::ini::Ini> {
    let mut parser = configparser::ini::Ini::new();
    parser
        .load(models_ini_path)
        .map_err(|e| anyhow!("failed to parse {models_ini_path}: {e}"))?;
    Ok(parser)
}

const HEALTH_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const HEALTH_CHECK_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(400);

/// Spawns `llama-server` and doesn't return until it's actually answering
/// `/health` (or the timeout/an early process exit proves it never will).
/// A bare `spawn()` succeeding only means the OS created the process — on a
/// port conflict (another llama-server, or a leftover one this app doesn't
/// track) the process can spawn fine and still fail to bind, and callers
/// otherwise have no way to tell "started" from "started and immediately
/// failed".
pub async fn start_server(fork: &LlamaForkConfig) -> Result<Child> {
    if !PathBuf::from(&fork.server_exe).exists() {
        return Err(anyhow!("llama-server.exe not found at {}", fork.server_exe));
    }
    let mut child = Command::new(&fork.server_exe)
        .arg("--models-preset")
        .arg(&fork.models_ini)
        .arg("--models-max")
        .arg("1")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(fork.port.to_string())
        .kill_on_drop(true)
        .spawn()?;

    if let Err(e) = wait_for_health(&mut child, fork.port).await {
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
                "llama-server encerrou sozinho logo depois de iniciar (codigo {}) — confira se a porta {port} ja esta em uso ou os logs do processo",
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
                "llama-server nao respondeu em /health na porta {port} depois de {}s — pode estar travado, ou a porta ja esta ocupada por outro processo",
                HEALTH_CHECK_TIMEOUT.as_secs()
            ));
        }

        tokio::time::sleep(HEALTH_CHECK_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_ini(content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cerne-models-ini-test-{}.ini",
            uuid::Uuid::new_v4()
        ));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn preset_supports_vision_true_when_mmproj_key_is_set() {
        let path = write_ini(
            "[gemma3-vision]\nmodel = C:\\models\\gemma3.gguf\nmmproj = C:\\models\\mmproj-gemma3.gguf\nctx-size = 8192\n",
        );
        assert!(preset_supports_vision(
            path.to_str().unwrap(),
            "gemma3-vision"
        ));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn preset_supports_vision_false_when_base_model_has_no_mmproj_configured() {
        // Exatamente o cenario que motivou a checagem: um preset de modelo com
        // vision teorico (gemma4) mas sem mmproj configurado no .ini nao tem
        // vision de verdade.
        let path = write_ini("[gemma4]\nmodel = C:\\models\\gemma4.gguf\nctx-size = 8192\n");
        assert!(!preset_supports_vision(path.to_str().unwrap(), "gemma4"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn preset_supports_vision_false_for_unknown_preset_or_missing_file() {
        let path = write_ini("[qwen3]\nmodel = C:\\models\\qwen3.gguf\n");
        assert!(!preset_supports_vision(
            path.to_str().unwrap(),
            "nao-existe"
        ));
        assert!(!preset_supports_vision(
            "C:\\caminho\\que\\nao\\existe.ini",
            "qwen3"
        ));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn list_presets_marks_supports_vision_true_when_mmproj_is_set() {
        let path = write_ini(
            "[gemma4-e4b-qat-mtp]\nmodel = C:\\models\\gemma4.gguf\nmmproj = C:\\models\\mmproj.gguf\nctx-size = 8192\n",
        );
        let presets = list_presets(path.to_str().unwrap()).unwrap();
        let preset = presets
            .iter()
            .find(|p| p.id == "gemma4-e4b-qat-mtp")
            .unwrap();
        assert_eq!(preset.supports_vision, Some(true));
        assert_eq!(preset.vision_hint, None);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn list_presets_hints_vision_family_when_mmproj_is_missing() {
        // Cenario exato relatado: preset de familia com visao (gemma4) mas
        // sem mmproj configurado — supports_vision fica false, mas o hint
        // avisa que a familia tem variante multimodal, em vez de simplesmente
        // dizer "sem visao" (o que seria enganoso pra quem so esqueceu o
        // mmproj).
        let path = write_ini("[gemma4-26b-qat-mtp]\nmodel = C:\\models\\gemma4.gguf\nctx-size = 8192\n");
        let presets = list_presets(path.to_str().unwrap()).unwrap();
        let preset = presets
            .iter()
            .find(|p| p.id == "gemma4-26b-qat-mtp")
            .unwrap();
        assert_eq!(preset.supports_vision, Some(false));
        assert!(preset.vision_hint.as_deref().unwrap().contains("Gemma 4"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn list_presets_no_hint_for_non_vision_family() {
        // qwen3.5/qwen3.6 texto puro nao tem variante -VL conhecida na lista
        // (so qwen2-vl/qwen2.5-vl/qwen-vl tem) — nao deveria alucinar um
        // hint de visao pra ele.
        let path = write_ini("[qwen3.5-9b-mtp]\nmodel = C:\\models\\qwen3.5.gguf\nctx-size = 8192\n");
        let presets = list_presets(path.to_str().unwrap()).unwrap();
        let preset = presets.iter().find(|p| p.id == "qwen3.5-9b-mtp").unwrap();
        assert_eq!(preset.supports_vision, Some(false));
        assert_eq!(preset.vision_hint, None);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn vision_family_hint_matches_on_model_path_too() {
        // A familia pode estar so no caminho do arquivo do modelo, nao no
        // nome do preset (ex: usuario nomeou o preset so de "vl-test").
        let path = write_ini(
            "[vl-test]\nmodel = C:\\models\\qwen2-vl-7b-instruct.gguf\nctx-size = 4096\n",
        );
        let parser = load_ini(path.to_str().unwrap()).unwrap();
        let hint = vision_family_hint(&parser, "vl-test");
        assert!(hint.unwrap().contains("Qwen2-VL"));
        fs::remove_file(&path).ok();
    }

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cerne-forks-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_fork(id: &str) -> LlamaForkConfig {
        LlamaForkConfig {
            id: id.to_string(),
            label: format!("Fork {id}"),
            server_exe: format!("C:\\{id}\\llama-server.exe"),
            models_ini: format!("C:\\{id}\\models.ini"),
            port: 8082,
        }
    }

    #[test]
    fn load_forks_is_empty_on_a_fresh_install_with_no_providers_toml() {
        // Cerne e distribuido open source - nao pode assumir o layout de
        // pastas de uma maquina especifica como padrao pra quem nunca
        // configurou nada ainda.
        let dir = scratch_dir();
        let forks = load_forks(&dir).unwrap();
        assert!(forks.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_fork_then_load_forks_roundtrips() {
        let dir = scratch_dir();
        let added = add_fork(&dir, sample_fork("custom")).unwrap();
        assert_eq!(added.len(), 1);
        let loaded = load_forks(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "custom");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_fork_upserts_by_id_instead_of_duplicating() {
        let dir = scratch_dir();
        add_fork(&dir, sample_fork("custom")).unwrap();
        let mut updated = sample_fork("custom");
        updated.label = "Renomeado".to_string();
        let forks = add_fork(&dir, updated).unwrap();
        assert_eq!(forks.len(), 1, "deveria atualizar, nao duplicar");
        assert_eq!(forks[0].label, "Renomeado");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_fork_deletes_only_the_matching_id() {
        let dir = scratch_dir();
        add_fork(&dir, sample_fork("keep")).unwrap();
        add_fork(&dir, sample_fork("drop")).unwrap();
        let remaining = remove_fork(&dir, "drop").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "keep");
        fs::remove_dir_all(&dir).ok();
    }
}
