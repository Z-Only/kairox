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
#[derive(Debug)]
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
            match block_type {
                "tool_use" => {
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
                "server_tool_use" => {
                    // Server tool invocation — emit as text delta for display
                    let name = value["content_block"]["name"]
                        .as_str()
                        .unwrap_or("server_tool");
                    events.push(AnthropicRawEvent::Event(ModelEvent::TokenDelta(format!(
                        "\n[server tool: {name}]\n"
                    ))));
                }
                "web_search_tool_result" => {
                    // Web search results from server
                    let text = format_web_search_result(&value["content_block"]);
                    if !text.is_empty() {
                        events.push(AnthropicRawEvent::Event(ModelEvent::TokenDelta(text)));
                    }
                }
                "code_execution_tool_result" => {
                    // Code execution results from server
                    let text = format_code_execution_result(&value["content_block"]);
                    if !text.is_empty() {
                        events.push(AnthropicRawEvent::Event(ModelEvent::TokenDelta(text)));
                    }
                }
                "bash_code_execution_tool_result" | "text_editor_code_execution_tool_result" => {
                    let text = format_current_code_execution_result(&value["content_block"]);
                    if !text.is_empty() {
                        events.push(AnthropicRawEvent::Event(ModelEvent::TokenDelta(text)));
                    }
                }
                _ => {}
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
                            cache_creation_input_tokens: usage_value["cache_creation_input_tokens"]
                                .as_u64(),
                            cache_read_input_tokens: usage_value["cache_read_input_tokens"]
                                .as_u64(),
                        })
                    } else {
                        None
                    };
                    events.push(AnthropicRawEvent::Event(ModelEvent::Completed { usage }));
                }
            }
        }
        "message_start" => {
            // `message_start` is not terminal. Some Anthropic-compatible
            // proxies include usage here before any content deltas; emitting a
            // `Completed` event would make downstream consumers stop before
            // reading the actual text/tool events.
        }
        "ping" => {
            // No model events to emit for pings
        }
        "error" => {
            let msg = value["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Anthropic API error");
            events.push(AnthropicRawEvent::Event(ModelEvent::Failed {
                message: msg.to_string(),
            }));
        }
        event_type if event_type.ends_with("_ERROR") || event_type.contains("ERROR") => {
            events.push(AnthropicRawEvent::Event(ModelEvent::Failed {
                message: proxy_error_message(&value, event_type),
            }));
        }
        _ => {
            // Unknown event type — skip
        }
    }

    Ok(events)
}

fn proxy_error_message(value: &serde_json::Value, event_type: &str) -> String {
    if let Some(message) = value["message"].as_str() {
        if let Ok(nested) = serde_json::from_str::<serde_json::Value>(message) {
            if let Some(nested_message) = nested["error"]["message"]
                .as_str()
                .or_else(|| nested["message"].as_str())
            {
                return nested_message.to_string();
            }
        }
        return message.to_string();
    }

    value["error"]["message"]
        .as_str()
        .or_else(|| value["error"]["type"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Anthropic proxy error: {event_type}"))
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
                "server_tool_use" => {
                    let name = block["name"].as_str().unwrap_or("server_tool");
                    events.push(ModelEvent::TokenDelta(format!("\n[server tool: {name}]\n")));
                }
                "web_search_tool_result" => {
                    let text = format_web_search_result(block);
                    if !text.is_empty() {
                        events.push(ModelEvent::TokenDelta(text));
                    }
                }
                "code_execution_tool_result" => {
                    let text = format_code_execution_result(block);
                    if !text.is_empty() {
                        events.push(ModelEvent::TokenDelta(text));
                    }
                }
                "bash_code_execution_tool_result" | "text_editor_code_execution_tool_result" => {
                    let text = format_current_code_execution_result(block);
                    if !text.is_empty() {
                        events.push(ModelEvent::TokenDelta(text));
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
                cache_creation_input_tokens: usage_value["cache_creation_input_tokens"].as_u64(),
                cache_read_input_tokens: usage_value["cache_read_input_tokens"].as_u64(),
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

/// Format a `web_search_tool_result` content block as human-readable text.
fn format_web_search_result(block: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    if let Some(content) = block["content"].as_array() {
        for item in content {
            let item_type = item["type"].as_str().unwrap_or("");
            if item_type == "web_search_result" {
                let title = item["title"].as_str().unwrap_or("");
                let url = item["url"].as_str().unwrap_or("");
                let snippet = item["encrypted_content"]
                    .as_str()
                    .or_else(|| item["page_content"].as_str())
                    .unwrap_or("");
                if !title.is_empty() || !url.is_empty() {
                    parts.push(format!("[web result: {title} ({url})] {snippet}"));
                }
            }
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("\n[web search results]\n{}\n", parts.join("\n"))
}

/// Format a `code_execution_tool_result` content block as human-readable text.
fn format_code_execution_result(block: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    if let Some(content) = block["content"].as_array() {
        for item in content {
            let item_type = item["type"].as_str().unwrap_or("");
            match item_type {
                "code_execution_output" => {
                    if let Some(stdout) = item["stdout"].as_str() {
                        if !stdout.is_empty() {
                            parts.push(format!("[stdout] {stdout}"));
                        }
                    }
                    if let Some(stderr) = item["stderr"].as_str() {
                        if !stderr.is_empty() {
                            parts.push(format!("[stderr] {stderr}"));
                        }
                    }
                }
                "code_execution_result" => {
                    if let Some(return_value) = item["return_value"].as_str() {
                        if !return_value.is_empty() {
                            parts.push(format!("[return] {return_value}"));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // Also check top-level stdout/stderr/return_value (flat format)
    if let Some(stdout) = block["stdout"].as_str() {
        if !stdout.is_empty() && parts.is_empty() {
            parts.push(format!("[stdout] {stdout}"));
        }
    }
    if let Some(stderr) = block["stderr"].as_str() {
        if !stderr.is_empty() && parts.is_empty() {
            parts.push(format!("[stderr] {stderr}"));
        }
    }
    if let Some(rv) = block["return_value"].as_str() {
        if !rv.is_empty() && parts.is_empty() {
            parts.push(format!("[return] {rv}"));
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("\n[code execution result]\n{}\n", parts.join("\n"))
}

/// Format current `code_execution_20250825` result blocks as human-readable text.
fn format_current_code_execution_result(block: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    let Some(content) = block.get("content") else {
        return String::new();
    };

    collect_current_code_execution_parts(content, &mut parts);

    if parts.is_empty() {
        return String::new();
    }
    format!("\n[code execution result]\n{}\n", parts.join("\n"))
}

fn collect_current_code_execution_parts(value: &serde_json::Value, parts: &mut Vec<String>) {
    if let Some(array) = value.as_array() {
        for item in array {
            collect_current_code_execution_parts(item, parts);
        }
        return;
    }

    let Some(object) = value.as_object() else {
        return;
    };

    if let Some(stdout) = object.get("stdout").and_then(|value| value.as_str()) {
        if !stdout.is_empty() {
            parts.push(format!("[stdout] {stdout}"));
        }
    }
    if let Some(stderr) = object.get("stderr").and_then(|value| value.as_str()) {
        if !stderr.is_empty() {
            parts.push(format!("[stderr] {stderr}"));
        }
    }
    if let Some(return_code) = object.get("return_code").and_then(|value| value.as_i64()) {
        parts.push(format!("[return_code] {return_code}"));
    }
    if let Some(content) = object.get("content").and_then(|value| value.as_str()) {
        if !content.is_empty() {
            parts.push(format!("[content] {content}"));
        }
    }
    if let Some(lines) = object.get("lines").and_then(|value| value.as_array()) {
        let text = lines
            .iter()
            .filter_map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            parts.push(format!("[diff]\n{text}"));
        }
    }
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
