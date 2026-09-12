use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant" | "tool"
    #[serde(default)]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Imagens anexadas pelo usuário, como data URIs
    /// (`data:image/png;base64,...`). Só faz sentido em mensagens `user` — o
    /// provider precisa suportar vision de verdade (ver
    /// `providers::supports_vision`) pra isso funcionar, então a UI só deixa
    /// anexar depois de confirmar isso, não assume que qualquer modelo aceita.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    /// O que aparece no balão do usuário na UI, quando difere de `content` —
    /// existe pra mensagens com anexo de documento: `content` carrega o texto
    /// extraído inteiro (é o que precisa ir pro modelo), mas mostrar isso cru
    /// na tela faz um scroll enorme pra um anexo grande. Quando presente, a UI
    /// mostra `display_content` (só o texto digitado + nome do anexo);
    /// `content` continua sendo o que de fato é enviado ao provider. Nunca
    /// serializado pro provider (`providers::to_wire_messages` remove antes
    /// de montar a requisição).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "default_fn_type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

fn default_fn_type() -> String {
    "function".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolCallFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunctionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub context_length: Option<u32>,
    /// Nome legível do modelo — OpenRouter traz (`name`); provedores
    /// OpenAI-compat genéricos não, então a UI cai no `id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Descrição curta do modelo (OpenRouter). Usada como tooltip no modal
    /// de navegação de modelos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tamanho do arquivo do modelo em bytes — só o Ollama (`/api/tags`)
    /// informa; os demais provedores não expõem isso na listagem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Tamanho em parâmetros (ex: "7B", "70B") — Ollama
    /// (`details.parameter_size`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_size: Option<String>,
    /// Preço por token de entrada (USD) — OpenRouter (`pricing.prompt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_prompt: Option<f64>,
    /// Preço por token de saída (USD) — OpenRouter (`pricing.completion`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_completion: Option<f64>,
    /// Se o modelo aceita imagem — inferido das modalidades de entrada
    /// (OpenRouter `architecture.input_modalities` contém "image") ou, pra
    /// llama.cpp, de o preset ter `mmproj`/`clip` configurado (ver
    /// `llama_cpp::preset_supports_vision`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_vision: Option<bool>,
    /// Só populado pra llama.cpp quando `supports_vision` é `false` mas o
    /// nome/caminho do modelo bate com uma família conhecida por ter
    /// variante multimodal (Gemma 3/4, Qwen-VL, LLaVA, etc.) — sinaliza "a
    /// arquitetura base suporta visão, mas falta apontar o `mmproj` nesse
    /// preset" em vez de "esse modelo não vê imagem de jeito nenhum" (ver
    /// `llama_cpp::vision_family_hint`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision_hint: Option<String>,
    /// Se o modelo tem tool-calling confirmado — hoje só a OpenRouter expõe
    /// isso de forma verificável (`supported_parameters` contém "tools").
    /// Ollama/LM Studio/llama.cpp/Custom não têm um jeito confiável e barato
    /// de checar isso pra toda a lista (tool-calling depende do template do
    /// modelo + do backend, não é um metadado estático) — fica `None`
    /// (mostrado como "não verificado" na UI) em vez de arriscar errado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    /// Se o modelo aceita áudio — mesma fonte da OpenRouter
    /// (`architecture.input_modalities` contém "audio"). Nenhum outro
    /// provider suportado hoje expõe áudio via chat completions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_audio: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Openrouter,
    LlamaCpp,
    Ollama,
    LmStudio,
    /// Qualquer endpoint compatível com a API de chat completions da OpenAI
    /// que o usuário mesmo configurou (ver `providers::custom`) — em vez de
    /// hardcodar um provider por nome (Claude, Grok, ChatGPT, Qwen, Kimi...),
    /// um único kind genérico cobre qualquer um desses (e qualquer outro que
    /// apareça), já que todos falam esse mesmo formato de wire. Distribuição
    /// open source não pode assumir chave/endpoint de nenhum provider de
    /// terceiro.
    Custom,
}

impl ProviderKind {
    /// Esforço de raciocínio default pra uma sessão/chamada utilitária deste
    /// provider. Locais (llama.cpp/ollama/lmstudio) nascem DESLIGADOS porque
    /// "Auto" deixaria o modelo usar o default dele — e Qwen3/GLM pensam por
    /// default, ficando lentos à toa (vale pra sessão e pras chamadas
    /// utilitárias: verificador, sub-agente, compactação). OpenRouter e Custom
    /// ficam em `None` (Auto): em Custom não existe "off" universal e um
    /// backend OpenAI estrito rejeitaria o payload de desligar com 400 — então
    /// não forçamos nada e deixamos o default do modelo (sem regressão).
    pub fn default_reasoning_effort(self) -> Option<ReasoningEffort> {
        match self {
            ProviderKind::LlamaCpp | ProviderKind::Ollama | ProviderKind::LmStudio => {
                Some(ReasoningEffort::Off)
            }
            _ => None,
        }
    }

    /// Roda num processo/GPU da própria máquina do usuário (llama.cpp/
    /// Ollama/LM Studio) — mesmos três já tratados como "local" em
    /// `default_reasoning_effort` acima. Usado pela Fase A4 do roteiro de
    /// Agentes/Skills pra decidir fila sequencial (local, uma GPU só, uma
    /// coisa por vez) vs. execução paralela (API, sem essa limitação de
    /// hardware compartilhado — mas com custo/rate-limit em troca).
    pub fn is_local(self) -> bool {
        matches!(
            self,
            ProviderKind::LlamaCpp | ProviderKind::Ollama | ProviderKind::LmStudio
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default)]
    pub has_api_key: bool,
    #[serde(default)]
    pub llama_fork: Option<String>, // "turboquant" | "prismml"
    /// Só relevante pra `ProviderKind::Custom` — vem de
    /// `CustomProviderConfig.supports_vision` (ver `providers::custom`),
    /// confirmação manual do usuário já que não dá pra perguntar isso de
    /// forma genérica pra um endpoint OpenAI-compatible qualquer.
    #[serde(default)]
    pub supports_vision_override: bool,
    /// Só relevante pra `ProviderKind::Custom` — vem de
    /// `CustomProviderConfig.context_length` (ver `providers::custom`),
    /// override manual pra quando o `/models` da conexão não devolve um
    /// campo de contexto utilizável.
    #[serde(default)]
    pub context_length_override: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub active_provider: ProviderKind,
    pub active_model: Option<String>,
    pub openrouter_base_url: String,
    pub llama_cpp_base_url: String,
    pub ollama_base_url: String,
    pub lmstudio_base_url: String,
    pub active_llama_fork: String,
    /// Qual provider customizado o seletor "Custom" usa por padrão pra
    /// sessão nova — mesmo papel do `active_llama_fork` pra `LlamaCpp`, já
    /// que "Custom" sozinho não diz qual das conexões configuradas usar.
    #[serde(default)]
    pub active_custom_provider_id: Option<String>,
    /// Qual conexão usa pra "ler em voz alta" (TTS) — por padrão OpenRouter
    /// (já usa a mesma chave do resto do app), mas o usuário pode trocar pra
    /// qualquer provider já configurado (local ou com API key), já que o
    /// endpoint de áudio (`/audio/speech`) é OpenAI-compatible e pode existir
    /// em outras conexões custom também.
    #[serde(default = "default_tts_provider")]
    pub tts_provider: ProviderKind,
    #[serde(default)]
    pub tts_llama_fork: Option<String>,
    #[serde(default)]
    pub tts_custom_provider_id: Option<String>,
    #[serde(default = "default_tts_model")]
    pub tts_model: String,
    /// Vozes não são universais entre modelos/providers de TTS — deixa
    /// configurável (ver `audio::DEFAULT_TTS_VOICE`) em vez de fixo no
    /// código, já que trocar de modelo quase sempre exige trocar de voz
    /// junto.
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String,
    /// Quando ligado (default), detecta o idioma do texto e troca a voz
    /// automaticamente — só tem efeito com o modelo Kokoro (única conexão
    /// que o app sabe mapear idioma→voz, ver `audio::resolve_voice`).
    /// Pedido do usuário, 2026-08-19: ler em inglês quando o texto é em
    /// inglês, em português quando é português, etc.
    #[serde(default = "default_true")]
    pub tts_auto_language: bool,
    /// Mesma ideia que `tts_provider`, mas pro microfone (STT,
    /// `/audio/transcriptions`).
    #[serde(default = "default_stt_provider")]
    pub stt_provider: ProviderKind,
    #[serde(default)]
    pub stt_llama_fork: Option<String>,
    #[serde(default)]
    pub stt_custom_provider_id: Option<String>,
    #[serde(default = "default_stt_model")]
    pub stt_model: String,
    /// Qual "motor" fala/transcreve — o padrão continua sendo qualquer
    /// endpoint OpenAI-compatible (`tts_provider`/`stt_provider` acima,
    /// OpenRouter/custom/local). `Voicebox` troca pra falar com um app
    /// Voicebox (https://github.com/jamiepine/voicebox) já rodando local na
    /// máquina do usuário — TTS/STT 100% local, sem chave, sem rede — via
    /// `voicebox.rs` (wire format próprio, incompatível com o OpenAI). Cada
    /// um (TTS/STT) tem o próprio backend porque o usuário pode querer só
    /// um dos dois local (ex: ler local, mas transcrever via Whisper cloud).
    #[serde(default)]
    pub tts_backend: VoiceBackend,
    #[serde(default)]
    pub stt_backend: VoiceBackend,
    #[serde(default = "default_voicebox_base_url")]
    pub voicebox_base_url: String,
    /// Nome ou id do perfil de voz já criado no Voicebox (ver
    /// `voicebox::synthesize_speech`) — vazio deixa o Voicebox cair no
    /// binding/perfil padrão dele, se existir algum.
    #[serde(default)]
    pub voicebox_tts_profile: String,
    /// Dica de idioma (código ISO 639-1, ex: "pt") pro Whisper do Voicebox
    /// na transcrição — vazio deixa auto-detectar. Pedido do usuário,
    /// 2026-08-20: STT em português via Voicebox.
    #[serde(default)]
    pub voicebox_stt_language: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VoiceBackend {
    #[default]
    OpenaiCompatible,
    Voicebox,
}

fn default_tts_provider() -> ProviderKind {
    ProviderKind::Openrouter
}

fn default_stt_provider() -> ProviderKind {
    ProviderKind::Openrouter
}

fn default_voicebox_base_url() -> String {
    crate::voicebox::DEFAULT_BASE_URL.to_string()
}

fn default_tts_model() -> String {
    crate::audio::DEFAULT_TTS_MODEL.to_string()
}

fn default_tts_voice() -> String {
    crate::audio::DEFAULT_TTS_VOICE.to_string()
}

fn default_true() -> bool {
    true
}

fn default_stt_model() -> String {
    crate::audio::DEFAULT_STT_MODEL.to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_provider: ProviderKind::Ollama,
            active_model: None,
            openrouter_base_url: "https://openrouter.ai/api/v1".to_string(),
            llama_cpp_base_url: "http://127.0.0.1:8082/v1".to_string(),
            ollama_base_url: "http://127.0.0.1:11434/v1".to_string(),
            lmstudio_base_url: "http://127.0.0.1:1234/v1".to_string(),
            active_llama_fork: "turboquant".to_string(),
            active_custom_provider_id: None,
            tts_provider: default_tts_provider(),
            tts_llama_fork: None,
            tts_custom_provider_id: None,
            tts_model: default_tts_model(),
            tts_voice: default_tts_voice(),
            tts_auto_language: true,
            stt_provider: default_stt_provider(),
            stt_llama_fork: None,
            stt_custom_provider_id: None,
            stt_model: default_stt_model(),
            tts_backend: VoiceBackend::default(),
            stt_backend: VoiceBackend::default(),
            voicebox_base_url: default_voicebox_base_url(),
            voicebox_tts_profile: String::new(),
            voicebox_stt_language: String::new(),
        }
    }
}

/// Modo de acesso de uma pasta extra: só leitura ou leitura+escrita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderMode {
    Read,
    ReadWrite,
}

impl Default for FolderMode {
    fn default() -> Self {
        FolderMode::Read
    }
}

/// Uma pasta extra com seu modo de acesso.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderEntry {
    pub path: String,
    #[serde(default)]
    pub mode: FolderMode,
}

impl FolderEntry {
    /// Extrai só os caminhos como `Vec<String>` — compatível com funções que
    /// ainda esperam `&[String]` (leitura).
    pub fn paths(entries: &[FolderEntry]) -> Vec<String> {
        entries.iter().map(|e| e.path.clone()).collect()
    }

    /// Retorna os caminhos que permitem escrita.
    pub fn writable_paths(entries: &[FolderEntry]) -> Vec<String> {
        entries.iter().filter(|e| e.mode == FolderMode::ReadWrite).map(|e| e.path.clone()).collect()
    }
}

/// Deserializador compatível: aceita tanto o formato antigo (array de strings)
/// quanto o novo (array de objetos `{path, mode}`). Strings viram `FolderEntry`
/// com modo `Read` (padrão seguro).
fn deserialize_folder_entries<'de, D>(deserializer: D) -> Result<Vec<FolderEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Array(arr) => {
            let mut entries = Vec::with_capacity(arr.len());
            for item in arr {
                match item {
                    serde_json::Value::String(s) => {
                        entries.push(FolderEntry { path: s, mode: FolderMode::Read });
                    }
                    serde_json::Value::Object(map) => {
                        let path = map.get("path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mode = map.get("mode")
                            .and_then(|v| v.as_str())
                            .map(|s| match s {
                                "read_write" => FolderMode::ReadWrite,
                                _ => FolderMode::Read,
                            })
                            .unwrap_or(FolderMode::Read);
                        if !path.is_empty() {
                            entries.push(FolderEntry { path, mode });
                        }
                    }
                    _ => {}
                }
            }
            Ok(entries)
        }
        _ => Ok(Vec::new()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub provider: ProviderKind,
    pub model: String,
    pub project_root: Option<String>,
    /// Context window of `model`, in tokens. Best-effort (read from the
    /// provider when known); falls back to `DEFAULT_CONTEXT_LENGTH` when
    /// the provider doesn't expose it (e.g. plain Ollama /api/tags).
    #[serde(default)]
    pub context_length: Option<u32>,
    /// Which llama.cpp fork this session's model came from — only
    /// meaningful when `provider == LlamaCpp`. Needed to auto-start the
    /// right server binary; older sessions predate this field and fall
    /// back to the configured default fork.
    #[serde(default)]
    pub llama_fork: Option<String>,
    /// Qual provider customizado (ver `providers::custom::CustomProviderConfig`)
    /// esta sessão usa — só relevante quando `provider == Custom`, mesma ideia
    /// do `llama_fork` pra `LlamaCpp`.
    #[serde(default)]
    pub custom_provider_id: Option<String>,
    /// Pastas extras (fora de `project_root`) que as ferramentas podem acessar
    /// via caminho absoluto. Cada entrada tem um modo: `Read` (só leitura —
    /// `read_file`/`list_dir`/`grep`/`ast_grep`) ou `ReadWrite` (leitura +
    /// escrita — também permite `write_file`/`edit_file`/`ast_edit` nessa pasta).
    /// Serializa como array de objetos `{"path": "...", "mode": "read"|"read_write"}`.
    /// Compatível com o formato antigo (array de strings = tudo read-only).
    #[serde(default, deserialize_with = "deserialize_folder_entries")]
    pub extra_read_paths: Vec<FolderEntry>,
    /// "Manual" (todo tool call pausa o turno pedindo aprovação antes de
    /// rodar) ou "Auto" (roda livre — o usuário pode cancelar o turno inteiro
    /// a qualquer momento pela lista de tarefas na lateral). Default `Auto`
    /// pra não mudar o comportamento de sessões já existentes; trocável a
    /// qualquer momento pelo seletor ao lado do "+" no composer.
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    /// Esforço de raciocínio enviado ao modelo (modelos que não suportam
    /// ignoram silenciosamente). None = não enviar o campo (default do modelo).
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Servidores MCP habilitados nesta sessão (por nome). `None` = todos os
    /// servidores globais habilitados são usados (comportamento padrão).
    /// `Some(vec)` = só os servidores cujos nomes estão na lista são expostos
    /// ao agente. Permite desativar MCPs individuais por sessão via composer.
    #[serde(default)]
    pub enabled_mcp_servers: Option<Vec<String>>,
    /// Método Fable (github.com/Sahir619/fable-method) injetado no system
    /// prompt quando ligado. É um loop de trabalho (classificar → agir →
    /// verificar) que ajuda modelos pequenos/médios a não abandonar tarefas;
    /// por isso fica DESLIGADO por padrão e só entra quando o usuário liga no
    /// ícone do composer — em modelos grandes só infla o prompt à toa.
    #[serde(default)]
    pub fable_method: bool,
    /// Persona ativa (prompt pronto cadastrado pelo usuário, ver `personas.rs`)
    /// injetada no system prompt desta sessão. `None` = nenhuma persona
    /// selecionada (comportamento padrão, prompt base do Cerne apenas).
    #[serde(default)]
    pub persona_id: Option<String>,
    /// Tokens reais acumulados na sessão (entrada + saída + requisições).
    /// Atualizados após cada chamada ao modelo, persistidos no session.json.
    #[serde(default)]
    pub total_prompt_tokens: u32,
    #[serde(default)]
    pub total_completion_tokens: u32,
    #[serde(default)]
    pub total_requests: u32,
    /// Pasta (ver `folders.rs`) que agrupa esta sessão na barra lateral.
    /// `None` = solta na raiz (comportamento de toda sessão criada antes
    /// dessa feature existir, T29).
    #[serde(default)]
    pub folder_id: Option<String>,
    /// Id da sessão que criou esta (Fase G: sessões paralelas orquestradas,
    /// via `start_agent_session`) — `None` pra qualquer sessão criada
    /// normalmente pelo usuário. Marca "sessão orquestrada" pra UI mostrar
    /// de onde veio e pro toolset dela excluir `start_agent_session`/
    /// `check_agent_session`/`list_agent_sessions` (guarda de profundidade:
    /// nível único, mesmo espírito do guard que `task` já tem).
    #[serde(default)]
    pub parent_session_id: Option<String>,
}

/// Pasta pra organizar sessões na barra lateral (T29). Só 2 níveis: uma
/// pasta de raiz (`parent_id: None`) pode conter sessões e subpastas; uma
/// subpasta (`parent_id: Some(..)`) só pode conter sessões — ver validação
/// em `folders::create_folder`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Padrão: toda chamada de ferramenta para e pede aprovação antes de rodar.
    #[default]
    Manual,
    Auto,
    /// Escreve direto no arquivo real (sem sandbox), sem pedir permissao.
    /// Para usuarios que confiam no agente e querem velocidade maxima.
    Yolo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    /// Desliga o raciocínio de forma explícita (o oposto de `None`/Auto, que
    /// deixa o modelo usar o default dele — e Qwen3/GLM pensam por default,
    /// daí a lentidão). O campo enviado no wire depende do provider, porque
    /// cada um desliga de um jeito (ver `providers::chat_stream`).
    Off,
    /// Liga o raciocínio de forma explícita — só faz sentido pra providers
    /// locais (LlamaCpp/LmStudio/Ollama), que não têm graduação low/medium/
    /// high real (llama.cpp trata `reasoning_effort` como binário: liga ou
    /// desliga via `chat_template_kwargs.enable_thinking`). A UI só oferece
    /// essa opção quando o provider é local; pra providers de API (Openrouter/
    /// Custom) usa-se Low/Medium/High, que têm graduação de verdade.
    On,
    Low,
    Medium,
    High,
}

/// Used when a provider doesn't report the model's context window and we
/// have no better guess. Conservative on purpose (better to compact too
/// early than to silently overflow the model's real window).
pub const DEFAULT_CONTEXT_LENGTH: u32 = 8192;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUsage {
    pub session_id: String,
    pub used_tokens: u32,
    pub context_length: u32,
    pub is_estimated_length: bool,
    pub percent: f32,
    #[serde(default)]
    pub total_prompt_tokens: u32,
    #[serde(default)]
    pub total_completion_tokens: u32,
    #[serde(default)]
    pub total_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub label: String,
    pub status: String, // pending | running | done | failed
    pub detail: Option<String>,
    /// Quantas mensagens do usuário já tinham sido enviadas nesta sessão
    /// quando esta tarefa foi criada — usado pra intercalar os passos na
    /// timeline do chat, agrupados sob a mensagem do usuário que os
    /// disparou, em vez de só aparecerem no painel lateral.
    #[serde(default)]
    pub turn: u32,
    /// Caminho do arquivo envolvido nesta operacao (extraido dos args da
    /// tool call) — a UI mostra como chip inline com icone de tipo de arquivo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Linhas adicionadas no diff (contagem de linhas `+` no unified diff).
    #[serde(default)]
    pub additions: u32,
    /// Linhas removidas no diff (contagem de linhas `-` no unified diff).
    #[serde(default)]
    pub deletions: u32,
    /// Timestamp (epoch ms) de quando a tarefa comecou a executar.
    #[serde(default)]
    pub started_at_ms: u64,
    /// Duracao em ms da execucao da tarefa (None enquanto running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Texto bruto do comando (so populado pra run_command/ferramentas tipo
    /// shell) — a UI usa isso pra mostrar um bloco "IN" separado do "OUT"
    /// (que fica em `detail`), como um terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Preenchido so pra chamadas de `task`/`verify_completion`/
    /// `run_pipeline` — o id da `AgentExecution` (ver abaixo) que essa
    /// chamada disparou. A UI usa isso pra saber que esse item pode ser
    /// expandido pra mostrar os passos internos da execucao (que ficam em
    /// `AgentExecution.steps`, nao aqui — esse `TaskItem` representa so a
    /// chamada de fora, nao os passos de dentro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
}

/// Uma execução isolada de agente/skill (`task`/`verify_completion` hoje —
/// ver `agent/subagent.rs`/`agent/verifier.rs`), rastreada por UUID pra
/// permitir reconstruir a árvore de chamadas (agente A chama skill B chama
/// agente C) sem perder a referência de quem chamou quem.
/// Vive num registro em memória (`AppState.agent_executions`), não
/// persistido: é só pra UI consultar "o que está rodando agora" enquanto o
/// app está aberto, não é histórico de longo prazo (isso já existe via
/// `TaskItem`/mensagens da sessão).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecution {
    pub id: String,
    /// `execution_id` de quem chamou esta execução, se houver — hoje sempre
    /// `None` na prática: a guarda de profundidade estrutural (sub-agente
    /// não tem a tool `task`/`verify_completion` no seu toolset) impede que
    /// uma execução chame outra, então só o loop principal da sessão (que
    /// não é ele mesmo uma "execução" rastreada) inicia `task`/
    /// `verify_completion`. O campo já existe pronto pra quando isso mudar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub session_id: String,
    /// "task" | "verify_completion" — qual ferramenta disparou esta execução.
    pub kind: String,
    /// Descrição curta pra UI (a `description` passada pro `task`, ou um
    /// rótulo fixo tipo "verificador" pro `verify_completion`).
    pub name: String,
    pub status: String, // "running" | "done" | "failed"
    pub started_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<u64>,
    /// Passos (chamadas de ferramenta) que essa execução deu por dentro —
    /// antes só existiam como evento efêmero de UI (`agent:tool_call`/
    /// `agent:tool_result`), perdidos assim que o turno terminava; agora
    /// ficam registrados aqui pra sobreviver ao fim do turno enquanto o app
    /// continua aberto (achado testando ao vivo, 2026-08-16: usuário queria
    /// continuar vendo o que rodou dentro de um `task`/pipeline depois de
    /// terminado, não só enquanto rodava).
    #[serde(default)]
    pub steps: Vec<TaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEdit {
    pub id: String,
    pub session_id: String,
    pub target_path: String,
    pub sandbox_path: String,
    pub diff: String,
    pub is_new_file: bool,
    /// True quando a edicao ja foi aplicada direto no arquivo real (modo
    /// YOLO) — a UI mostra o diff mas sem botoes Aceitar/Rejeitar.
    #[serde(default)]
    pub already_applied: bool,
}
