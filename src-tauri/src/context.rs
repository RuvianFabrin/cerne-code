//! Medição de contexto (quanto do prompt cabe na janela do modelo).
//!
//! Duas fontes, nessa ordem de prioridade:
//!
//! 1. **O número REAL que o provider devolveu** (`usage.prompt_tokens` da
//!    última requisição). É a verdade absoluta: já vem contado pelo tokenizador
//!    do próprio modelo, inclui system prompt + histórico + tool specs + MCP, e
//!    serve pra qualquer provider (local inclusive). Fica gravado em
//!    `Session.last_prompt_tokens` e é o que o medidor mostra.
//! 2. **Estimativa**, só quando ainda não há número real (sessão nova, antes da
//!    primeira resposta). Aí usamos o tokenizador de verdade (crate `tiktoken`,
//!    ver `Cargo.toml`) escolhido pelo nome do modelo — DeepSeek e Qwen têm
//!    vocabulário próprio, modelo sem encoding conhecido cai no `cl100k_base`.
//!
//! **O que mudou e por quê (2026-09-13).** Antes existia só a estimativa, e ela
//! somava apenas as mensagens — deixando de fora as tool specs (~6,4 mil tokens
//! por requisição numa sessão com pasta de projeto), que vão no request. A
//! heurística era `chars/4`, que erra justamente no conteúdo que mais trafega
//! num agente de código: medido contra tokenizador real, ela subestimava 17% em
//! português acentuado, 27% em JSON de resultado de ferramenta e 74% em CJK —
//! ou seja, o medidor mostrava ~metade do que era realmente enviado nas sessões
//! com muitas tool calls. Numa sessão de 8k de janela ele exibia 97% quando o
//! request real era 198%.

use crate::models::{ChatMessage, ContextUsage, ToolSpec};

/// Overhead de enquadramento por mensagem (role/name/tool_call_id). Valor do
/// cookbook da OpenAI para o formato de chat; não escala com o conteúdo.
const PER_MESSAGE_OVERHEAD: u32 = 4;

/// Overhead por chamada de ferramenta na mensagem (id/type/function).
const PER_TOOL_CALL_OVERHEAD: u32 = 6;

/// Fallback final, quando nem o tokenizador carrega (não deveria acontecer —
/// os vocabulários são embutidos no binário —, mas o medidor nunca deve
/// derrubar nem travar por causa disso).
fn fallback_chars_div_4(text: &str) -> u32 {
    ((text.chars().count() as f32) / 4.0).ceil() as u32
}

/// Qual vocabulário usar pro modelo em questão.
///
/// Não usa `tiktoken::model_to_encoding` de propósito: a tabela do crate cobre
/// 116 nomes exatos, e o Cerne roda com nomes que ela não conhece (apelidos
/// locais como `gemma4-e4b-qat-mtp`, ids da OpenRouter como
/// `deepseek/deepseek-v4-flash-0731`, modelos customizados do usuário). Casar
/// por substring é mais robusto e o vocabulário dos modelos de uma mesma
/// família é compatível.
///
/// Quem não tem encoding próprio (Claude, Grok, modelos customizados) cai no
/// `cl100k_base` — é uma aproximação, não a contagem do modelo deles.
fn encoding_name_for_model(model: &str) -> &'static str {
    let m = model.to_ascii_lowercase();

    if m.contains("deepseek") {
        "deepseek_v4"
    } else if m.contains("qwen") {
        "qwen2"
    } else if [
        "llama", "ornith", "gemma", "mistral", "devstral", "ministral", "lfm", "nemotron", "pixtral",
    ]
    .iter()
    .any(|k| m.contains(k))
    {
        "llama3"
    } else if ["gpt-4o", "gpt-4.1", "gpt-5", "o1", "o3", "o4", "gpt-oss"]
        .iter()
        .any(|k| m.contains(k))
    {
        "o200k_base"
    } else {
        // OpenAI antigo, Claude, Grok e qualquer nome desconhecido.
        "cl100k_base"
    }
}

/// Vocabulário do tiktoken pro modelo — `None` se o vocabulário não estiver
/// compilado neste build (aí a contagem cai no fallback).
///
/// `get_encoding` devolve uma referência ao tokenizador já construído e
/// cacheado pelo crate, então chamar isto repetidamente é barato (não
/// reconstrói a tabela BPE a cada mensagem).
fn encoder_for(model: &str) -> Option<&'static tiktoken::CoreBpe> {
    tiktoken::get_encoding(encoding_name_for_model(model))
        // Modelo cujo vocabulário não foi compilado neste build (a feature
        // correspondente está desligada no Cargo.toml): cai no genérico em vez
        // de perder a contagem.
        .or_else(|| tiktoken::get_encoding("cl100k_base"))
}

/// Piso e teto do custo de uma imagem, em tokens.
///
/// O piso �� o que a OpenAI cobra por imagem em "low detail"; o teto evita que
/// uma imagem gigante (foto de 8000px, por exemplo) sozinha estoure a conta e
/// faça o medidor achar que a janela acabou.
const IMAGE_TOKEN_MIN: u32 = 85;
const IMAGE_TOKEN_MAX: u32 = 4000;

/// Fórmula de mercado pra custo de imagem em tokens: área / 750. É a mesma
/// usada pela Anthropic pra contar imagens na janela, e bate com o que a
/// OpenAI documenta pra "high detail" (tile 512×512 = 170 tokens;
/// 512×512/750 ≈ 350, mesma ordem). Não é exata pra todo provider — cada um
/// tem seu tiling —, mas erra muito menos que um número fixo.
const IMAGE_AREA_DIVISOR: u64 = 750;

/// Custo de uma imagem a partir do seu data URI.
///
/// Lê **só as dimensões** do cabeçalho (não decodifica a imagem), então é
/// barato. Se não conseguir (formato exótico, data URI malformado), cai num
/// chute conservador — melhor um número aproximado que ignorar a imagem.
///
/// Por que não usar um valor fixo: medido numa sessão real do usuário
/// (2026-09-13), um print de ~50 KB custava ~2.582 tokens de verdade, enquanto
/// o palpite fixo de 800 que existia antes subestimava em 3×.
fn estimate_image_tokens(data_uri: &str) -> u32 {
    use base64::Engine as _;

    // "data:image/png;base64,AAAA..." -> "AAAA..."
    let b64 = data_uri
        .split_once(',')
        .map(|(_, payload)| payload)
        .unwrap_or(data_uri);

    let bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
        Ok(b) => b,
        Err(_) => return IMAGE_TOKEN_MIN.max(800),
    };

    let dimensoes = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok());

    match dimensoes {
        Some((w, h)) => ((w as u64 * h as u64 / IMAGE_AREA_DIVISOR) as u32)
            .clamp(IMAGE_TOKEN_MIN, IMAGE_TOKEN_MAX),
        None => IMAGE_TOKEN_MIN.max(800),
    }
}

fn count(enc: Option<&tiktoken::CoreBpe>, text: &str) -> u32 {
    match enc {
        Some(e) => e.count(text) as u32,
        None => fallback_chars_div_4(text),
    }
}

/// Conta os tokens de um texto solto com o tokenizador do modelo.
///
/// Só existe pros testes: em produção a contagem sempre parte de uma lista de
/// mensagens (`estimate_messages_tokens`), porque é isso que vai no request.
#[cfg(test)]
pub fn estimate_tokens_for(model: &str, text: &str) -> u32 {
    count(encoder_for(model), text)
}

/// Estima o custo total de um request: mensagens + tool specs.
///
/// As **tool specs entram na conta** porque vão em toda requisição (ver
/// `providers::chat_stream`, `body["tools"] = ...`) e nunca eram contadas —
/// são ~6,4 mil tokens numa sessão com pasta de projeto, ~8 mil com modelo de
/// visão. Deixá-las de fora fazia o medidor subnotificar mesmo com a heurística
/// certa.
///
/// As tool specs são contadas pelo **JSON serializado**, que é exatamente o que
/// vai no wire (inclui os nomes de campo, não só o texto das descrições).
pub fn estimate_messages_tokens(
    messages: &[ChatMessage],
    tool_specs: &[ToolSpec],
    model: &str,
) -> u32 {
    let enc = encoder_for(model);

    let mut tokens: u32 = messages
        .iter()
        .map(|m| {
            let mut t = count(enc, &m.content) + PER_MESSAGE_OVERHEAD;
            // Custo por imagem calculado das dimensões reais (ver
            // `estimate_image_tokens`) — um valor fixo errava por 3×.
            t += m
                .images
                .iter()
                .map(|img| estimate_image_tokens(img))
                .sum::<u32>();
            if let Some(calls) = &m.tool_calls {
                for call in calls {
                    t += count(enc, &call.function.name);
                    t += count(enc, &call.function.arguments);
                    t += PER_TOOL_CALL_OVERHEAD;
                }
            }
            t
        })
        .sum();

    for spec in tool_specs {
        // O `to_string` do serde já é a forma exata enviada (o mesmo
        // `serde_json::to_value(tools)` do provider). Se falhar, cai pro texto
        // das partes — nunca deixa de contar.
        match serde_json::to_string(spec) {
            Ok(json) => tokens += count(enc, &json),
            Err(_) => {
                tokens += count(enc, &spec.function.name);
                tokens += count(enc, &spec.function.description);
                tokens += count(enc, &spec.function.parameters.to_string());
            }
        }
    }

    tokens
}

/// Tudo que `usage_for` precisa. Struct em vez de 10 parâmetros soltos porque
/// a lista já passou do ponto em que a ordem vira fonte de bug silencioso
/// (trocar `total_prompt_tokens` por `total_completion_tokens` compila).
pub struct UsageInputs<'a> {
    pub session_id: &'a str,
    pub messages: &'a [ChatMessage],
    pub tool_specs: &'a [ToolSpec],
    /// Modelo desta sessão — define o tokenizador da estimativa.
    pub model: &'a str,
    /// Tamanho da janela do modelo.
    pub context_length: u32,
    /// `true` quando `context_length` NÃO veio do provider nem da tabela e é
    /// só o chute conservador (`DEFAULT_CONTEXT_LENGTH`).
    pub is_estimated_length: bool,
    /// `prompt_tokens` reais da última requisição (fonte da verdade). `None` ou
    /// `Some(0)` = ainda não houve resposta, usa estimativa.
    pub real_used_tokens: Option<u32>,
    pub total_prompt_tokens: u32,
    pub total_completion_tokens: u32,
    pub total_requests: u32,
}

pub fn usage_for(input: UsageInputs<'_>) -> ContextUsage {
    // Número real sempre vence: já é a contagem do tokenizador do próprio
    // modelo, do conteúdo exato que foi enviado.
    let real = input.real_used_tokens.filter(|t| *t > 0);
    let (used_tokens, is_estimated_usage) = match real {
        Some(real) => (real, false),
        None => (
            estimate_messages_tokens(input.messages, input.tool_specs, input.model),
            true,
        ),
    };

    ContextUsage {
        session_id: input.session_id.to_string(),
        used_tokens,
        context_length: input.context_length,
        is_estimated_length: input.is_estimated_length,
        is_estimated_usage,
        percent: (used_tokens as f32 / input.context_length.max(1) as f32) * 100.0,
        total_prompt_tokens: input.total_prompt_tokens,
        total_completion_tokens: input.total_completion_tokens,
        total_requests: input.total_requests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ToolCall, ToolCallFunction, ToolFunctionSpec};

    fn msg(role: &str, content: &str) -> ChatMessage {
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

    fn spec(name: &str, desc: &str) -> ToolSpec {
        ToolSpec {
            kind: "function".to_string(),
            function: ToolFunctionSpec {
                name: name.to_string(),
                description: desc.to_string(),
                parameters: serde_json::json!({ "type": "object" }),
            },
        }
    }

    #[test]
    fn empty_input_costs_nothing() {
        assert_eq!(estimate_messages_tokens(&[], &[], "deepseek-v4-flash"), 0);
    }

    #[test]
    fn tool_specs_are_counted() {
        // Regressão do bug: as tool specs vão em toda requisição e não eram
        // contadas. Sem elas, o medidor subnotifica em ~6 mil tokens.
        let sem = estimate_messages_tokens(&[msg("user", "oi")], &[], "deepseek-v4-flash");
        let com = estimate_messages_tokens(
            &[msg("user", "oi")],
            &[spec("read_file", "Le o conteudo de um arquivo do projeto.")],
            "deepseek-v4-flash",
        );
        assert!(
            com > sem,
            "incluir tool specs tem que aumentar a contagem ({com} vs {sem})"
        );
        assert!(
            com - sem > 10,
            "uma tool spec com descricao deveria custar mais que 10 tokens, custou {}",
            com - sem
        );
    }

    #[test]
    fn real_token_count_wins_over_estimate() {
        let messages = vec![msg("user", "uma mensagem qualquer")];
        let usage = usage_for(UsageInputs {
            session_id: "s",
            messages: &messages,
            tool_specs: &[],
            model: "deepseek-v4-flash",
            context_length: 1000,
            is_estimated_length: false,
            real_used_tokens: Some(777),
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_requests: 1,
        });
        assert_eq!(usage.used_tokens, 777, "o numero real do provider deve vencer");
        assert!(!usage.is_estimated_usage);
    }

    #[test]
    fn falls_back_to_estimate_without_real_data() {
        let messages = vec![msg("user", "uma mensagem qualquer")];
        let usage = usage_for(UsageInputs {
            session_id: "s",
            messages: &messages,
            tool_specs: &[],
            model: "deepseek-v4-flash",
            context_length: 1000,
            is_estimated_length: false,
            real_used_tokens: None,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_requests: 0,
        });
        assert!(usage.used_tokens > 0);
        assert!(usage.is_estimated_usage, "sem dado real, tem que marcar estimado");
    }

    #[test]
    fn zero_real_tokens_is_treated_as_missing() {
        // Sessão que ainda não recebeu usage nenhum (provider não manda).
        let messages = vec![msg("user", "oi")];
        let usage = usage_for(UsageInputs {
            session_id: "s",
            messages: &messages,
            tool_specs: &[],
            model: "deepseek-v4-flash",
            context_length: 1000,
            is_estimated_length: false,
            real_used_tokens: Some(0),
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_requests: 0,
        });
        assert!(usage.is_estimated_usage, "0 nao e um numero real valido");
    }

    #[test]
    fn percent_never_divides_by_zero() {
        let usage = usage_for(UsageInputs {
            session_id: "s",
            messages: &[msg("user", "oi")],
            tool_specs: &[],
            model: "m",
            context_length: 0,
            is_estimated_length: true,
            real_used_tokens: Some(10),
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_requests: 1,
        });
        assert!(usage.percent.is_finite(), "janela 0 nao pode gerar NaN/inf");
    }

    #[test]
    fn tokenizer_is_much_closer_than_chars_div_4_on_code_and_json() {
        // O motivo de trocar a heuristica: conteudo denso (codigo, JSON) gasta
        // bem mais tokens por caractere que "chars/4" assume.
        let codigo = "pub fn repair(messages: &mut Vec<ChatMessage>) -> usize {\n\
                      \x20   let mut fixed = 0usize;\n\
                      \x20   for msg in messages.drain(..) { fixed += 1; }\n\
                      \x20   fixed\n}\n";
        let texto = codigo.repeat(10);

        let heuristico = fallback_chars_div_4(&texto);
        let real = estimate_tokens_for("deepseek-v4-flash", &texto);

        assert!(
            real > heuristico,
            "codigo deveria gastar MAIS tokens que chars/4 diz ({real} vs {heuristico})"
        );
    }

    #[test]
    fn model_names_map_to_the_expected_vocabulary() {
        assert_eq!(encoding_name_for_model("deepseek-v4-flash"), "deepseek_v4");
        assert_eq!(encoding_name_for_model("deepseek/deepseek-v4-pro"), "deepseek_v4");
        assert_eq!(encoding_name_for_model("qwen3.5-9b-mtp"), "qwen2");
        assert_eq!(encoding_name_for_model("gemma4-e4b-qat-mtp"), "llama3");
        assert_eq!(encoding_name_for_model("gpt-4o-mini"), "o200k_base");
        // Claude/Grok/desconhecido: aproximacao pelo cl100k.
        assert_eq!(encoding_name_for_model("claude-sonnet-4"), "cl100k_base");
        assert_eq!(encoding_name_for_model("grok-4.3"), "cl100k_base");
    }

    #[test]
    fn counting_is_stable_across_known_models() {
        // Sanidade: modelos diferentes podem contar diferente (vocabulário
        // próprio), mas todos têm que devolver algo plausível pro mesmo texto.
        let texto = "O usuário precisa verificar se a configuração está correta.";
        for modelo in ["deepseek-v4-flash", "qwen3.5", "gemma4", "gpt-4o", "claude-sonnet-4"] {
            let n = estimate_tokens_for(modelo, texto);
            assert!(
                (5..80).contains(&n),
                "{modelo}: contagem implausível ({n}) pra um texto de {} chars",
                texto.chars().count()
            );
        }
    }

    #[test]
    fn tool_call_arguments_are_counted() {
        let mut m = msg("assistant", "");
        m.tool_calls = Some(vec![ToolCall {
            id: "c1".to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\":\"C:/cerne/src-tauri/src/history.rs\"}".to_string(),
            },
        }]);
        let com = estimate_messages_tokens(&[m.clone()], &[], "deepseek-v4-flash");
        m.tool_calls = None;
        let sem = estimate_messages_tokens(&[m], &[], "deepseek-v4-flash");
        assert!(com > sem, "os argumentos da chamada contam ({com} vs {sem})");
    }
}
