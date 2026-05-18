use super::tool_accumulator::OpenAiToolCallAccumulator;
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

pub(super) fn stream_openai_response(
    response: reqwest::Response,
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

    let mut accumulator = OpenAiToolCallAccumulator::new();
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
mod tests {
    use super::*;

    #[test]
    fn parses_token_delta_from_chunk() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenAiChunkEvent::Event(ModelEvent::TokenDelta(t)) => assert_eq!(t, "Hello"),
            _ => panic!("expected TokenDelta event"),
        }
    }

    #[test]
    fn parses_tool_call_start_from_chunk() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"fs.read","arguments":"{\"pa"}}]},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 2);
        match &events[0] {
            OpenAiChunkEvent::ToolCallStarted { index, id, name } => {
                assert_eq!(*index, 0);
                assert_eq!(id, "call_1");
                assert_eq!(name, "fs.read");
            }
            _ => panic!("expected ToolCallStarted"),
        }
        match &events[1] {
            OpenAiChunkEvent::ToolCallArgumentDelta {
                index,
                partial_arguments,
            } => {
                assert_eq!(*index, 0);
                assert_eq!(partial_arguments, "{\"pa");
            }
            _ => panic!("expected ToolCallArgumentDelta"),
        }
    }

    #[test]
    fn parses_tool_call_argument_delta_chunk() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\": \"README.md\"}"}}]},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenAiChunkEvent::ToolCallArgumentDelta {
                index,
                partial_arguments,
            } => {
                assert_eq!(*index, 0);
                assert_eq!(partial_arguments, "th\": \"README.md\"}");
            }
            _ => panic!("expected ToolCallArgumentDelta"),
        }
    }

    #[test]
    fn parses_completion_event() {
        let data = r#"{"choices":[{"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert!(matches!(
            &events[0],
            OpenAiChunkEvent::Event(ModelEvent::Completed { usage: Some(u) })
            if u.input_tokens == 10 && u.output_tokens == 5
        ));
    }
}
