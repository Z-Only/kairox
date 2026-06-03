---
title: Eval Harness
description: Headless benchmark runner for Kairox agent sessions using JSONL scenarios.
---

# Eval Harness

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

## Scenario format

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

| Field             | Required | Description                                                                               |
| ----------------- | -------- | ----------------------------------------------------------------------------------------- |
| `id`              | yes      | Unique scenario identifier                                                                |
| `prompt`          | yes      | User message sent to the agent                                                            |
| `profile`         | no       | Model profile override (falls back to `--profile`, then config default)                   |
| `approval_policy` | no       | `never` / `on_request` / `always`                                                         |
| `sandbox_policy`  | no       | `{ "kind": "read_only" }` / `{ "kind": "workspace_write" }` / `{ "kind": "full_access" }` |
| `tags`            | no       | String tags for `--tags` filtering                                                        |
| `expected`        | no       | Assertion block (see below)                                                               |

### Expectations

| Field                  | Type       | Description                                                         |
| ---------------------- | ---------- | ------------------------------------------------------------------- |
| `assistant_contains`   | `string[]` | Substrings the assistant response must contain                      |
| `event_types`          | `string[]` | Domain event types that must appear in the trace                    |
| `min_tool_invocations` | `number`   | Minimum tool calls expected                                         |
| `max_tool_failures`    | `number`   | Maximum tool failures allowed before marking the scenario as failed |

## Result metrics

Each result row includes:

- pass/fail state and expectation failures;
- selected profile;
- final assistant response;
- elapsed time;
- event type sequence;
- tool invocation and failure counts;
- last context input token estimate and context window when emitted by runtime;
- optional full trace when `--include-trace` is set.

The summary reports total cases, pass count, success rate, elapsed time, tool counts, and summed context input token estimates.

## CLI flags

| Flag              | Default        | Description                         |
| ----------------- | -------------- | ----------------------------------- |
| `--scenarios`     | required       | Path to JSONL scenario file         |
| `--output`        | required       | Path for per-scenario JSONL results |
| `--summary`       | required       | Path for aggregate JSON summary     |
| `--workspace`     | `.`            | Workspace root for tool sandboxing  |
| `--profile`       | config default | Default model profile               |
| `--tags`          | all            | Comma-separated tag filter          |
| `--fail-fast`     | `false`        | Stop on first failure               |
| `--include-trace` | `false`        | Include full event trace in results |
| `--enable-mcp`    | `false`        | Enable MCP servers during scenarios |
| `--enable-hooks`  | `false`        | Enable hooks during scenarios       |

## Architecture

The eval harness is built on the same `LocalRuntime` used by the TUI and GUI. It exercises the full runtime path — model calls, tool execution, policy enforcement, context budgeting — without any UI layer. This ensures that eval results reflect production behavior.

Key types in `agent-eval`:

- `EvalHarness` — orchestrates scenario execution, manages runtime setup and teardown.
- `EvalScenario` — parsed JSONL row with prompt, profile, policy, tags, and expectations.
- `EvalReport` — aggregates individual scenario results into a summary.

## Related

- [Runtime & Sessions](../concepts/runtime-and-sessions) — how the runtime processes each turn.
- [Permissions & Tools](../concepts/permissions-and-tools) — approval and sandbox policies used during eval.
- [Configuration](./configuration) — model profile and context budget settings.
