use crate::{ModelError, ModelEvent, Result};

/// Internal events during OpenAI SSE stream processing.
/// Tool call arguments arrive across multiple chunks and must be accumulated
/// before emitting a `ModelEvent::ToolCallRequested`.
pub(super) enum OpenAiChunkEvent {
    /// A regular model event (text delta, completion — no accumulation needed).
    Event(ModelEvent),
    /// A tool call start chunk (has non-empty id + name, may also have partial arguments).
    ToolCallStarted {
        index: usize,
        id: String,
        name: String,
    },
    /// A tool call argument delta chunk (continuation of arguments).
    ToolCallArgumentDelta {
        index: usize,
        partial_arguments: String,
    },
}

pub(super) fn parse_openai_chunk(data: &str) -> Result<Vec<OpenAiChunkEvent>> {
    let chunk: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let mut events = Vec::new();

    if let Some(choices) = chunk["choices"].as_array() {
        for choice in choices {
            let delta = &choice["delta"];

            // Content token delta
            if let Some(content) = delta["content"].as_str() {
                if !content.is_empty() {
                    events.push(OpenAiChunkEvent::Event(ModelEvent::TokenDelta(
                        content.to_string(),
                    )));
                }
            }

            // Tool calls — split into start and argument-delta events
            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                for tc in tool_calls {
                    let index = tc["index"].as_u64().unwrap_or(0) as usize;
                    let id = tc["id"].as_str().unwrap_or("").to_string();
                    let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                    let arguments_str = tc["function"]["arguments"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    if !id.is_empty() && !name.is_empty() {
                        // First chunk for this tool call — has id and name
                        events.push(OpenAiChunkEvent::ToolCallStarted { index, id, name });
                        // If arguments are also present in this chunk, emit a delta
                        if !arguments_str.is_empty() {
                            events.push(OpenAiChunkEvent::ToolCallArgumentDelta {
                                index,
                                partial_arguments: arguments_str,
                            });
                        }
                    } else if !arguments_str.is_empty() {
                        // Continuation chunk — only argument delta
                        events.push(OpenAiChunkEvent::ToolCallArgumentDelta {
                            index,
                            partial_arguments: arguments_str,
                        });
                    }
                }
            }

            // Finish reason
            if let Some(finish) = choice["finish_reason"].as_str() {
                if finish == "stop" || finish == "tool_calls" {
                    let usage_value = &chunk["usage"];
                    let usage = if usage_value.is_object() {
                        Some(crate::ModelUsage {
                            input_tokens: usage_value["prompt_tokens"].as_u64().unwrap_or(0),
                            output_tokens: usage_value["completion_tokens"].as_u64().unwrap_or(0),
                        })
                    } else {
                        None
                    };
                    events.push(OpenAiChunkEvent::Event(ModelEvent::Completed { usage }));
                }
            }
        }
    }

    Ok(events)
}
