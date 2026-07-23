use crate::models::{ChatMessage, ContextUsage};

/// Rough token estimate (chars/4), the same heuristic most local tooling
/// uses when a real tokenizer isn't available — good enough to decide
/// "are we anywhere near the window", not meant to be exact.
pub fn estimate_tokens(text: &str) -> u32 {
    ((text.chars().count() as f32) / 4.0).ceil() as u32
}

/// Flat per-image token estimate — vision models tokenize an image into a
/// few hundred to a couple thousand tokens depending on resolution/tiling,
/// which varies per provider/model in a way `estimate_tokens`'s chars/4
/// heuristic can't approximate at all. This is a rough middle-of-the-road
/// guess (roughly what a single default-resolution tile costs on most
/// vision models) — good enough so the context gauge doesn't silently
/// ignore images, not meant to be exact.
const IMAGE_TOKEN_ESTIMATE: u32 = 800;

/// Sums content + tool-call arguments across the whole message list, plus a
/// small per-message overhead for role/name/tool_call_id framing.
pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> u32 {
    messages
        .iter()
        .map(|m| {
            let mut tokens = estimate_tokens(&m.content) + 4;
            tokens += m.images.len() as u32 * IMAGE_TOKEN_ESTIMATE;
            if let Some(calls) = &m.tool_calls {
                for call in calls {
                    tokens += estimate_tokens(&call.function.name);
                    tokens += estimate_tokens(&call.function.arguments);
                    tokens += 6;
                }
            }
            tokens
        })
        .sum()
}

pub fn usage_for(
    session_id: &str,
    messages: &[ChatMessage],
    context_length: u32,
    is_estimated_length: bool,
    total_prompt_tokens: u32,
    total_completion_tokens: u32,
    total_requests: u32,
) -> ContextUsage {
    let used_tokens = estimate_messages_tokens(messages);
    ContextUsage {
        session_id: session_id.to_string(),
        used_tokens,
        context_length,
        is_estimated_length,
        percent: (used_tokens as f32 / context_length as f32) * 100.0,
        total_prompt_tokens,
        total_completion_tokens,
        total_requests,
    }
}
