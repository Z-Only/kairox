use super::tool_accumulator::AnthropicToolCallAccumulator;
use crate::{ModelError, ModelEvent, Result};
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use reqwest::header::CONTENT_TYPE;
use std::collections::HashMap;

/// Internal events during Anthropic SSE stream processing.
/// Tool call arguments arrive across multiple `content_block_delta` chunks
/// and must be accumulated before emitting a `ModelEvent::ToolCallRequested`.
pub(super) enum AnthropicRawEvent {
    /// A regular model event (text delta, completion, error — no accumulation needed).
    Event(ModelEvent),
    /// A `content_block_start` with `type: "tool_use"` — begins a new tool call.
    ToolUseStarted { id: String, name: String },
    /// A `content_block_delta` with `type: "input_json_delta"` — partial JSON arguments.
    ToolUseArgumentDelta { partial_json: String },
    /// A `content_block_stop` — signals the current content block is complete.
    ToolUseFinished,
}

/// Parse a single SSE event from the Anthropic Messages API into a list of
/// raw events. Text deltas and completion events are emitted immediately;
/// tool_use blocks are split into start/delta/finished events that will be
/// accumulated by `AnthropicToolCallAccumulator`.
pub(super) fn parse_anthropic_raw_events(data: &str) -> Result<Vec<AnthropicRawEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let event_type = value["type"].as_str().unwrap_or("");
    let mut events = Vec::new();

    match event_type {
        "content_block_start" => {
            let block_type = value["content_block"]["type"].as_str().unwrap_or("");
            if block_type == "tool_use" {
                let id = value["content_block"]["id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let name = value["content_block"]["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if !id.is_empty() && !name.is_empty() {
                    events.push(AnthropicRawEvent::ToolUseStarted { id, name });
                }
            }
        }
        "content_block_delta" => {
            let delta_type = value["delta"]["type"].as_str().unwrap_or("");
            match delta_type {
                "text_delta" => {
                    if let Some(text) = value["delta"]["text"].as_str() {
                        if !text.is_empty() {
                            events.push(AnthropicRawEvent::Event(ModelEvent::TokenDelta(
                                text.to_string(),
                            )));
                        }
                    }
                }
                "input_json_delta" => {
                    if let Some(partial) = value["delta"]["partial_json"].as_str() {
                        events.push(AnthropicRawEvent::ToolUseArgumentDelta {
                            partial_json: partial.to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            // This fires for every content block (text or tool_use).
            // The accumulator will only act on it if there's a pending tool call.
            events.push(AnthropicRawEvent::ToolUseFinished);
        }
        "message_delta" => {
            if let Some(stop_reason) = value["delta"]["stop_reason"].as_str() {
                if stop_reason == "end_turn"
                    || stop_reason == "max_tokens"
                    || stop_reason == "stop_sequence"
                    || stop_reason == "tool_use"
                {
                    let usage_value = &value["usage"];
                    let usage = if usage_value.is_object() {
                        Some(crate::ModelUsage {
                            input_tokens: usage_value["input_tokens"].as_u64().unwrap_or(0),
                            output_tokens: usage_value["output_tokens"].as_u64().unwrap_or(0),
                        })
                    } else {
                        None
                    };
                    events.push(AnthropicRawEvent::Event(ModelEvent::Completed { usage }));
                }
            }
        }
        "message_start" | "ping" => {
            // No model events to emit for these
        }
        "error" => {
            let msg = value["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Anthropic API error");
            events.push(AnthropicRawEvent::Event(ModelEvent::Failed {
                message: msg.to_string(),
            }));
        }
        _ => {
            // Unknown event type — skip
        }
    }

    Ok(events)
}

/// Parse a non-streaming (JSON) response from the Anthropic Messages API.
/// The proxy may return a complete JSON object instead of SSE events when
/// `stream: true` is requested but the proxy does not support streaming.
pub(super) fn parse_anthropic_json_response(data: &str) -> Result<Vec<ModelEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let mut events = Vec::new();

    // Extract content from the response (both text and tool_use blocks)
    if let Some(content) = value["content"].as_array() {
        for block in content {
            match block["type"].as_str().unwrap_or("") {
                "text" => {
                    if let Some(text) = block["text"].as_str() {
                        if !text.is_empty() {
                            events.push(ModelEvent::TokenDelta(text.to_string()));
                        }
                    }
                }
                "tool_use" => {
                    let id = block["id"].as_str().unwrap_or("").to_string();
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    let arguments = block["input"].clone();
                    if !id.is_empty() && !name.is_empty() {
                        events.push(ModelEvent::ToolCallRequested {
                            tool_call_id: id,
                            tool_id: name,
                            arguments,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    // Check for completion
    let stop_reason = value["stop_reason"].as_str().unwrap_or("");
    if stop_reason == "end_turn"
        || stop_reason == "max_tokens"
        || stop_reason == "stop_sequence"
        || stop_reason == "tool_use"
    {
        let usage_value = &value["usage"];
        let usage = if usage_value.is_object() {
            Some(crate::ModelUsage {
                input_tokens: usage_value["input_tokens"].as_u64().unwrap_or(0),
                output_tokens: usage_value["output_tokens"].as_u64().unwrap_or(0),
            })
        } else {
            None
        };
        events.push(ModelEvent::Completed { usage });
    }

    // Check for error — handle both {"type":"error"} (standard) and {"error":{}} (proxy) formats
    if value["type"].as_str() == Some("error") || value["error"].is_object() {
        let msg = value["error"]["message"]
            .as_str()
            .or_else(|| value["error"]["type"].as_str())
            .unwrap_or("Unknown Anthropic API error");
        events.push(ModelEvent::Failed {
            message: msg.to_string(),
        });
    }

    Ok(events)
}

pub(super) fn stream_anthropic_response(
    response: reqwest::Response,
    name_map: HashMap<String, String>,
) -> BoxStream<'static, Result<ModelEvent>> {
    let is_json_response = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("application/json"));

    if is_json_response {
        return Box::pin(async_stream::stream! {
            match response.text().await {
                Ok(data) => match parse_anthropic_json_response(&data) {
                    Ok(events) => {
                        for event in events {
                            yield Ok(event);
                        }
                    }
                    Err(error) => yield Err(error),
                },
                Err(error) => yield Err(ModelError::StreamParse(error.to_string())),
            }
        });
    }

    let raw_stream = response
        .bytes_stream()
        .eventsource()
        .map_err(|e| ModelError::StreamParse(e.to_string()))
        .and_then(|event| async move {
            if event.data == "[DONE]" {
                Ok(None)
            } else {
                parse_anthropic_raw_events(&event.data).map(Some)
            }
        })
        .filter_map(
            |result: std::result::Result<Option<Vec<AnthropicRawEvent>>, ModelError>| async move {
                match result {
                    Ok(Some(raw_events)) => {
                        Some(futures::stream::iter(raw_events.into_iter().map(Ok)).boxed())
                    }
                    Ok(None) => Some(futures::stream::empty::<Result<AnthropicRawEvent>>().boxed()),
                    Err(e) => Some(futures::stream::once(async { Err(e) }).boxed()),
                }
            },
        )
        .flatten()
        .boxed();

    let mut accumulator = AnthropicToolCallAccumulator::new(name_map);
    Box::pin(async_stream::stream! {
        let mut raw_stream = raw_stream;

        while let Some(item) = raw_stream.next().await {
            let events: Vec<Result<ModelEvent>> = match item {
                Ok(raw) => accumulator.process(raw).into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e)],
            };
            for event in events {
                yield event;
            }
        }

        for event in accumulator.flush() {
            yield Ok(event);
        }
    })
}

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod tests;
