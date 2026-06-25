use super::tool_accumulator::OpenAiToolCallAccumulator;
use super::tool_names::OpenAiToolNameMap;
use crate::{ModelError, ModelEvent, Result};
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};

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

    if let Some(message) = openai_stream_error_message(&chunk) {
        return Ok(vec![OpenAiChunkEvent::Event(ModelEvent::Failed {
            message,
        })]);
    }

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
                            cache_creation_input_tokens: None,
                            cache_read_input_tokens: None,
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

fn openai_stream_error_message(chunk: &serde_json::Value) -> Option<String> {
    let error = chunk.get("error")?;
    if error.is_null() {
        return None;
    }

    let mut message = error
        .get("message")
        .and_then(|message| message.as_str())
        .filter(|message| !message.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            error
                .as_str()
                .filter(|message| !message.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| error.to_string());

    let error_type = error
        .get("type")
        .and_then(|error_type| error_type.as_str())
        .or_else(|| chunk.get("type").and_then(|error_type| error_type.as_str()))
        .filter(|error_type| !error_type.is_empty());
    let error_code = error
        .get("code")
        .and_then(|error_code| error_code.as_str())
        .or_else(|| chunk.get("code").and_then(|error_code| error_code.as_str()))
        .filter(|error_code| !error_code.is_empty());

    let details = [
        error_type.map(|error_type| format!("type: {error_type}")),
        error_code.map(|error_code| format!("code: {error_code}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if !details.is_empty() {
        message.push_str(" (");
        message.push_str(&details.join(", "));
        message.push(')');
    }

    Some(message)
}

pub(super) fn stream_openai_response(
    response: reqwest::Response,
    tool_name_map: OpenAiToolNameMap,
) -> BoxStream<'static, Result<ModelEvent>> {
    let raw_stream = response
        .bytes_stream()
        .eventsource()
        .map_err(|e| ModelError::StreamParse(e.to_string()))
        .and_then(|event| async move {
            if event.data == "[DONE]" {
                Ok(None)
            } else {
                parse_openai_chunk(&event.data).map(Some)
            }
        })
        .filter_map(
            |result: std::result::Result<Option<Vec<OpenAiChunkEvent>>, ModelError>| async move {
                match result {
                    Ok(Some(events)) => {
                        Some(futures::stream::iter(events.into_iter().map(Ok)).boxed())
                    }
                    Ok(None) => Some(futures::stream::empty::<Result<OpenAiChunkEvent>>().boxed()),
                    Err(e) => Some(futures::stream::once(async { Err(e) }).boxed()),
                }
            },
        )
        .flatten()
        .boxed();

    let mut accumulator = OpenAiToolCallAccumulator::with_tool_name_map(tool_name_map);
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
