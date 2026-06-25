# Kairox Live Tool Stream Fix

## Problem

`kairox-live` points at the local ChatMock OpenAI-compatible endpoint. Direct API checks show ChatMock can return normal responses, non-streaming tool calls, and streaming tool calls. Kairox does not reliably surface those tool calls, so turns can complete without tool execution.

The suspected failure path is:

1. OpenAI-compatible streaming chunks contain `delta.tool_calls`.
2. ChatMock sends `finish_reason: "tool_calls"` before the SSE stream ends.
3. Kairox converts that finish reason into `ModelEvent::Completed`.
4. The runtime stops consuming on `Completed`.
5. `OpenAiToolCallAccumulator::flush()` only runs after stream end, so pending tool calls are lost.

## Plan

1. Add a focused failing unit test for the OpenAI-compatible tool accumulator: if a completion event arrives while tool call chunks are pending, it must emit `ToolCallRequested` before `Completed`.
2. Update the accumulator so pending tool calls are flushed before forwarding completion.
3. Run focused `agent-models` tests and formatting checks.
4. Run `kairox-eval` from this worktree against `kairox-live` and verify a real LocalRuntime session can invoke a built-in tool against the local ChatMock endpoint.
5. Summarize whether the remaining issue is Kairox or the model API using actual evidence.
