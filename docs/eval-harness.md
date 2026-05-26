# Kairox Eval Harness

`kairox-eval` is a headless benchmark runner for Kairox agent sessions. It runs JSONL scenarios through the same `LocalRuntime`/`AppFacade` path used by the TUI and GUI, records per-scenario JSONL results, and writes aggregate metrics for version-to-version comparisons.

## Run

```bash
cargo run -p agent-eval --bin kairox-eval -- run \
  --scenarios examples/eval/smoke.jsonl \
  --output target/eval/results.jsonl \
  --summary target/eval/summary.json \
  --workspace .
```

By default the harness:

- loads normal Kairox model configuration for the selected workspace;
- uses `on_request` approval with `workspace_write` sandbox by default;
- enables built-in tools rooted at `--workspace`;
- disables MCP servers and hooks for reproducibility.

Use `--enable-mcp` or `--enable-hooks` only when the benchmark explicitly depends on them.

## Scenario Format

Each non-empty, non-comment JSONL row is one scenario:

```json
{
  "id": "fake-smoke",
  "prompt": "Say hello from the configured fake model.",
  "profile": "fake",
  "approval_policy": "on_request",
  "sandbox_policy": { "kind": "workspace_write" },
  "tags": ["smoke"],
  "expected": {
    "assistant_contains": ["Kairox"],
    "event_types": ["UserMessageAdded", "AssistantMessageCompleted"],
    "min_tool_invocations": 0,
    "max_tool_failures": 0
  }
}
```

`profile`, `approval_policy`, `sandbox_policy`, and `tags` are optional. If `profile` is omitted, the harness uses `--profile`, then the loaded config default.

## Result Metrics

Each result row includes:

- pass/fail state and expectation failures;
- selected profile;
- final assistant response;
- elapsed time;
- event type sequence;
- tool invocation and failure counts;
- last context input token estimate and context window when emitted by runtime;
- optional full trace when `--include-trace` is set.

The summary reports total cases, pass count, success rate, elapsed time, tool counts, and summed context input token estimates. Provider-specific dollar costs are intentionally not baked into this crate; keep pricing tables outside the runner so historical results remain comparable when vendors change prices.
