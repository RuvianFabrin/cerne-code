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
        }
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
    /// Pastas extras (fora de `project_root`) que as ferramentas de LEITURA
    /// (`read_file`/`list_dir`/`grep`/`ast_grep`) podem acessar via caminho
    /// absoluto — pra referenciar material relevante que nao mora dentro do
    /// projeto (outro repo, documentacao, etc.) sem abrir o disco inteiro.
    /// `write_file`/`edit_file`/`ast_edit` continuam restritos a
    /// `project_root` de proposito (e onde a sandbox vive).
    #[serde(default)]
    pub extra_read_paths: Vec<String>,
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
    /// Método Fable (github.com/Sahir619/fable-method) injetado no system
    /// prompt quando ligado. É um loop de trabalho (classificar → agir →
    /// verificar) que ajuda modelos pequenos/médios a não abandonar tarefas;
    /// por isso fica DESLIGADO por padrão e só entra quando o usuário liga no
    /// ícone do composer — em modelos grandes só infla o prompt à toa.
    #[serde(default)]
    pub fable_method: bool,
    /// Tokens reais acumulados na sessão (entrada + saída + requisições).
    /// Atualizados após cada chamada ao modelo, persistidos no session.json.
    #[serde(default)]
    pub total_prompt_tokens: u32,
    #[serde(default)]
    pub total_completion_tokens: u32,
    #[serde(default)]
    pub total_requests: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Manual,
    #[default]
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
