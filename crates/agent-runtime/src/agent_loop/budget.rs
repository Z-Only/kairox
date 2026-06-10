use agent_core::DomainEvent;
use agent_models::sanitize_markdown_data_uri_images;

use super::build_model_messages;

/// Decide whether the agent loop should fire an auto-compaction request
/// for this iteration. Pure function so it's trivial to unit-test the
/// boundary cases (threshold == 1.0 disables; busy gate skips; exact
/// equality counts as crossing the threshold per spec §4.4).
pub fn should_trigger_auto_compaction(
    usage: &agent_core::ContextUsage,
    threshold: f32,
    already_compacting: bool,
) -> bool {
    if already_compacting || threshold >= 1.0 {
        return false;
    }
    usage.ratio() >= threshold
}

/// Builds a `Vec<ModelMessage>` from `session_events` (preserving tool_call /
/// tool_result id pairing) and trims the FRONT until cumulative input tokens
/// fit `budget_tokens`. The system prompt + the most-recent user message are
/// always kept (they're appended last by `build_model_messages`).
///
/// Token accounting MUST match what providers actually bill — `ModelMessage`
/// has three serialised parts: `role`, `content`, and `tool_calls`
/// (a `Vec<ToolCall>` whose `arguments` is `serde_json::Value`). Tool calls
/// alone often weigh thousands of tokens for non-trivial payloads, so we
/// serialise the whole message to JSON and count that. This matches the
/// estimator used by `ContextAssembler` (cl100k_base on serialised text).
pub fn build_model_messages_within_budget(
    user_content: &str,
    session_events: &[DomainEvent],
    budget_tokens: u64,
) -> Vec<agent_models::ModelMessage> {
    let mut messages = build_model_messages(user_content, session_events);

    let bpe = match tiktoken_rs::cl100k_base() {
        Ok(bpe) => bpe,
        Err(_) => return messages, // tokenizer unavailable; emit as-is
    };
    let count_message = |m: &agent_models::ModelMessage| -> u64 {
        let (countable_content, image_tokens) = match sanitize_markdown_data_uri_images(&m.content)
        {
            Some(sanitized) => (
                sanitized.text,
                sanitized
                    .images
                    .iter()
                    .map(|image| image.estimated_tokens)
                    .sum(),
            ),
            None => (m.content.clone(), 0),
        };
        let countable_message = agent_models::ModelMessage {
            content: countable_content,
            ..m.clone()
        };
        // Use compact JSON to mirror what the OpenAI/Anthropic adapters
        // ultimately serialise. Failures fall back to content-only count.
        let text_tokens = match serde_json::to_string(&countable_message) {
            Ok(s) => bpe.encode_with_special_tokens(&s).len() as u64,
            Err(_) => bpe
                .encode_with_special_tokens(&countable_message.content)
                .len() as u64,
        };
        text_tokens + image_tokens
    };

    // Always keep the trailing user message (the active turn). Trim from the
    // FRONT, but NEVER drop a `tool` role message without also dropping the
    // matching assistant `tool_calls` message that precedes it — otherwise
    // OpenAI / Anthropic reject the request with "tool_call_id has no
    // matching assistant tool_calls".
    let mut total: u64 = messages.iter().map(&count_message).sum();
    while total > budget_tokens && messages.len() > 1 {
        let front = messages
            .first()
            .expect("messages guaranteed non-empty by loop guard");
        if front.role == "tool" {
            // No matching assistant left at the front — safe to drop alone.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            continue;
        }
        if front.role == "assistant" && !front.tool_calls.is_empty() {
            // Drop the assistant AND every tool message immediately following it
            // (the matching `tool_call_id` results) in one atomic step.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            while !messages.is_empty() && messages[0].role == "tool" {
                total = total.saturating_sub(count_message(&messages[0]));
                messages.remove(0);
            }
            continue;
        }
        // Plain user/assistant text — drop one.
        total = total.saturating_sub(count_message(front));
        messages.remove(0);
    }
    messages
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
