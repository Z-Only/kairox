---
title: Troubleshooting & FAQ
description: Curated answers for the errors and questions that come up most often.
outline: [2, 3]
---

# Troubleshooting & FAQ

This page collects the symptoms most users hit on their first week with Kairox, together with the underlying cause and the fix. If your question is not here, search the [GitHub discussions](https://github.com/Z-Only/kairox/discussions) and ask if nothing matches.

## Install and build

### `bun: command not found`

Bun installed but is not on your PATH. The installer writes to `~/.bun/bin`. Add it:

```bash
export PATH="$HOME/.bun/bin:$PATH"
```

Persist that line in your shell rc (`~/.zshrc`, `~/.bashrc`).

### `npm`, `pnpm`, or `yarn` errors when running scripts

Kairox enforces Bun via the `packageManager` field. Use `bun install`, `bun run <script>`, and the `just` recipes. Do not mix package managers.

### `webkit2gtk` or `libsoup` not found on Linux

You are missing the Tauri 2 platform dependencies. Install per [Installation](./installation). On Ubuntu 24.04+ the package is `libwebkit2gtk-4.1-dev`. On older releases, `libwebkit2gtk-4.0-dev`. Match the version your distro ships.

### `link.exe not found` or MSVC errors on Windows

Install the Visual Studio 2022 C++ Build Tools and select the "Desktop development with C++" workload:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

### `xcrun: error: invalid active developer path` on macOS

```bash
xcode-select --install
```

### `cargo build` is slow on the first run

The first Tauri build compiles hundreds of crates and downloads platform SDKs. Expect 10–20 minutes on a typical laptop. Subsequent builds are incremental and take seconds.

If a build appears stalled, check whether `cargo` is actually producing output (it will print compilation progress) or whether it is stuck on a network operation. A stall longer than 30 minutes with no output is unusual; check disk space and your network.

### Husky pre-commit hooks did not run

You committed before running `bun install`. Husky installs its hooks via the `prepare` script. Run:

```bash
bun install
```

Then your next commit will go through format and lint gates.

### `just check` fails on a fresh checkout

If `just check` fails on a clean `origin/main` checkout, something is wrong in your environment — not in the repository. The first error tells you what:

- A missing platform dep (almost always) — see [Installation](./installation).
- An outdated Rust toolchain — `rustup update stable`.
- An outdated Bun — `bun upgrade`.

Do not start implementing on a broken baseline.

## Sessions and runtime

### "Model returned no content" or empty responses

The most common cause is a misconfigured profile. Verify:

1. `provider`, `model_id`, and `base_url` match what your provider expects.
2. `api_key_env` points to an env var that is actually set in your shell.
3. The model name is spelled exactly as the provider documents (`gpt-4.1`, not `gpt4`; `claude-sonnet-4-20250514`, not `claude-4`).

If everything looks right, run the live smoke test:

```bash
just test-live    # self-skips if GITHUB_TOKEN is not set
```

That runs against GitHub Models with a known-good profile to verify your toolchain end-to-end.

### "Permission denied" on a tool the model wants to run

This is expected behavior under the default `ApprovalPolicy::OnRequest` + `SandboxPolicy::WorkspaceWrite` pair: the policy engine prompts on anything risky and denies if you say no, or fails the call structurally if the active sandbox does not permit it (for example, any write attempt under `SandboxPolicy::ReadOnly`).

- To approve once: <kbd>Y</kbd> in the TUI, **Allow** in the GUI.
- To approve persistently for this workspace: **Always allow** in the GUI.
- To stop being prompted for sandbox-cleared calls: switch `ApprovalPolicy` to `Never` (TUI status bar selector; GUI `ChatApprovalSelector`).
- To allow writes that the sandbox is currently rejecting: switch `SandboxPolicy` to a wider variant such as `WorkspaceWrite` or `DangerFullAccess` (TUI: <kbd>B</kbd> cycles; GUI `ChatSandboxSelector`). Approval cannot widen the sandbox — only switching the sandbox can.

See [Permissions & Tools](../concepts/permissions-and-tools) for the full decision matrix.

### Context compaction triggers too often (or never)

Tune `[context].auto_compact_threshold` in `kairox.toml`. The default is `0.85` (compact when 85% of the active model's context window is used). Lower it (e.g., `0.7`) to compact earlier; raise it (e.g., `0.95`) to delay. Setting it to `1.0` disables auto-compaction; you can still trigger manual compaction from the command palette.

If your active model has an unusual context window that Kairox does not know about, set `context_window` and `output_limit` explicitly on the profile. See [Configuration](../reference/configuration).

### Mid-session model switch does not take effect

Switches are queued through the session actor and apply at the next turn boundary. If you switched while a turn was streaming, the switch waits for the turn to complete before the new model takes over. The trace shows a `ProfileChanged` event when the switch lands.

### "Failed to assemble context: budget exceeded"

A single message — usually a pasted file or a large tool result — exceeds the available context. Options:

1. Trigger a manual compaction first.
2. Switch to a profile with a larger `context_window`.
3. Lower `auto_compact_threshold` so the next compaction triggers earlier.
4. Trim the input.

## MCP

### MCP server stuck at "Starting"

The transport handshake is hanging. Check:

- For stdio: does the `command` exist on your PATH? Try running it manually in a terminal — `npx -y @modelcontextprotocol/server-filesystem /tmp` should print MCP messages on stderr.
- For SSE or Streamable HTTP: is the `url` reachable? `curl -i <url>` should return 200 (or a streaming response).
- For stdio servers that need env vars: is the var actually set? The empty-string convention (`MY_VAR = ""`) means "read the env var of the same name." If the env var is unset, the server starts but cannot authenticate.

The server emits `McpServerStarting` → `McpServerReady` (or `McpServerFailed`) — the failure event carries a diagnostic payload visible in the trace.

### MCP server tools not appearing in the picker

The server is started but not all tools are registered. Some servers expose tools only after a successful sub-command (e.g., a GitHub server needs the token to enumerate its tool set). Check the trace for a `McpServerFailed` after `Ready` — that usually means the handshake succeeded but tool discovery failed.

### "MCP server keeps restarting"

By default, `auto_restart = true` and `max_restart_attempts = 3`. After three failed restarts the manager gives up and emits `McpServerFailed` with the diagnostic. Look at the trace for the underlying error; often it is a missing env var or a stale command path. Fix the config and stop/start the server from the marketplace view (or `kill` it manually with `Ctrl+C` in the TUI and let the manager restart it).

## GUI

### GUI launches but the window is blank

The Vite dev server crashed or did not finish booting before Tauri tried to load the URL. Stop, restart `just tauri-dev`, and watch for Vite errors in the terminal output. If the page loads but content is missing, open the devtools (right-click in the window → "Inspect Element" in dev builds) and look for console errors.

### Pre-built GUI says "auto-update failed"

The updater could not reach GitHub Releases. Check your network and that GitHub is reachable. The update is downloaded on the next launch — failure is non-fatal.

### Settings changes do not apply

Some settings (model defaults, MCP server registration) apply on the next session; others (skills, `ApprovalPolicy`, `SandboxPolicy`) apply immediately. If a setting did not take effect, start a new session and try again. If it still does not, file an issue with the exact setting.

## Data and storage

### Where does Kairox store data?

| What                                | Where                                                  |
| ----------------------------------- | ------------------------------------------------------ |
| Session events (chat, trace, tasks) | `~/.kairox/kairox.db` (SQLite)                         |
| Memory entries                      | Same SQLite database, separate tables                  |
| User-scoped skills                  | `~/.kairox/skills/`                                    |
| Workspace-scoped skills             | `<workspace>/.kairox/skills/`                          |
| Project config                      | `<workspace>/.kairox/config.toml`                      |
| User config                         | `~/.kairox/config.toml`                                |
| Plugins                             | `~/.kairox/plugins/` or `<workspace>/.kairox/plugins/` |

The exact path may differ on Windows (`%APPDATA%\kairox\`) and macOS (`~/Library/Application Support/kairox/`). The GUI's "About" panel shows the resolved paths for your install.

### How do I reset memory?

In the GUI, open the memory browser, select entries, and delete. From the TUI trace panel: select a memory entry and press <kbd>D</kbd> (then <kbd>Y</kbd> to confirm).

To wipe everything, stop the app and remove `~/.kairox/kairox.db` (or back it up first). The next launch creates a fresh database.

### How do I export a session?

Sessions are SQLite rows; the export feature exists at the API level. For now, query the database directly with the `sqlite3` CLI to extract events for a session ID. A first-class export UI is on the roadmap.

## Logging

### How do I enable verbose logging?

Set `RUST_LOG` before launching:

```bash
RUST_LOG=agent_runtime=debug,agent_models=debug just tui
```

Filters use the `tracing` syntax. To see everything (very loud):

```bash
RUST_LOG=debug just tui
```

For the GUI, set the env var before `just tauri-dev`. Log output goes to the terminal that launched it.

### Privacy and tracing in production

Production configuration defaults to minimal trace when a real model client or shell tool is configured. This is enforced in code, not in TOML. Verbose tracing is automatically allowed only when the configured providers and tools are safe for development (e.g., `fake` provider with no real shell). See [Configuration](../reference/configuration#privacy-defaults).

## Getting help

If you have a question that is not here:

- Search [GitHub Discussions](https://github.com/Z-Only/kairox/discussions) — most asked-and-answered questions live there.
- Open a new discussion for product/integration questions.
- Open a GitHub Issue for reproducible bugs with a minimal reproduction.
- Read the [Crate Index](../reference/crate-index) to find the right module's source if you want to dig in.

## What this page does not cover

This page is the curated FAQ. It does not cover the conceptual model behind the runtime ([Architecture](../concepts/architecture)), the configuration schema in detail ([Configuration](../reference/configuration)), or the contribution workflow ([Contributing](../community/contributing)).
