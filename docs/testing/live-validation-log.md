# Live Validation Log

This log tracks Kairox behavior that has been exercised against a real model
and the real GUI/runtime path. Use it to avoid repeating recent checks and to
spot entries that need revalidation after related code changes.

## Rules

- Prefer `ali-mo-claude` for live GUI/runtime validation when credentials are
  available. Use Fake only for deterministic tests or when the live profile is
  unavailable, and record that fallback explicitly.
- Add entries only when there is fresh evidence from the current code state:
  commit, model/profile, exact scenario, verification commands or observable
  outputs, and cleanup state.
- Treat an entry as stale before relying on it if a later PR changes the
  listed area, Tauri IPC path, runtime agent loop, tool policy, GUI component,
  or model/tool protocol involved in the scenario.
- Do not include API keys, tokens, raw config files, or secret-bearing logs.
- Previous live checks that are not listed here should be treated as
  unrecorded until they are rerun or linked to current, reviewable evidence.

## Entries

### 2026-06-05 10:33 CST — GUI builtin read/search/shell tools

- Commit: `c6a431b0`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`,
  `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, registered a temp git project at
  `/tmp/kairox-read-shell-project-0605`, opened a project session in the real
  GUI, selected `Ali Mo · Claude Opus 4 6`, and sent one live model turn that
  had to call `fs.list`, `fs.read`, `search.ripgrep`, and `shell.exec` exactly
  once in that order. The fixture project had `notes/alpha.txt` containing
  `READ_SEARCH_SENTINEL_0605` and `notes/beta.txt` as a second list entry. The
  final response marker was split into `READ_SHELL_TOOLS_FINAL_` and `OK_0605`
  so the full `READ_SHELL_TOOLS_FINAL_OK_0605` string did not exist before the
  model response.
- Method: `HOME=/tmp/kairox-read-shell-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-read-shell-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
CARGO_TARGET_DIR=/Users/chanyu/AIProjects/kairox/target KAIROX_DEV_PORT=1443
KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features pilot`. The temp
  profile used `provider = "ali-mo"` and `api_key_env =
  "KAIROX_VALIDATION_ALI_MO_KEY"`; the env value was supplied only to the local
  process from the user profile and was not written to temp files or logs. No
  Starpoint approval or AMFI workaround was needed.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/tauri-pilot-dev.kairox.agent.dev1443.sock`, `windows` showed
  `http://localhost:1443/#/workbench/ses_4b2a2f58fbff4d388db68b18fdfb4f0a`,
  and `tauri-pilot logs --level error` reported `No logs captured`. IPC
  `list_profiles_with_limits` showed `ali-mo-claude` with `has_api_key=true`,
  `provider="ali-mo"`, `model_id="claude-opus-4-6"`, `context_window=200000`,
  and `output_limit=16384`. The GUI project session materialized as
  `ses_4b2a2f58fbff4d388db68b18fdfb4f0a` with profile `ali-mo-claude`, branch
  `main`, workspace-write sandbox, and worktree path
  `/tmp/kairox-read-shell-project-0605`; `get_session_git_status` returned
  `kind=clean`. The chat stream showed completed `Tool call:` rows and
  completed trace rows for `fs.list`, `fs.read`, `search.ripgrep`, and
  `shell.exec`, then page text contained `READ_SHELL_TOOLS_FINAL_OK_0605`.
  `tauri-pilot assert count '[data-test="permission-prompt"]' 0` and
  `assert count '[data-test="chat-permission-item"]' 0` both passed.
  Exported trace had `event_count=41`, with `ModelToolCallRequested` and
  `ToolInvocationCompleted` in order for `fs.list`, `fs.read`,
  `search.ripgrep`, and `shell.exec`; it had four `PermissionGranted` events and
  no `PermissionRequested`, `PermissionDenied`, or `ToolInvocationFailed`
  events. `fs.list` output included `notes/alpha.txt` and `notes/beta.txt`;
  `fs.read` output included `READ_SEARCH_SENTINEL_0605 in alpha notes`;
  `search.ripgrep` output used the fallback search engine and found the sentinel
  in `notes/alpha.txt`; `shell.exec` output was
  `/private/tmp/kairox-read-shell-project-0605`, confirming the shell ran in the
  project root rather than the GUI app cwd. The trace summary also confirmed the
  user prompt lacked the full final marker while `AssistantMessageCompleted`
  content was exactly `READ_SHELL_TOOLS_FINAL_OK_0605`. After shutdown, no
  process from this worktree or listener on port `1443` remained, and the temp
  `HOME`, runtime dir, project fixture, and exported trace file were removed.
- Result: Pass for the live GUI/runtime builtin read/search/shell path. Ali Mo
  can see and call the read-class tools and a read-only shell command from a
  project session, Kairox auto-grants those read effects without showing a
  permission card, records the tool sequence in trace, scopes shell execution to
  the project root, and completes the live turn normally.

### 2026-06-05 09:58 CST — GUI provider `code_execution` server tool

- Commit: `76b3ae2c`
- Model: `ali-mo-code-tool` (`ali-mo` / `claude-opus-4-6`,
  `client_identity = "claude_code"`) and `tokensflow-code-tool`
  (`anthropic` / `anthropic/claude-opus-4.7`)
- Scenario: In an isolated temp `HOME`, configured provider-side
  `server_tool_code_execution = true`, started the real GUI with pilot, selected
  the code-execution profile through the GUI model selector, and sent live
  composer turns that asked the model to use provider `code_execution` to print
  split marker strings. The prompts explicitly prohibited Kairox local tools so
  any success would have to come from the provider server tool path.
- Method: `HOME=/tmp/kairox-server-tool-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-server-tool-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
CARGO_TARGET_DIR=/Users/chanyu/AIProjects/kairox/target KAIROX_DEV_PORT=1442
KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features pilot` from
  `apps/agent-gui`, with validation API keys supplied only through local process
  env vars. The worktree updated Kairox to the current Anthropic tool type
  `code_execution_20250825`, current beta header `code-execution-2025-08-25`,
  and parser support for `bash_code_execution_tool_result` /
  `text_editor_code_execution_tool_result`.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/tauri-pilot-dev.kairox.agent.dev1442.sock`, and `windows` showed
  `http://localhost:1442/#/workbench`. Against Ali Mo, the legacy
  `code_execution_20250522` shape without a code-execution beta was rejected as
  `tools.18.custom.input_schema: Field required`; adding legacy beta
  `code-execution-2025-05-22` was rejected as an unexpected
  `anthropic-beta` value. After updating to `code_execution_20250825`, the
  no-code-beta request was still treated as custom tool
  (`tools.12.custom.input_schema: Field required`), while the current beta
  `code-execution-2025-08-25` was also rejected as an unexpected
  `anthropic-beta` value. The last Ali Mo session was
  `ses_74fdc2a84eb641d3b0839e22f376e24c`, and the GUI rendered the provider
  error without any Kairox permission prompt. A second attempt with
  `tokensflow-code-tool` selected through the GUI created session
  `ses_5c53ab7d33854204b1e79161c86f4977`; its exported trace had
  `event_count=7`, `ContextAssembled` at `1676 / 181616` tokens with
  `tool_definitions=1282`, and `AgentTaskFailed` with
  `model returned an empty response; check model availability, quota, or plan`.
  `tauri-pilot logs --level error` reported `No logs captured`, and
  `tauri-pilot assert count '[data-test="permission-prompt"]' 0` passed.
  Focused tests passed for the updated request/header/parser behavior:
  `cargo test -p agent-config server_tool_code_execution_appends_current_beta_header -- --nocapture`,
  `cargo test -p agent-models code_execution -- --nocapture`, and
  `cargo test -p agent-models server_tool -- --nocapture`.
- Result: Not passed with the available live providers. Kairox's local protocol
  implementation was updated to the current Anthropic `code_execution_20250825`
  contract and verified with focused tests, but live end-to-end provider-side
  code execution still requires revalidation with a provider that accepts the
  current code-execution beta/tool combination. Ali Mo should be treated as
  unsupported for this server tool until its gateway support changes.

### 2026-06-05 09:20 CST — GUI monitor lifecycle tools (#841/#849/#855)

- Commit: `9b22b564`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`,
  `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, registered a temp project at
  `/tmp/kairox-monitor-project-0605`, opened a project draft in the real GUI,
  selected `Ali Mo · Claude Opus 4 6`, and sent one live model turn that had to
  call `monitor.start`, `monitor.list`, and `monitor.stop` in order. The monitor
  command wrote `pwd` to `monitor-cwd.txt`, emitted `MONITOR_LIVE_LINE_0605`,
  and slept long enough for the model to list and stop it. The final response
  marker was split into `MONITOR_LIFECYCLE_FINAL_` and `OK_0605` so the full
  `MONITOR_LIFECYCLE_FINAL_OK_0605` string did not exist before the model
  response.
- Method: `HOME=/tmp/kairox-monitor-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-monitor-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
CARGO_TARGET_DIR=/Users/chanyu/AIProjects/kairox/target KAIROX_DEV_PORT=1441
KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features pilot`. The temp
  profile used `provider = "ali-mo"` and `api_key_env =
  "KAIROX_VALIDATION_ALI_MO_KEY"` with the value supplied only as a local
  process env var. No Starpoint approval or AMFI workaround was needed.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/tauri-pilot-dev.kairox.agent.dev1441.sock`. The GUI project session
  materialized as `ses_b6ea2bd5a5054e61aaa0b305f43e1f33` and kept the footer at
  `Ali Mo · Claude Opus 4 6`, `按需`, and `工作区写入`. The chat stream showed
  completed entries for `Tool call: monitor.start`, `monitor.start`, monitor
  item `live monitor cwd 0605`, `Tool call: monitor.list`, `monitor.list`,
  `Tool call: monitor.stop`, and `monitor.stop`; page text contained
  `MONITOR_LIFECYCLE_FINAL_OK_0605`. IPC `list_monitors` returned `[]` after the
  turn. `/tmp/kairox-monitor-project-0605/monitor-cwd.txt` contained
  `/private/tmp/kairox-monitor-project-0605`, while neither the GUI app root nor
  the validation worktree root had `monitor-cwd.txt`, confirming
  `monitor.start` ran in the project root rather than the GUI cwd. Exported
  trace had `event_count=35`, with model tool requests and matching started /
  completed events for `monitor.start`, `monitor.list`, and `monitor.stop`.
  Tool outputs were `Monitor started: mon_1`,
  `- mon_1 (live monitor cwd 0605): persistent=false, timeout=120000ms`, and
  `Monitor stopped: mon_1`. The trace also contained `MonitorStarted` for
  `mon_1`, `MonitorEvent` line `MONITOR_LIVE_LINE_0605`, `MonitorStopped` with
  `UserStopped`, `ContextAssembled` at `2266 / 181616` tokens, and
  `AssistantMessageCompleted` content `MONITOR_LIFECYCLE_FINAL_OK_0605`.
  Failure events were absent, and `tauri-pilot logs --level error` reported
  `No logs captured`. After shutdown, no `agent-gui-tauri` process or `1441`
  listener remained.
- Result: Pass for the live GUI/runtime monitor lifecycle. Ali Mo can discover
  and call the monitor tools in sequence, the shared registry lists and stops
  the live monitor, monitor stdout and stop events are persisted into trace, and
  project-session monitor commands are scoped to the project root.

### 2026-06-05 09:00 CST — GUI project draft restore settings

- Commit: `353aefb8`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`,
  `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, started the real GUI with pilot, created
  an ordinary live Ali Mo session, registered a temp project, opened a project
  draft, selected `Ali Mo · Claude Opus 4 6`, typed an unsent project draft,
  restarted the full GUI process twice, and verified the restored draft kept its
  project, branch, text, model, approval, and sandbox settings before first
  send. This first reproduced a bug where the project draft text and branch
  restored but the footer model reset to `Fake · Fake`; the fix persists pending
  draft settings in `kairox.last-workbench-state` and routes pre-send model
  selection through the session store so it is saved immediately.
- Method: `HOME=/tmp/kairox-context-restore-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-context-restore-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
CARGO_TARGET_DIR=/Users/chanyu/AIProjects/kairox/target KAIROX_DEV_PORT=1440
KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features pilot`. The temp
  profile used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"` with the value
  supplied only as a local process env var. No Starpoint approval or AMFI
  workaround was needed for this lane.
- Evidence: Before the fix, a restored project draft under
  `prj_233b4575b97d46bb8b8b7dd29e244d4f` kept the textarea content and `main`
  branch but rendered `选择模型。当前模型：Fake · Fake`; localStorage contained
  only `{"kind":"project-draft","projectId":"...","branch":"main"}`. After the
  fix and a cold restart, selecting Ali Mo wrote
  `profile:"ali-mo-claude"`, `reasoningEffort:null`, `approval:"on_request"`,
  and the workspace-write `sandboxJson` into `kairox.last-workbench-state`.
  A second full process restart restored the same project draft with the split
  marker prompt ending in `CHAT_CONTEXT_RESTORE_PROJECT_` plus `OK_0605`, branch
  `main`, and footer `Ali Mo · Claude Opus 4 6`. Sending the restored draft
  materialized project session `ses_4453a8f534cb4d4c8c831d3802a4378f`;
  `list_project_sessions` reported `profile=ali-mo-claude`,
  `approval_policy=on_request`,
  `sandbox_policy={"kind":"workspace_write","network_access":false,"writable_roots":[]}`,
  `branch=main`, and `visibility=visible`. Page text contained
  `CHAT_CONTEXT_RESTORE_PROJECT_OK_0605`, and `tauri-pilot logs --level error`
  reported `No logs captured`. Exported trace had `event_count=12`, including
  `ModelProfileSwitched` from `fake` to `ali-mo-claude`, `ContextAssembled` at
  `1973 / 181616` tokens with `project_instruction=21`, four
  `ModelTokenDelta` events, and `AssistantMessageCompleted` content
  `CHAT_CONTEXT_RESTORE_PROJECT_OK_0605`.
- Result: Fixed and passed for project draft restore. Pending ordinary/project
  draft settings now survive process restart and the first send uses the
  restored model, approval, and sandbox settings instead of falling back to the
  default Fake profile.

### 2026-06-05 08:15 CST — GUI runtime instance registry (#817)

- Commit: `ca1ca8b0`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`)
- Scenario: In one isolated temp `HOME`, started GUI instance A on port `1438`
  and GUI instance B on port `1439`, both with pilot enabled and sharing the same
  Kairox data dir. Verified runtime instance JSON records while both were
  running, confirmed B reported A through the startup `Other Kairox instances`
  warning, sent a no-tool live Ali Mo turn from B, killed B to leave a stale
  record, then started GUI instance C on port `1439` to verify startup pruning
  removed stale B while still reporting running A.
- Method: For A and B/C, ran `HOME=/tmp/kairox-instance-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-instance-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
CARGO_TARGET_DIR=/Users/chanyu/AIProjects/kairox/target KAIROX_DEV_PORT=<port>
KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features pilot`. The temp
  profile used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"` with the value
  supplied only as a local process env var. The shared target kept the second
  build incremental; no Starpoint approval was needed.
- Evidence: A printed pilot socket
  `/tmp/kairox-instance-runtime-0605/tauri-pilot-dev.kairox.agent.dev1438.sock`,
  `tauri-pilot ping` returned ok, and `windows` showed
  `http://localhost:1438/#/workbench/ses_8338b0ea221041c2b9618c60f5a8ca98`.
  A created exactly one JSON record under
  `/tmp/kairox-instance-home-0605/.kairox/runtime/instances/`:
  `kind=gui`, `pid=62496`, `database_filename=kairox-gui.sqlite`,
  `data_dir=/tmp/kairox-instance-home-0605/.kairox`,
  `workspace_root=/Users/chanyu/AIProjects/kairox/.worktrees/docs-live-instance-registry-0605/apps/agent-gui/src-tauri`,
  and `executable=/Users/chanyu/AIProjects/kairox/target/debug/agent-gui-tauri`.
  B printed pilot socket
  `/tmp/kairox-instance-runtime-0605/tauri-pilot-dev.kairox.agent.dev1439.sock`
  and startup warning
  `Other Kairox instances: gui pid=62496 db=kairox-gui.sqlite workspace=.../apps/agent-gui/src-tauri`.
  Both A and B pilot sockets pinged successfully at the same time; the shared
  records dir contained exactly two records for PIDs `62496` and `66848`, both
  with `kind=gui`, `database_filename=kairox-gui.sqlite`, and the same data dir
  and workspace root. B switched from Fake to `Ali Mo · Claude Opus 4 6` through
  the GUI and sent a no-tool prompt whose expected marker was split into
  `INSTANCE_REGISTRY_FINAL_` and `OK_0605`. Page polling found
  `INSTANCE_REGISTRY_FINAL_OK_0605`, and `tauri-pilot logs --level error`
  reported `No logs captured`. Exported trace for
  `ses_8338b0ea221041c2b9618c60f5a8ca98` had `event_count=11`, including
  `ModelProfileSwitched` from `fake` to `ali-mo-claude`, `ContextAssembled` at
  `1562 / 181616` tokens, three `ModelTokenDelta` events, and
  `AssistantMessageCompleted` content `INSTANCE_REGISTRY_FINAL_OK_0605`; trace
  summary showed `user_prompt_has_full_marker=false` and
  `user_prompt_has_split_marker=true`. After B was killed, its stale record still
  existed while only A's `agent-gui-tauri` process was running. Starting C on
  port `1439` printed the same `Other Kairox instances` warning for A only,
  proving startup `prune_stale()` removed B's dead-PID record. The records dir
  then contained exactly two live records for PIDs `62496` and `74591`; both
  matched running `agent-gui-tauri` processes. After shutdown, no
  `agent-gui-tauri` process or `1438`/`1439` listener remained.
- Result: Pass for GUI runtime instance tracking. The GUI registers local
  instance metadata in the expected data-dir location, reports other running
  instances on startup, keeps distinct pilot sockets for simultaneous dev ports,
  prunes stale records on subsequent startup, and concurrent instance tracking
  does not break live Ali Mo chat.

### 2026-06-05 08:02 CST — GUI environment automation builtin tools (#806)

- Commit: `aa3d89c4`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, started the real GUI with pilot, switched
  the workbench from Fake to `ali-mo-claude`, changed approval to Always and the
  sandbox to Danger Full Access through the GUI, then sent one live composer turn
  requiring exactly one `browser.batch` call followed by exactly one
  `computer.use` call. The prompt split the expected marker into
  `ENV_TOOLS_FINAL_` and `OK_0605` so the full `ENV_TOOLS_FINAL_OK_0605` string
  did not exist before the model response.
- Method: `HOME=/tmp/kairox-env-tools-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-env-tools-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
KAIROX_DEV_PORT=1437 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features
pilot` compiled the app and printed the isolated pilot socket. On this machine,
  `cargo run` rewrote the debug binary with a linker-only ad-hoc signature and
  macOS AMFI killed `target/debug/agent-gui-tauri` before setup, so verification
  continued by running `KAIROX_DEV_PORT=1437 KAIROX_DEV_STRICT_PORT=1 bun run
  dev`, then `codesign --force --sign - target/debug/agent-gui-tauri`, then
  `./target/debug/agent-gui-tauri` under the same temp environment. The temp
  profile used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"` with the value
  supplied only as a local process env var.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/kairox-env-tools-runtime-0605/tauri-pilot-dev.kairox.agent.dev1437.sock`,
  and `windows` showed
  `http://localhost:1437/#/workbench/ses_0a67c5eac68943678b44dafe0003ec97`.
  The GUI controls rendered `Ali Mo · Claude Opus 4 6`, `总是`, and
  `完全访问（危险）`; IPC confirmed `get_session_approval_policy=always` and
  `get_session_sandbox_policy={"kind":"danger_full_access"}`. The live model
  requested `browser.batch` with actions `navigate about:blank`, `get_state`,
  `get_text body`, and `close`; the GUI showed an approval card and clicking
  `允许` completed the tool with preview `total=4`, `succeeded=4`, `failed=0`,
  including `Navigated to about:blank` and `Browser state retrieved`. The live
  model then requested `computer.use({"action":"get_screen_size"})`; clicking
  `允许` completed the tool with preview `Screen size: 1920x1080` and
  `screen_size={width:1920,height:1080}`. Page checks showed
  `ENV_TOOLS_FINAL_OK_0605`, `permissionCount=0`, `cancelVisible=false`, and
  `tauri-pilot logs --level error` reported `No logs captured`.
  Exported trace for session `ses_0a67c5eac68943678b44dafe0003ec97` had
  `event_count=28`, included `ModelProfileSwitched` from `fake` to
  `ali-mo-claude`, `ContextAssembled` with `tool_definitions=1668` tokens, two
  `ModelToolCallRequested` events for `browser.batch` and `computer.use`, two
  matching `PermissionRequested`/`PermissionGranted` pairs, two
  `ToolInvocationStarted`/`ToolInvocationCompleted` pairs, and
  `AssistantMessageCompleted` content `ENV_TOOLS_FINAL_OK_0605`. Trace summary
  showed `user_prompt_has_full_marker=false` and
  `user_prompt_has_split_marker=true`.
- Result: Pass for the live GUI/runtime registration, permission, execution, and
  trace path for the #806 builtin environment automation tools. This validates
  that Ali Mo can see and call `browser.batch` and `computer.use` and that Kairox
  executes and records them correctly. It does not claim real external browser or
  desktop control, because the current `PlaywrightManager` and `DesktopBackend`
  implementations are explicitly simulated placeholders.

### 2026-06-05 07:50 CST — GUI devtools setting persistence (#812)

- Commit: `efadf0af`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, started the real GUI with pilot, opened
  Settings -> General, verified the developer tools toggle default state, toggled
  it off and on through the GUI, checked the persisted `gui-settings.toml` file
  and restart indicator after each change, then returned to the workbench,
  selected `ali-mo-claude`, and sent a no-tool prompt. The prompt split the
  expected marker into `DEVTOOLS_SETTING_FINAL_` and `OK_0605` so the full
  `DEVTOOLS_SETTING_FINAL_OK_0605` string did not exist before the model response.
- Method: `HOME=/tmp/kairox-devtools-setting-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-devtools-setting-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
KAIROX_DEV_PORT=1436 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features
pilot` compiled the app and printed the isolated pilot socket. On this machine,
  `cargo run` rewrote the debug binary with a linker-only ad-hoc signature and
  macOS AMFI killed `target/debug/agent-gui-tauri` before setup, so verification
  continued by running `KAIROX_DEV_PORT=1436 KAIROX_DEV_STRICT_PORT=1 bun run
  dev`, then `codesign --force --sign - target/debug/agent-gui-tauri`, then
  `./target/debug/agent-gui-tauri` under the same temp environment. The temp
  profile used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"` with the value
  supplied only as a local process env var.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/kairox-devtools-setting-runtime-0605/tauri-pilot-dev.kairox.agent.dev1436.sock`,
  and `windows` showed
  `http://localhost:1436/#/workbench/ses_76612cdf838c4557819b99c4d26835d9`.
  Initial `get_gui_settings` returned `devtools_enabled=true`,
  `default_devtools_enabled=true`, and `requires_restart=false`; no
  `/tmp/kairox-devtools-setting-home-0605/.kairox/gui-settings.toml` existed yet.
  The Settings -> General checkbox `[data-test="settings-devtools"]` was checked,
  with no restart or error message. Clicking the checkbox off through the GUI made
  `get_gui_settings` return `devtools_enabled=false` and `requires_restart=true`;
  the UI checkbox was unchecked, `[data-test="settings-devtools-restart"]`
  displayed `需要重启`, no error was visible, and
  `.kairox/gui-settings.toml` contained `devtools_enabled = false`. Clicking the
  checkbox on again made `get_gui_settings` return `devtools_enabled=true` and
  `requires_restart=false`; the checkbox was checked, restart/error messages were
  absent, and `.kairox/gui-settings.toml` contained `devtools_enabled = true`.
  The GUI model selector displayed `Ali Mo · Claude Opus 4 6`; before send, the
  textarea did not contain `DEVTOOLS_SETTING_FINAL_OK_0605` and the send button
  was enabled. Page polling found `DEVTOOLS_SETTING_FINAL_OK_0605` on the first
  check after send. Exported trace for session
  `ses_76612cdf838c4557819b99c4d26835d9` had `event_count=11`, with events
  `SessionInitialized`, `ModelProfileSwitched`, `UserMessageAdded`,
  `ContextAssembled`, `AgentTaskCreated`, `AgentTaskStarted`, three
  `ModelTokenDelta`, `AssistantMessageCompleted`, and `AgentTaskCompleted`. The
  trace summary showed `user_prompt_has_full_marker=false`,
  `user_prompt_has_split_marker=true`, and `assistant_has_full_marker=true`.
  `tauri-pilot logs --level error` reported `No logs captured`.
- Result: Pass for GUI devtools settings. The toggle reflects the debug-build
  default, persists explicit overrides, sets the restart-required state only when
  the persisted value differs from the running window state, clears that state
  when toggled back, and does not break live Ali Mo interaction in the same GUI
  run.

### 2026-06-05 07:38 CST — GUI generic worktree basename label (#845)

- Commit: `33a1fdc9`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, created a real git project at
  `/tmp/kairox-generic-label-project-0605/repo`, created a project worktree
  session for branch `wt-validation-0604`, moved the actual git worktree to
  `/tmp/kairox-generic-label-project-0605/repo/worktree`, and updated the temp
  GUI DB session binding to model an existing/external project session whose
  `worktree_path` basename is the generic word `worktree`. Navigated the real GUI
  to that session, verified the composer git metadata, selected `ali-mo-claude`,
  and sent a no-tool prompt. The prompt split the expected marker into
  `GENERIC_WORKTREE_LABEL_FINAL_` and `OK_0605` so the full
  `GENERIC_WORKTREE_LABEL_FINAL_OK_0605` string did not exist before the model
  response.
- Method: `HOME=/tmp/kairox-generic-label-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-generic-label-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
KAIROX_DEV_PORT=1435 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features
pilot` compiled the app and printed the isolated pilot socket. On this machine,
  `cargo run` rewrote the debug binary with a linker-only ad-hoc signature and
  macOS AMFI killed `target/debug/agent-gui-tauri` before setup, so verification
  continued by running `KAIROX_DEV_PORT=1435 KAIROX_DEV_STRICT_PORT=1 bun run
  dev`, then `codesign --force --sign - target/debug/agent-gui-tauri`, then
  `./target/debug/agent-gui-tauri` under the same temp environment. The temp
  profile used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"` with the value
  supplied only as a local process env var.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/kairox-generic-label-runtime-0605/tauri-pilot-dev.kairox.agent.dev1435.sock`,
  and `windows` showed
  `http://localhost:1435/#/workbench/ses_385833cb4e53422497313ef7200bcd52`.
  `list_project_sessions` returned session
  `ses_385833cb4e53422497313ef7200bcd52` with
  `worktree_path=/tmp/kairox-generic-label-project-0605/repo/worktree`, branch
  `wt-validation-0604`, and `visibility=visible`. `get_session_git_status`
  returned `kind=clean`, branch `wt-validation-0604`, and the same moved
  worktree path. The GUI composer `[data-test="session-git-meta"]` rendered
  exactly `wt-validation-0604 · wt-validation-0604`; DOM checks showed
  `containsWorktreeWord=false` and `containsMovedPath=false`, so neither the
  generic basename nor the full path leaked into the label. The GUI model selector
  displayed `Ali Mo · Claude Opus 4 6`. Before send, the textarea did not contain
  `GENERIC_WORKTREE_LABEL_FINAL_OK_0605` and the send button was enabled. Page
  polling found `GENERIC_WORKTREE_LABEL_FINAL_OK_0605` on the first check after
  send. Exported trace had `event_count=11`, with events `SessionInitialized`,
  `ModelProfileSwitched`, `UserMessageAdded`, `ContextAssembled`,
  `AgentTaskCreated`, `AgentTaskStarted`, three `ModelTokenDelta`,
  `AssistantMessageCompleted`, and `AgentTaskCompleted`. The trace summary showed
  `user_prompt_has_full_marker=false`, `user_prompt_has_split_marker=true`, and
  `assistant_has_full_marker=true`. `tauri-pilot logs --level error` reported
  `No logs captured`.
- Result: Pass for generic worktree basename display. A non-main project session
  whose `worktree_path` ends in `worktree` renders a stable branch-derived
  worktree name plus branch, instead of showing `worktree · <branch>` or leaking
  the absolute path.

### 2026-06-05 07:21 CST — GUI composer textarea autosize before and after send (#825)

- Commit: `d0d548ea`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, started the real GUI with pilot, selected
  `ali-mo-claude`, opened a fresh workbench session, measured the empty composer,
  filled a 26-line prompt through the GUI textarea, measured the expanded composer,
  clicked the GUI send button, measured the cleared composer, then verified the
  live model response. The prompt split the expected marker into
  `AUTOSIZE_FINAL_` and `OK_0605` so the full `AUTOSIZE_FINAL_OK_0605` string did
  not exist before the model response.
- Method: `HOME=/tmp/kairox-composer-autosize-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-composer-autosize-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
KAIROX_DEV_PORT=1434 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features
pilot` compiled the app and printed the isolated pilot socket. On this machine,
  `cargo run` rewrote the debug binary with a linker-only ad-hoc signature and
  macOS AMFI killed `target/debug/agent-gui-tauri` before setup, so verification
  continued by running `KAIROX_DEV_PORT=1434 KAIROX_DEV_STRICT_PORT=1 bun run
  dev`, then `codesign --force --sign - target/debug/agent-gui-tauri`, then
  `./target/debug/agent-gui-tauri` under the same temp environment. The temp
  profile used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`; after one launcher
  attempt missed that env because the user profile stores a direct `api_key`, the
  app was restarted with the env populated from that profile and a fresh session
  was used for the pass evidence.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/kairox-composer-autosize-runtime-0605/tauri-pilot-dev.kairox.agent.dev1434.sock`,
  and `windows` showed
  `http://localhost:1434/#/workbench/ses_d471bcfd2ee5442492515c97364875f5`.
  The GUI model selector displayed `Ali Mo · Claude Opus 4 6`. Empty composer
  measurement was `valueLength=0`, `styleHeight=32px`, `clientHeight=32`,
  `scrollHeight=32`, `overflowY=hidden`, `computedResize=none`, and
  `sendDisabled=true`. After filling 26 lines, measurement was
  `valueLength=1014`, `styleHeight=160px`, `clientHeight=158`,
  `scrollHeight=480`, `overflowY=auto`, `computedResize=none`,
  `sendDisabled=false`, and `containsFullMarker=false`. Immediately after
  clicking send, the composer measured `valueLength=0`, `styleHeight=32px`,
  `clientHeight=32`, `scrollHeight=32`, `overflowY=hidden`, and
  `sendDisabled=true`. Page polling found `AUTOSIZE_FINAL_OK_0605` on the first
  check after send. Exported trace for session
  `ses_d471bcfd2ee5442492515c97364875f5` had `event_count=9`, with events
  `SessionInitialized`, `UserMessageAdded`, `ContextAssembled`,
  `AgentTaskCreated`, `AgentTaskStarted`, two `ModelTokenDelta`,
  `AssistantMessageCompleted`, and `AgentTaskCompleted`. The trace summary
  showed `user_prompt_has_full_marker=false`,
  `user_prompt_has_split_marker=true`, and `assistant_has_full_marker=true`.
  `tauri-pilot logs --level error` reported `No logs captured`.
- Result: Pass for the real GUI composer autosize path. The textarea expands to
  the configured 160px cap with internal scrolling for long input, stays
  non-resizable by the browser, and shrinks back to the single-line height after
  send while the live Ali Mo turn completes normally.

### 2026-06-05 07:03 CST — GUI composer textarea `change` flush before send (#826)

- Commit: `abe74b86`
- Model: `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`)
- Scenario: In an isolated temp `HOME`, started the real GUI with pilot, selected
  `ali-mo-claude`, seeded the composer with a stale prompt via the textarea `input`
  path until the send button was enabled, then in the same WebView turn replaced
  the DOM textarea value with a final prompt, dispatched `change`, and immediately
  clicked the send button. The stale prompt asked for `STALE_COMPOSER_FLUSH_BAD_0605`;
  the final prompt split the expected marker into `COMPOSER_FLUSH_FINAL_` and
  `OK_0605` so the full `COMPOSER_FLUSH_FINAL_OK_0605` string did not exist before
  the model response.
- Method: `HOME=/tmp/kairox-composer-flush-home-0605
XDG_RUNTIME_DIR=/tmp/kairox-composer-flush-runtime-0605
CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup
KAIROX_DEV_PORT=1433 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features
pilot` compiled the app and printed the isolated pilot socket. On this machine,
  `cargo run` rewrote the debug binary with a linker-only ad-hoc signature and
  macOS AMFI killed `target/debug/agent-gui-tauri` before setup with
  `has no CMS blob?` / `Unrecoverable CT signature issue`; verification continued
  by running `KAIROX_DEV_PORT=1433 KAIROX_DEV_STRICT_PORT=1 bun run dev`, then
  `codesign --force --sign - target/debug/agent-gui-tauri`, then
  `./target/debug/agent-gui-tauri` under the same temp environment.
- Evidence: `tauri-pilot ping` returned ok through
  `/tmp/kairox-composer-flush-runtime-0605/tauri-pilot-dev.kairox.agent.dev1433.sock`,
  and `windows` showed `http://localhost:1433/#/workbench`. The GUI model selector
  changed from `Fake · Fake` to `Ali Mo · Claude Opus 4 6`. Before the final click,
  the textarea held `STALE_COMPOSER_FLUSH_BAD_0605` and the send button was enabled;
  the final synchronous `change` plus click reported `disabledBeforeClick=false`.
  The rendered message list contained the final user prompt and assistant marker
  `COMPOSER_FLUSH_FINAL_OK_0605`. `list_sessions` returned session
  `ses_332108acde064111a4a78c44fecc745f` with profile `ali-mo-claude` and a title
  derived from the final prompt, not the stale prompt. Exported trace had
  `event_count=11`, with events `SessionInitialized`, `ModelProfileSwitched`,
  `UserMessageAdded`, `ContextAssembled`, `AgentTaskCreated`, `AgentTaskStarted`,
  three `ModelTokenDelta`, `AssistantMessageCompleted`, and `AgentTaskCompleted`.
  The `UserMessageAdded` payload was the final prompt, `ContextAssembled` was
  `1948 / 181616` tokens, `AssistantMessageCompleted` content was
  `COMPOSER_FLUSH_FINAL_OK_0605`, and
  `rg STALE_COMPOSER_FLUSH_BAD_0605 /tmp/composer-flush-trace-0605.json` found no
  match. `tauri-pilot logs --level error` reported `No logs captured`.
- Result: Pass for the real GUI composer path. A textarea `change` immediately
  before send updates the store before `send_message`, so the backend/model receive
  the latest text instead of the stale draft.

### 2026-06-07 23:40 CST — Full GUI feature validation sweep (post-#907 through #930)

- Commit: `678e1619`
- Model: `gpt-5-4` (`ali-idealab` / `gpt-5.4-0305-global`)
- Scenario: In the user's real `HOME` with the shared Kairox data dir, started
  the real GUI from a fresh `test/live-validation-0607` worktree with pilot on
  port `1451`, and exercised: (1) basic workspace session lifecycle with gpt-5-4,
  (2) trajectory recording and viewer for no-tool and tool-calling turns,
  (3) autonomous task settings page empty state, (4) software update settings
  check-now button and persistence, (5) model connectivity test and i18n (#930),
  (6) model router refresh on config reload (#929), (7) live gpt-5-4 tool calls
  (`fs.read` + `search.ripgrep`) in a project session, (8) agents settings page
  with builtin agent list, (9) MCP settings page with installed servers, and
  (10) skills settings page with builtin and user skills.
- Method: `KAIROX_DEV_PORT=1451 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev
--features pilot` from `apps/agent-gui` in the worktree. Binary was ad-hoc
  codesigned after initial AMFI kill. Pilot interaction via `tauri-pilot` CLI.
  gpt-5-4 profile used `ali-idealab` provider with existing user config key.
- Evidence:
  - **Workspace session**: `tauri-pilot ping` ok; new session
    `ses_9d0a30498cc64272a503e9739e85fab3` created with gpt-5-4; assistant replied
    `WORKSPACE_SESSION_FINAL_OK_0607`; trace had `event_count=17` including
    `SessionInitialized`, `ContextAssembled` at `1510 / 200000`, `TrajectoryStarted`,
    `AssistantMessageCompleted`, and `TrajectoryCompleted`.
  - **Trajectory viewer**: Right sidebar "轨迹" tab displayed trajectory with
    `success`, `0 步`, duration `22:58:12 - 22:58:13`, and Export button. After
    tool-calling turn, showed `2 步` correctly.
  - **Autonomous settings**: Settings → 自主任务 rendered "暂无自主任务。";
    `list_autonomous_tasks` IPC returned `[]`.
  - **Software update settings**: Settings → 通用 showed version `v0.37.0`,
    "已是最新版本", auto-check and auto-download toggles. "立即检查" button
    updated timestamp from `23:25:53` to `23:27:05`.
  - **Model connectivity (#930)**: `test_model_connectivity(alias="gpt-5-4")`
    returned `ok=true`, `status="chat_ready"`, `response_preview="OK"`.
  - **Router refresh (#929)**: `refresh_config` IPC succeeded;
    `list_profiles_with_limits` still showed gpt-5-4 with correct metadata.
  - **Tool calls**: Project session `ses_03f31d4ee7c44238b4cb94d0adf1a143` with
    gpt-5-4 on temp project `/tmp/kairox-tool-test-0607`; model called `fs.read`
    and `search.ripgrep`; trace `event_count=34` with 2×`ModelToolCallRequested`,
    2×`ToolInvocationCompleted`, 2×`TrajectoryStepRecorded`, `TrajectoryCompleted`
    with `steps=2`, and `AssistantMessageCompleted` content
    `TOOL_CALL_FINAL_OK_0607`.
  - **Agents settings**: Showed builtin agents `code-reviewer`, `default`,
    `explorer`, `test-runner`, `worker` with model/tools/path metadata.
  - **MCP settings**: Showed 5 user MCP servers (fetch, filesystem, git,
    playwright, sqlite) with status, trust, tool counts, and action buttons.
  - **Skills settings**: Showed builtin `skill-creator` and user `weather` skill
    with activation mode, source, path, and permission metadata.
  - **Advisor**: `cargo test -p agent-core advisor` passed 5 tests;
    `cargo test -p agent-runtime advisor` passed 21 tests.
  - `tauri-pilot logs --level error` reported `No logs captured` throughout.
- Result: Pass for all tested areas. The Kairox GUI at commit `678e1619`
  (post-#930) handles workspace sessions, project sessions with live tool calls,
  trajectory recording/viewing, all settings pages (general, MCP, skills, agents,
  autonomous), model connectivity, config refresh, and software updates without
  JS errors or runtime failures when using the gpt-5-4 real model.

### 2026-06-08 00:00 CST — Early feature validation: model switch, fs.write, settings pages, sidebar

- Commit: `678e1619`
- Model: `gpt-5-4` (`ali-idealab` / `gpt-5.4-0305-global`) and `fake`
- Scenario: Continued testing in the same `KAIROX_DEV_PORT=1451` GUI instance
  from the user's real `HOME`, exercising earlier-era features that were not
  covered by the 2026-06-05 validation log: (1) model switching mid-session
  (fake → gpt-5-4), (2) `fs.write` tool execution in a project session with
  on_request approval, (3) hooks/instructions/plugins/archive settings pages,
  (4) session search/filter in sidebar, (5) right sidebar trace/tasks/memory
  tabs with live data.
- Method: Same `KAIROX_DEV_PORT=1451 KAIROX_DEV_STRICT_PORT=1 bun run tauri --
dev --features pilot` from `apps/agent-gui`. Pilot interaction via
  `tauri-pilot` CLI.
- Evidence:
  - **Model switch mid-session**: New workspace session
    `ses_67bc437da35745d9932bc7611830f1c6`; turn 1 with fake returned
    `Hello from the Kairox fake provider!`; switched to gpt-5-4 via GUI selector;
    turn 2 returned `MODEL_SWITCH_TURN2_GPT54_0607`; trace `event_count=30` with
    `ModelProfileSwitched` event `from=fake to=gpt-5-4`.
  - **fs.write tool**: In project session `ses_03f31d4ee7c44238b4cb94d0adf1a143`
    (workspace_write sandbox, on_request approval), gpt-5-4 called `fs.write`;
    `PermissionGranted` auto-granted; file
    `/tmp/kairox-tool-test-0607/write-test-0607.txt` created with exact content
    `FS_WRITE_SENTINEL_0607`; assistant confirmed `FS_WRITE_FINAL_OK_0607`;
    session total `event_count=57`.
  - **Hooks settings**: Rendered 3 template shortcuts (Stop validation, Prompt
    secret scan, Pre-tool policy), project hook count, and empty state with
    "添加钩子" button.
  - **Instructions settings**: Showed User/Project tabs, editable textarea, Save
    button, and "生效指令" effective preview section.
  - **Plugins settings**: Showed 2 user plugins (`commit-commands`,
    `quality-review`) with publisher, signature, skill/MCP counts, paths, and
    enable/disable/delete controls. Marketplace tab visible.
  - **Archive settings**: Showed 1 archived session with "恢复" and "永久删除"
    buttons.
  - **Session search**: Typing "TOOL_CALL" in sidebar search box filtered to 1
    matching session; clearing restored full list.
  - **Trace tab**: Displayed `全部 6 / 活跃 0 / 失败 0 / 完成 6`, detail level
    selectors L1/L2/L3, and event entries for context, task, fs.read (0.0s),
    search.ripgrep (0.2s).
  - **Tasks tab**: Showed DAG tree `All 3 / Active 0 / Failed 0 / Done 3` with
    parent task (P) and 2 worker subtasks (W) for fs.read and search.ripgrep.
  - **Memory tab**: Rendered refresh button and "暂无记忆" empty state.
  - `tauri-pilot logs --level error` reported `No logs captured` throughout.
- Result: Pass for all tested early features. Model switching mid-session
  correctly records `ModelProfileSwitched`, `fs.write` auto-grants under
  workspace_write sandbox, all settings pages render without errors, sidebar
  search filters correctly, and right sidebar tabs display live trace/tasks data.

| Date (CST)       | Commit     | Area                                                            | Scenario                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Model                                                                             | Method                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Result                                                                                                                                                                                                               |
| ---------------- | ---------- | --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-06-05 06:36 | `8ed0ac17` | GUI builtin `skill-creator` discovery, settings, and context    | In an isolated temp `HOME`, started the real GUI with pilot, verified the builtin `skill-creator` skill is discovered from the runtime-provisioned builtin-skills root, opened Settings -> Skills in the GUI to confirm the builtin row is visible and protected, added a temp git project, created a project session from the sidebar, selected `ali-mo-claude`, sent a no-tool ping to materialize the project session, activated `skill-creator`, then sent a second no-tool prompt asking the model to produce a complete `SKILL.md` draft. The final marker was split in the prompt so the full `BUILTIN_SKILL_CREATOR_FINAL_OK_0605` string did not exist before send.                                                                                | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`) | `HOME=/tmp/kairox-builtin-skill-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-builtin-skill-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1432 KAIROX_DEV_STRICT_PORT=1 bun run tauri -- dev --features pilot` from `apps/agent-gui`, with `KAIROX_VALIDATION_ALI_MO_KEY` supplied only as a local process env var. Two stale GUI processes from older worktrees initially caused the new dev1432 app to exit before creating a pilot socket; they were confirmed with `lsof` as running from `.worktrees/fix-worktree-label-0605` and `.worktrees/docs-live-deeplink-0605`, then stopped with normal `TERM` because they blocked the current isolated GUI launch.                                                                                     | `tauri-pilot ping` returned ok through `/tmp/kairox-builtin-skill-runtime-0605/tauri-pilot-dev.kairox.agent.dev1432.sock`, and `windows` showed `http://localhost:1432/#/workbench`. `list_profiles_with_limits` showed `ali-mo-claude` with `has_api_key=true`, `provider="ali-mo"`, `model_id="claude-opus-4-6"`, `context_window=200000`, and `output_limit=16384`. `list_skills` returned `skill-creator` with `source="builtin"`, `activation_mode="suggest"`, `valid=true`, and the expected SKILL.md keywords. `list_skill_settings` returned `settings_id="builtin:skill-creator"`, `scope="builtin"`, `enabled=true`, `effective=true`, `editable=false`, `deletable=false`, and path `/tmp/kairox-builtin-skill-home-0605/.kairox/builtin-skills/skill-creator/SKILL.md`. `get_skill_detail` returned body markdown containing `# Skill Creator`, `## Workflow`, and the minimal `SKILL.md` template. The GUI Skills settings pane displayed `skill-creator`, `Builtin`, `已启用`, `生效中`, `有效`, and disabled `禁用`/`更新`/`删除` controls. The materialized project session was `ses_61845ce07acd4c718a42c801d87b8b58` with profile `ali-mo-claude`, branch `main`, workspace-write sandbox, and worktree path `/tmp/kairox-builtin-skill-project-0605`; the first live turn replied `ALI_MO_BUILTIN_SKILL_PING_OK_0605`. `activate_skill` and `list_active_skills` both returned `skill-creator` with `source="builtin"`. The second live turn rendered a `SKILL.md` draft with `name: builtin-skill-live-0605`, `description`, `version: 0.1.0`, `kairox.activation.mode: suggest`, `# Builtin Skill Live 0605`, `## Workflow`, and final marker `BUILTIN_SKILL_CREATOR_FINAL_OK_0605`. Exported trace had `event_count=62`, including `SkillActivated` for `skill-creator`; the second `ContextAssembled.usage.by_source` included `["skill", 734]`; and the final `AssistantMessageCompleted` contained the generated markdown plus `BUILTIN_SKILL_CREATOR_FINAL_OK_0605`. `tauri-pilot logs --level error` reported `No logs captured`. | Pass for builtin skill provisioning, GUI settings visibility/protection, active-skill state, context injection, and live Ali Mo behavior using the builtin `skill-creator` instructions.                             |
| 2026-06-05 06:16 | `008dd4f2` | GUI trusted MCP server tool execution without approval prompt   | In an isolated temp `HOME`, configured the stdio MCP `echo-fixture` server from `crates/agent-mcp/tests/fixtures/echo-mcp-server.mjs`, refreshed its tool list, opened Settings -> MCP in the real GUI, clicked the `echo-fixture` trust button, added a temp git project, reloaded the workbench so the project sidebar reflected the backend project list, clicked the project's `New session` action, selected `ali-mo-claude`, changed approval to Always, kept Workspace Write sandbox, and sent a live composer turn requiring exactly one call to `mcp.echo-fixture.echo` with `message = "MCP_TRUSTED_TOOL_OK_0605D"`. The final response marker was split in the prompt so the full `MCP_TRUSTED_FINAL_OK_0605D` string did not exist before send. | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`, `client_identity = "claude_code"`) | `HOME=/tmp/kairox-mcp-trusted-home4-0605 XDG_RUNTIME_DIR=/tmp/kairox-mcp-trusted-runtime4-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1431 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui`, plus `./target/debug/agent-gui-tauri` from the worktree with `KAIROX_VALIDATION_ALI_MO_KEY` supplied only as a local process env var. The temp profile used `base_url` from the local user profile, `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`, and `client_identity = "claude_code"`; using `api_base` instead of `base_url` was rejected during setup because it sent requests to the wrong default endpoint. A no-tool GUI ping first returned `ALI_MO_PING_OK_0605`, confirming the live profile was usable before the MCP scenario. | `tauri-pilot ping` returned ok through `/tmp/tauri-pilot-dev.kairox.agent.dev1431.sock`, and `list_profile_settings` showed `ali-mo-claude` with `has_api_key=true` and `client_identity="claude_code"`. `refresh_mcp_tools` for `echo-fixture` returned tools `echo` and `env`. After the GUI trust click, `list_mcp_server_settings` returned `trusted=true`, `runtime_status="running"`, and `verified=true` for `echo-fixture`. The materialized project session was `ses_66ec0d381dfa4e7a9f7a190f4a603227` with profile `ali-mo-claude`, approval policy `always`, branch `main`, workspace-write sandbox, and worktree path `/tmp/kairox-mcp-trusted-project-0605`. Before send, `document.body.innerText.includes("MCP_TRUSTED_FINAL_OK_0605D")` was `false`. The live turn rendered `Tool call: mcp.echo-fixture.echo`, then assistant content `MCP_TRUSTED_FINAL_OK_0605D`; `tauri-pilot assert count '[data-test="permission-prompt"]' 0` and `assert contains '[data-test="chat-panel"]' 'MCP_TRUSTED_FINAL_OK_0605D'` both passed, and `tauri-pilot logs --level error` reported `No logs captured`. Exported trace had `event_count=19`, including `ModelToolCallRequested` for `mcp.echo-fixture.echo`, `PermissionGranted`, `ToolInvocationStarted`, `ToolInvocationCompleted` preview `MCP_TRUSTED_TOOL_OK_0605D`, `AssistantMessageCompleted` content `MCP_TRUSTED_FINAL_OK_0605D`, and no `PermissionRequested` or `PermissionDenied`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Pass for trusted MCP server behavior: under Always approval, a trusted MCP tool was granted and executed without showing a permission prompt, then the live Ali Mo turn completed normally.                          |
| 2026-06-05 05:53 | `1b98cb97` | GUI approval-card denial for live tool call                     | In an isolated temp `HOME`, added a temp git project to the real GUI, refreshed the workbench so the sidebar loaded the project, clicked the project's `New session` action from the sidebar, selected `ali-mo-claude` through the GUI model selector, changed the approval selector from On Request to Always, kept the sandbox at Workspace Write, and sent a live composer turn requiring one `fs.write` to `approval-denied-0605.txt`. When the inline permission card appeared, clicked `Deny` instead of `Allow`.                                                                                                                                                                                                                                     | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-approval-deny-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-approval-deny-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1430 KAIROX_DEV_STRICT_PORT=1 bun --filter agent-gui tauri dev --features pilot` compiled the app with the dev1430 pilot identifier. The wrapper exited after launch, so verification continued with `KAIROX_DEV_PORT=1430 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment. The temp profile config used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`; the env value was supplied only to the local process and was not written to temp files or logs.                                                                   | `tauri-pilot ping` returned ok through `/tmp/tauri-pilot-dev.kairox.agent.dev1430.sock`, `tauri-pilot windows` showed `http://localhost:1430/#/workbench`, and the GUI model trigger rendered `Ali Mo · Claude Opus 4 6` while the approval trigger rendered `总是`. Before send, `/tmp/kairox-approval-deny-project-0605/approval-denied-0605.txt` was absent. The materialized project session was `ses_cf66943a1fb849c0b6c98fbddbde0e69` with profile `ali-mo-claude`, approval policy `always`, branch `main`, workspace-write sandbox, and worktree path `/tmp/kairox-approval-deny-project-0605`; `get_session_git_status` returned `kind: "clean"`. The inline permission card rendered `允许` and `拒绝`; clicking `拒绝` removed the permission prompt. Exported trace had `event_count=13`, with events `SessionInitialized`, `ModelProfileSwitched`, `UserMessageAdded`, `ContextAssembled`, `AgentTaskCreated`, `AgentTaskStarted`, `ModelToolCallRequested`, `PermissionRequested`, `PermissionDenied`, two `ModelTokenDelta` events, `AssistantMessageCompleted`, and `AgentTaskCompleted`. The trace had no `ToolInvocationStarted` or `ToolInvocationCompleted`; `PermissionDenied` reason was `User denied`; `AssistantMessageCompleted` content was `APPROVAL_DENY_FINAL_OK_0605`. `tauri-pilot assert contains '[data-test="chat-panel"]' 'APPROVAL_DENY_FINAL_OK_0605'` and `assert count '[data-test="permission-prompt"]' 0` both passed. The target file remained absent after denial, and `tauri-pilot logs --level error` reported `No logs captured`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Pass for GUI approval-card denial: the write tool was requested but never invoked, the denied write did not touch the filesystem, the prompt cleared, and the live turn completed with the expected assistant reply. |
| 2026-06-05 05:39 | `4da997c9` | GUI live turn cancellation and session recovery                 | In an isolated temp `HOME`, started the real GUI with pilot from a fresh worktree, selected `ali-mo-claude` through the GUI model selector in a normal workspace session, sent a long no-tool prompt asking for 200 marker lines so a live response would stream, waited for the GUI cancel button, clicked Cancel while the turn was active, then sent a short follow-up prompt in the same session to verify the cancelled session could continue.                                                                                                                                                                                                                                                                                                        | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-cancel-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-cancel-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1429 KAIROX_DEV_STRICT_PORT=1 bun --filter agent-gui tauri dev --features pilot` compiled the app with the dev1429 pilot identifier. The wrapper exited after launch, so verification continued with `KAIROX_DEV_PORT=1429 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment. The temp profile config used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`; the env value was supplied only to the local process and was not written to temp files or logs.                                                                                 | `tauri-pilot ping` returned ok through `/tmp/tauri-pilot-dev.kairox.agent.dev1429.sock`, `tauri-pilot windows` showed `http://localhost:1429/#/workbench`, and `lsof` showed PID `16084` running from `/Users/chanyu/AIProjects/kairox/.worktrees/docs-live-cancel-turn-0605/target/debug/agent-gui-tauri`. The GUI model trigger rendered `Ali Mo · Claude Opus 4 6`. After sending the long prompt, `[data-test="cancel-button"]` appeared and clicking it returned the composer to the idle state. The message list contained the user prompt plus `[已取消]`; `tauri-pilot assert contains '[data-test="message-list"]' '[已取消]'` passed. The materialized session was `ses_b9494a6960594387958a332f475038a8` with profile `ali-mo-claude`, approval policy `on_request`, and workspace-write sandbox. Exported trace after the cancel had 8 events, including `ModelProfileSwitched` from `fake` to `ali-mo-claude`, `ContextAssembled` at `1966 / 181616` tokens, `SessionCancelled` with reason `user requested cancellation`, `AgentTaskCreated`, `AgentTaskStarted`, and `TaskCancelled`, with no `AssistantMessageCompleted` for the cancelled turn. The follow-up prompt completed in the same session with assistant content `CANCEL_FOLLOWUP_OK_0605`; the final exported trace had `event_count=16`, including a second `ContextAssembled` at `1959 / 181616` tokens, two `ModelTokenDelta` events, `AssistantMessageCompleted` content `CANCEL_FOLLOWUP_OK_0605`, and `AgentTaskCompleted` for the follow-up task. `tauri-pilot assert contains '[data-test="message-list"]' 'CANCEL_FOLLOWUP_OK_0605'` and `assert count '[data-test="cancel-button"]' 0` both passed. `tauri-pilot logs --level error` reported `No logs captured`. Temp `HOME`, runtime socket dir, and project fixture dirs were removed, and port `1429` had no listener after shutdown.                                                                                                                                                                               | Pass for GUI cancel button behavior, runtime cancellation trace persistence, no assistant completion for the cancelled turn, and same-session recovery on a later live-model turn.                                   |
| 2026-06-05 05:24 | `4a06ff23` | GUI read-only sandbox denial for live tool call                 | In an isolated temp `HOME`, added a temp git project to the real GUI, refreshed the workbench so the sidebar loaded the project, clicked the project's `New session` action from the sidebar to create a pending project session through the frontend store, selected `ali-mo-claude` through the GUI model selector, changed the GUI sandbox selector from Workspace Write to Read Only, and sent a live composer turn requiring the model to call `fs.write` for `sandbox-denied-0605.txt`.                                                                                                                                                                                                                                                               | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-sandbox-denial-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-sandbox-denial-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1428 KAIROX_DEV_STRICT_PORT=1 bun --filter agent-gui tauri dev --features pilot` compiled the app with the dev1428 pilot identifier. The wrapper exited after launching, so verification continued with `KAIROX_DEV_PORT=1428 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment. The temp profile config used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`; the env value was supplied only to the local process and was not written to temp files or logs.                                                              | `tauri-pilot ping` returned ok through `/tmp/kairox-sandbox-denial-runtime-0605/tauri-pilot-dev.kairox.agent.dev1428.sock`, `tauri-pilot windows` showed `http://localhost:1428/#/workbench`, and `lsof` showed PID `61880` running from `/Users/chanyu/AIProjects/kairox/.worktrees/docs-live-sandbox-denial-0605/target/debug/agent-gui-tauri`. The GUI project section rendered `kairox-sandbox-denial-project-0605`, the pending project context rendered `项目：kairox-sandbox-denial-project-0605`, the model trigger rendered `Ali Mo · Claude Opus 4 6`, and the sandbox trigger rendered `只读`. Before send, `/tmp/kairox-sandbox-denial-project-0605/sandbox-denied-0605.txt` was absent. The materialized project session was `ses_16a0da95c433437aba4b856ea36f1898` with profile `ali-mo-claude`, branch `main`, worktree path `/tmp/kairox-sandbox-denial-project-0605`, and persisted sandbox policy `{"kind":"read_only"}`. Exported trace had `event_count=12`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, one `ContextAssembled` at `2032 / 181616` tokens, `ModelToolCallRequested` for `fs.write`, `PermissionDenied` with reason `read-only sandbox blocks writes`, and `AssistantMessageCompleted` content `SANDBOX_DENIED_FINAL_OK_0605`. GUI message text showed the failed `Tool call: fs.write`; `tauri-pilot assert count '[data-test="permission-prompt"]' 0` and `assert count '[data-test="chat-permission-item"]' 0` both passed, confirming no approval card was shown for a sandbox-denied write. The target file remained absent after denial, `tauri-pilot logs --level error` reported `No logs captured`, temp `HOME`/runtime/project dirs were removed, and port `1428` had no listener after shutdown.                                                                                                                                                                                                                                                                                           | Pass for GUI sandbox selector state, pending project session materialization with read-only sandbox, live model tool-call denial before approval, and filesystem non-write enforcement.                              |
| 2026-06-05 05:05 | `20965c2f` | GUI model settings live connectivity test                       | In an isolated temp `HOME`, opened Settings -> Models in the real GUI, verified the `ali-mo-claude` profile row rendered as a user config with `ali-mo / claude-opus-4-6`, enabled state, `context_window = 200_000`, `output_limit = 16_384`, and Claude Code identity, then clicked the row's `Test chat` button to exercise the settings connectivity path against the live provider.                                                                                                                                                                                                                                                                                                                                                                    | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-model-connectivity-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-model-connectivity-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1427 KAIROX_DEV_STRICT_PORT=1 bun --filter agent-gui tauri dev --features pilot` compiled the app with pilot enabled. The wrapper exited after launching, so verification continued with `KAIROX_DEV_PORT=1427 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment. The temp profile config used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`; the env value was supplied only to the local process and was not written to temp files or logs.                                                                     | `tauri-pilot ping` returned ok through `/tmp/kairox-model-connectivity-runtime-0605/tauri-pilot-dev.kairox.agent.dev1427.sock`, `tauri-pilot windows` showed `http://localhost:1427/#/workbench`, and `lsof` showed PID `2129` running from `/Users/chanyu/AIProjects/kairox/.worktrees/docs-live-model-connectivity-0605/target/debug/agent-gui-tauri`. `get_profile_info` showed `ali-mo-claude` with `has_api_key: true`. The Settings -> Models row text for `model-row-ali-mo-claude` was `ali-mo-claude用户配置已启用上下文窗口: 200,000输出上限: 16,384Claude Code 身份ali-mo / claude-opus-4-6 ▲  ▼ 编辑禁用测试对话`. Clicking `model-test-ali-mo-claude` produced a success toast `Model ali-mo-claude is ready to chat.` A structured `test_model_connectivity(alias="ali-mo-claude", projectRoot=null)` IPC call returned `ok: true`, `status: "chat_ready"`, `message: "Model ali-mo-claude is ready to chat."`, and `response_preview: "OK"`. `tauri-pilot logs --level error` reported `No logs captured`. The temp config contained `api_key_env` only, with no direct `api_key`; temp `HOME` and runtime socket dir were removed after shutdown, and port `1427` had no listener.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | Pass for GUI model settings profile visibility, live connectivity test button behavior, and Tauri `test_model_connectivity` command reaching a real chat-ready provider.                                             |
| 2026-06-05 04:52 | `33be8b51` | GUI user settings instructions and runtime prompt injection     | In an isolated temp `HOME`, opened Settings -> Instructions in the real GUI, kept the settings source on User, saved a user-scoped instruction telling the model to answer trigger `SETTINGS_USER_INSTRUCTION_CODE_0605` with `SETTINGS_USER_INSTRUCTION_OK_0605 JADE_0605`, returned to Workbench, selected `ali-mo-claude` through the GUI model selector, and sent a composer prompt containing only the trigger code.                                                                                                                                                                                                                                                                                                                                   | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-user-instructions-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-user-instructions-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1426 KAIROX_DEV_STRICT_PORT=1 bun --filter agent-gui tauri dev --features pilot` compiled the app with pilot enabled. The wrapper exited after launching, so verification continued with `KAIROX_DEV_PORT=1426 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment. The temp profile config used `api_key_env = "KAIROX_VALIDATION_ALI_MO_KEY"`; the env value was supplied only to the local process and was not written to temp files or logs.                                                                       | `tauri-pilot ping` returned ok through `/tmp/kairox-user-instructions-runtime-0605/tauri-pilot-dev.kairox.agent.dev1426.sock`, held by the current worktree's `agent-gui-tauri` process, and `tauri-pilot windows` showed `http://localhost:1426/#/workbench`. `get_profile_info` showed `ali-mo-claude` with `has_api_key: true`. After clicking Save in the GUI, `/tmp/kairox-user-instructions-home-0605/.kairox/config.toml` contained the user `instructions` key and `api_key_env` only, with no `api_key` entry; the GUI effective preview included the saved user instruction, and `get_instructions(scope=User)` returned the same user instruction. The materialized workspace session was `ses_d481161c0c7e4c24864d02c93a5271b3` with profile `ali-mo-claude`. The user prompt was exactly `SETTINGS_USER_INSTRUCTION_CODE_0605`; the assistant completed with `SETTINGS_USER_INSTRUCTION_OK_0605 JADE_0605`. Exported trace had `event_count=11`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, one `ContextAssembled` at `1910 / 181616` tokens with `usage.by_source` `[["system", 219], ["tool_definitions", 1668], ["request", 11], ["history", 12]]`, and `AssistantMessageCompleted` content `SETTINGS_USER_INSTRUCTION_OK_0605 JADE_0605`. `tauri-pilot logs --level error` reported `No logs captured`. Temp `HOME`, runtime socket dir, and trace file were removed, and port `1426` had no listener after shutdown.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Pass for GUI User instructions save, user config reload, runtime system prompt injection from settings config, and real-model instruction following.                                                                 |
| 2026-06-05 04:31 | `30c77d9f` | GUI project settings instructions and runtime prompt injection  | In an isolated temp `HOME`, added a temp git project with no root `AGENTS.md`/`README.md` project-instruction files, opened Settings -> Instructions in the real GUI, switched the settings source from User to Project, selected the temp project, saved a project-scoped instruction telling the model to answer trigger `SETTINGS_PROJECT_INSTRUCTION_CODE_0605` with `SETTINGS_PROJECT_INSTRUCTION_OK_0605 LANTERN_0605`, returned to Workbench, created a project-bound session from the sidebar, selected `ali-mo-claude` through the GUI model selector, and sent a composer prompt containing only the trigger code.                                                                                                                                | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-settings-instructions-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-settings-instructions-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup KAIROX_DEV_PORT=1425 bun --filter agent-gui tauri dev --features pilot` compiled the app with pilot enabled. The wrapper exited after launching, so verification continued with `KAIROX_DEV_PORT=1425 KAIROX_DEV_STRICT_PORT=1 bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment.                                                                                                                                                                                                                                                                     | `tauri-pilot ping` returned ok through `/tmp/kairox-settings-instructions-runtime-0605/tauri-pilot-dev.kairox.agent.dev1425.sock`, held by the current worktree's `agent-gui-tauri` process, and `tauri-pilot windows` showed `http://localhost:1425/#/workbench`. The Settings GUI Project source showed project `prj_d718fc6326e74b95ba52173fcf1d0be4`; after clicking Save, `/tmp/kairox-settings-instructions-project-0605/.kairox/config.toml` contained only the project `instructions` key, the GUI effective preview included the saved project instruction, and `get_instructions(scope=Project, projectRoot=/tmp/kairox-settings-instructions-project-0605)` returned the same project instruction. The materialized project session was `ses_7befb5c93c5948bba813d90052d0d263`, profile `ali-mo-claude`, branch `main`, and worktree path `/tmp/kairox-settings-instructions-project-0605`. The user prompt was exactly `SETTINGS_PROJECT_INSTRUCTION_CODE_0605`; the assistant completed with `SETTINGS_PROJECT_INSTRUCTION_OK_0605 LANTERN_0605`. Exported trace had `event_count=11`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, one `ContextAssembled` at `1920 / 181616` tokens with `usage.by_source` `[["system", 229], ["tool_definitions", 1668], ["request", 11], ["history", 12]]`, and `AssistantMessageCompleted` content `SETTINGS_PROJECT_INSTRUCTION_OK_0605 LANTERN_0605`. `tauri-pilot logs --level error` reported `No logs captured`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Pass for GUI Project instructions save, project config reload, runtime system prompt injection from settings config, and real-model instruction following.                                                           |
| 2026-06-05 04:13 | `0a9d0ffb` | GUI project instructions and runtime prompt injection           | In an isolated temp `HOME`, created a temp git project with root-level `AGENTS.md` instructing the model to answer `PROJECT_INSTRUCTION_CODE_0605` with `PROJECT_INSTRUCTION_OK_0605 CITRON_0605`, then added the project through Tauri IPC, reloaded the GUI, entered a project draft session from the sidebar, selected `ali-mo-claude` through the GUI model selector, and sent a composer prompt containing only the trigger code, not the expected answer.                                                                                                                                                                                                                                                                                             | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-project-instructions-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-project-instructions-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot` compiled the app; as in other local runs, the wrapper exited after launching, so verification continued with `bun run dev` from `apps/agent-gui` plus `./target/debug/agent-gui-tauri` under the same temp environment.                                                                                                                                                                                                                                                                                                                                     | `tauri-pilot ping` returned ok through `/tmp/tauri-pilot-dev.kairox.agent.dev1420.sock` held by the current worktree's `agent-gui-tauri` process. `get_profile_info` showed `ali-mo-claude` with credentials available. `get_project_instruction_summary` for `prj_ccc18cf61e384359a6fce76f662ff784` returned source paths `/tmp/kairox-project-instructions-project-0605/AGENTS.md` and `/tmp/kairox-project-instructions-project-0605/README.md` with `warning: null`, and the GUI empty/project session surface rendered `Loaded AGENTS.md, README.md`. The GUI model selector displayed `Ali Mo · Claude Opus 4 6`, the composer git metadata showed `main`, and the materialized session was `ses_70d87fa0361847fc880d64f492a2ae1c`. The user prompt was `Do not call tools. Respond to PROJECT_INSTRUCTION_CODE_0605.`; the assistant completed with `PROJECT_INSTRUCTION_OK_0605 CITRON_0605`. Exported trace had `event_count=11`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, one `ContextAssembled` at `1989 / 181616` tokens, and `usage.by_source` included `["project_instruction", 90]`. `tauri-pilot logs --level error` reported `No logs captured`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Pass for project instruction discovery, GUI project instruction summary rendering, runtime prompt injection, real-model instruction following, and `ContextSource::ProjectInstruction` accounting.                   |
| 2026-06-05 03:57 | `66dc3877` | Runtime memory context injection and source accounting          | In an isolated temp `HOME`, added a temp git project, entered a project draft session from the GUI sidebar, selected `ali-mo-claude` through the GUI model selector, and sent a live composer turn asking the model to propose user memory key `validation.memory.context.0605` with content `The live validation codename is ORCHID_0605.` plus marker `MEMORY_CONTEXT_SEED_OK_0605`. After accepting the inline memory prompt through the GUI, sent a second live composer turn that referenced only the memory key and asked the model to recall the codename without tools.                                                                                                                                                                             | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-memory-context-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-memory-context-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot` compiled the app and exposed the isolated pilot socket. As in other local runs, the wrapper exited after launching the binary, so verification continued with a separately running Vite dev server plus `./target/debug/agent-gui-tauri` under the same temp environment.                                                                                                                                                                                                                                                                                               | `tauri-pilot ping` returned ok through `/tmp/kairox-memory-context-runtime-0605/tauri-pilot-dev.kairox.agent.dev1420.sock`. `get_profile_info` showed `ali-mo-claude` with credentials available, and the GUI model selector displayed `Ali Mo · Claude Opus 4 6`. Before approval, `query_memories` returned one `accepted=false` user memory with key `validation.memory.context.0605` and content `The live validation codename is ORCHID_0605.`; after clicking `Accept`, the same memory returned `accepted=true`. The second user prompt did not contain `ORCHID_0605`, but the assistant replied `MEMORY_CONTEXT_RECALL_OK_0605 ORCHID_0605`. Exported trace for `ses_fed0aa5662dd4d868218a91e914bef47` had `event_count=25`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, one `MemoryProposed`, one `MemoryAccepted`, and two `ContextAssembled` events. The first context was `2017 / 181616` tokens with no memory source; the second was `3104 / 181616` tokens and `usage.by_source` included `["memory", 23]`. `AssistantMessageCompleted` contents were `MEMORY_CONTEXT_SEED_OK_0605` and `MEMORY_CONTEXT_RECALL_OK_0605 ORCHID_0605`; `tauri-pilot logs --level error` reported `No logs captured`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Pass for accepted memory recall in a later live-model turn and `ContextSource::Memory` accounting in runtime trace.                                                                                                  |
| 2026-06-05 03:43 | `a94ee834` | GUI project session deep links and restored chat continuity     | In an isolated temp `HOME`, added a temp git project, entered a project session through the GUI sidebar, selected `ali-mo-claude` through the GUI model selector, and sent a first live composer turn requiring the exact marker `DEEPLINK_TURN_1_OK_0605`. Then cleared WebView `localStorage`, reloaded directly to `#/workbench/ses_f8de16b5150e484aa1bcd69e48c3920d`, and verified the project session route restored rather than redirecting away. After the deep-link reload, sent a second live composer turn requiring `DEEPLINK_TURN_2_OK_0605` to confirm the restored project session could continue using the same live model.                                                                                                                  | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-deeplink-home-0605 XDG_RUNTIME_DIR=/tmp/kairox-deeplink-runtime-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot` initially compiled the app, then the verification used a separately running Vite dev server plus the debug Tauri binary with the same temp `HOME` and isolated pilot socket because the `tauri dev` wrapper exited after launching the binary in this environment.                                                                                                                                                                                                                                                                                                                  | `tauri-pilot ping` returned ok through the isolated socket `/tmp/kairox-deeplink-runtime-0605/tauri-pilot-dev.kairox.agent.dev1420.sock`. The GUI model selector displayed `Ali Mo · Claude Opus 4 6`. The first materialized project session URL was `/workbench/ses_f8de16b5150e484aa1bcd69e48c3920d`; before reload, the project sidebar contained `kairox-deeplink-project-0605` and the project session title `Direct project session deep link validation turn…`. After `localStorage.clear()` plus reload to `#/workbench/ses_f8de16b5150e484aa1bcd69e48c3920d`, `tauri-pilot assert url '/workbench/ses_f8de16b5150e484aa1bcd69e48c3920d'` passed, `[data-test="project-session-btn"]` was visible, the project sidebar still showed the restored project session, and `[data-test="message-list"]` still contained the turn-1 history and assistant marker. After the reload, the model trigger still displayed `Ali Mo · Claude Opus 4 6`; the second turn completed with `DEEPLINK_TURN_2_OK_0605`, and the URL stayed on the same session id. Exported trace had `event_count=18`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, two `ContextAssembled` events (`1952 / 181616` and `3006 / 181616` tokens), and two `AssistantMessageCompleted` marker messages. `tauri-pilot logs --level error` reported `No logs captured`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Pass for direct project-session route restoration, project sidebar/session history recovery, and continued live-model chat.                                                                                          |
| 2026-06-05 03:32 | `754ec6dc` | GUI user-requested context compaction and completed chat item   | In an isolated temp `HOME`, added a temp git project, entered a project session through the GUI sidebar, selected `ali-mo-claude` through the GUI model selector, and sent four GUI composer turns requiring exact marker replies `COMPACTION_TURN_{1..4}_OK_0605`. The compact button was intentionally not used as pass evidence because the small live fixture stayed far below the 30% compression-ratio threshold; instead, `compact_session` was invoked through Tauri IPC for the active GUI session, then the GUI chat stream was checked for the completed compaction item.                                                                                                                                                                        | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-compaction-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot` initially compiled the app, then the verification used a separately running Vite dev server plus the debug Tauri binary with `tauri-pilot` because the `tauri dev` wrapper exited after launching the binary in this environment.                                                                                                                                                                                                                                                                                                                                                                                                   | `tauri-pilot ping` returned ok and the window URL was `/workbench/ses_2b141d81321b40988afd826d1a6ba779`. `get_profile_info` showed `ali-mo-claude` with credentials available, and the GUI model selector displayed `Ali Mo · Claude Opus 4 6`. The four assistant messages completed with `COMPACTION_TURN_1_OK_0605`, `COMPACTION_TURN_2_OK_0605`, `COMPACTION_TURN_3_OK_0605`, and `COMPACTION_TURN_4_OK_0605`; the corresponding context assemblies were `1948 / 181616`, `2994 / 181616`, `2527 / 181616`, and `2702 / 181616` tokens. After `tauri-pilot ipc compact_session`, `tauri-pilot wait --selector '[data-test="chat-compaction-item"][data-status="completed"]' --timeout 120000` found the completed item and `tauri-pilot text '[data-test="chat-compaction-item"]'` returned `上下文已压缩`. Exported trace for `ses_2b141d81321b40988afd826d1a6ba779` had `event_count=38`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, four `AssistantMessageCompleted` marker messages, `ContextCompactionStarted` with `reason: UserRequested` and `candidate_event_count: 10`, `CompactionSummary` with `summarised_by_profile: "ali-mo-claude"` and non-empty summary content, and `ContextCompactionCompleted` with `fallback_used: false` and `after_tokens: 117`. `tauri-pilot logs --level error` reported `No logs captured`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Pass for live model compaction execution, trace persistence, and GUI completed compaction rendering.                                                                                                                 |
| 2026-06-05 02:59 | `2b87575b` | Monitor lifecycle tools, GUI/Tauri trace export persistence     | In an isolated temp `HOME`, added a temp project through Tauri GUI IPC, created a project draft session, switched the session from the default `fake` profile to `ali-mo-claude`, and asked the live model to call `monitor.start`, `monitor.list`, and `monitor.stop` in order. The first live run before the fix completed all three tools but exported trace lacked monitor lifecycle events; after the fix, reran the same scenario and verified event store export included the monitor lifecycle.                                                                                                                                                                                                                                                     | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-monitor-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot` initially compiled the app, then the verification used a separately running Vite dev server plus the debug Tauri binary with `tauri-pilot ipc` because `tauri-pilot windows/snapshot` triggered a local wry `Option::unwrap()` panic in this environment.                                                                                                                                                                                                                                                                                                                                                                              | Before the fix, the live model returned `MONITOR_FINAL_OK_0605`, `list_monitors` returned `[]`, and exported trace for `ses_a00e86e51db04e21ad581809196c5eb4` had `event_count=58` with `ToolInvocationCompleted` previews `Monitor started: mon_1`, `- mon_1 (monitor validation 0605): persistent=false, timeout=60000ms\n`, and `Monitor stopped: mon_1`, but no `MonitorStarted`, `MonitorEvent`, or `MonitorStopped` events. After the fix, session `ses_2ec95411aab445d1a658245598912a53` exported `event_count=42`, included `ModelProfileSwitched` from `fake` to `ali-mo-claude`, completed `monitor.start`, `monitor.list`, and `monitor.stop`, persisted `MonitorStarted` for `mon_1`, persisted `MonitorEvent` line `MONITOR_READY_OK_0605`, and persisted `MonitorStopped` with `UserStopped`. Assistant messages included `MONITOR_FINAL_OK_0605`; `tauri-pilot ipc list_monitors --json` returned `[]`; `tauri-pilot logs --level error` reported `No logs captured`. Focused checks passed: `cargo test -p agent-tools monitor`; `cargo test -p agent-runtime --test full_stack project_session_monitor_start_uses_project_worktree_root -- --nocapture`; `cargo test -p agent-runtime --test full_stack`; `cargo test -p agent-runtime --all-targets`; `cargo test -p agent-tools --all-targets`; `bun run format:check`; `bun run lint`. Full `cargo test --workspace --all-targets` was attempted twice but the local AMFI policy killed `target/debug/kairox-eval` child processes inside `agent-eval` CLI tests with `has no CMS blob` / `Unrecoverable CT signature issue`; direct `target/debug/kairox-eval list --scenarios crates/agent-eval/fixtures/smoke.jsonl` succeeded.                                                                                                                                                                                                                                                                                                                                                       | Fixed: monitor lifecycle events are now persisted and replayable through trace export, while live tool behavior still cleans up monitors.                                                                            |
| 2026-06-05 02:25 | `ce3b1d86` | GUI project-local skill discovery and active-skill context      | In an isolated temp `HOME`, created a temp git project with `.kairox/skills/project-sentinel-0605/SKILL.md`, registered the project, entered a project session through the sidebar GUI, selected `ali-mo-claude` through the GUI model selector, sent an initial GUI composer message to materialize the project session, verified current-session skill discovery saw the project-local skill as a workspace skill, activated `project-sentinel-0605`, then sent a second GUI composer message asking the model to follow the active skill. Slash palette activation was not used as pass evidence because it did not reliably render under this pilot run.                                                                                                | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-skills-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot`; `tauri-pilot` GUI interaction, app IPC for skill discovery/activation evidence, and IPC trace export                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `tauri-pilot ping` returned ok. After the first GUI send, the materialized project session was `ses_771d89724ccc414fb147791d5ece954c` and the model replied `PROJECT_SESSION_READY_0605`. `list_skills` in that project session returned `project-sentinel-0605` with `source: "workspace"` and `activation_mode: "manual"`; `activate_skill` returned the active skill view and `list_active_skills` returned the same workspace skill. The second GUI composer send produced assistant content `PROJECT_SKILL_FINAL_OK_0605` and `PROJECT_SKILL_CONTEXT_OK_0605`. Exported trace had `event_count=33`, included one `SkillActivated` payload for `project-sentinel-0605`, and had two `ContextAssembled` events; the second `ContextAssembled.usage.by_source` included `["skill", 88]`. `tauri-pilot logs --level error` reported `No logs captured`. Focused frontend checks `cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- ChatComposer.test.ts useChatComposer.test.ts --run` passed with 55 tests. Temp HOME/project/trace were removed and port `1420` had no listener after shutdown.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Pass for project-local skill discovery, activation state, context injection, and live model behavior.                                                                                                                |
| 2026-06-05 02:13 | `c66ced32` | GUI project worktree session creation, git metadata, tool root  | In an isolated temp `HOME`, added a temp git project on `main`, used the sidebar "new session in project" GUI path to start a pending project session, used the branch selector popover to create/select `wt/live-0605`, selected `ali-mo-claude` through the GUI model selector, changed approval to `always`, and asked the model to call `fs.write` once for relative path `worktree-live-0605.txt`. Verified the first send materialized a project worktree session and that tool execution used the worktree root rather than the main project root.                                                                                                                                                                                                   | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-worktree-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot`; `tauri-pilot` GUI interaction and IPC trace export                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `tauri-pilot ping` returned ok. Before approval, `/tmp/kairox-worktree-project-0605/worktree-live-0605.txt` and `/tmp/kairox-worktree-project-0605/.kairox/worktrees/wt-live-0605/worktree-live-0605.txt` were both absent. The pending composer branch selector showed `wt/live-0605`; after first send, the materialized session URL was `/workbench/ses_c831843e5c334147bf1e9a859f4321fb`, composer git metadata showed `wt-live-0605 · wt/live-0605`, and the project sidebar included branch badge `wt/live-0605`. The permission preview was `fs.write({"content":"WORKTREE_WRITE_OK_0605","path":"worktree-live-0605.txt"})`; clicking `Allow` completed the tool. The main project root file remained absent, while `.kairox/worktrees/wt-live-0605/worktree-live-0605.txt` contained exactly `WORKTREE_WRITE_OK_0605`; `git branch --show-current` was `main` in the project root and `wt/live-0605` in the worktree. Exported trace had `event_count=18` with one each of `ModelToolCallRequested`, `PermissionRequested`, `PermissionGranted`, `ToolInvocationStarted`, `ToolInvocationCompleted`, and `AssistantMessageCompleted`; `ToolInvocationCompleted` preview was `Written 22 bytes to worktree-live-0605.txt`; assistant content was `WORKTREE_SESSION_FINAL_OK_0605`. `tauri-pilot logs --level error` reported `No logs captured`. Temp HOME/project/trace were removed and port `1420` had no listener after shutdown.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Pass. No code fix needed.                                                                                                                                                                                            |
| 2026-06-05 02:03 | `429c0348` | GUI image attachment, file mention, multimodal model request    | In an isolated temp `HOME`, generated a temp git project containing `kairox-vision-0605.png` with a red square on the left and a green square on the right, created a project draft session, selected `ali-mo-claude` through the GUI model selector, opened the file mention palette from the composer input, selected `@kairox-vision-0605.png`, and sent a prompt asking the model to identify the colors and positions of the attached image without calling tools.                                                                                                                                                                                                                                                                                     | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-image-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot`; `tauri-pilot` GUI interaction and IPC trace export                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | `tauri-pilot ping` returned ok. The model selector displayed `Ali Mo · Claude Opus 4 6`. The file mention palette contained `@kairox-vision-0605.png`; clicking `[data-test="mention-file-item"]` produced an attachment tray with a chip whose `data-filename` was `kairox-vision-0605.png`. `AssistantMessageCompleted` for session `ses_4705b2ca971d4ee4895cf59c216c7036` contained `IMAGE_ATTACHMENT_FINAL_OK_0605` and identified the red block on the left and the green block on the right. Exported trace had `event_count=37` with one `ContextAssembled`, one `AssistantMessageCompleted`, and `ContextAssembled.usage.by_source` included `["image", 176]`; total context was `2217 / 181616` tokens. `tauri-pilot logs --level error` reported `No logs captured`. Temp HOME/project/trace were removed and port `1420` had no listener after shutdown.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Pass. No code fix needed.                                                                                                                                                                                            |
| 2026-06-05 01:40 | `7bdd34e9` | GUI memory prompt, memory marker parsing, MemoryStore approval  | In an isolated temp `HOME`, added a temp git project, created a project draft session, selected `ali-mo-claude` through the GUI model selector, and asked the model to reply with `<memory scope="user" key="validation.memory.0605">MEMORY_PROMPT_OK_0605</memory>` plus `MEMORY_PROMPT_FINAL_OK_0605`. Verified the inline memory prompt appeared with `Accept`/`Reject`, no tool shortcut hint block was rendered for the memory prompt, the memory was pending before approval, clicking `Accept` removed the prompt, and the Memory tab showed one accepted user memory.                                                                                                                                                                               | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-memory-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot`; `tauri-pilot` GUI interaction, `query_memories`, and IPC trace export                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Initial `query_memories` returned `[]`. `tauri-pilot assert count '[data-test="chat-permission-item-shortcuts"]' 0` passed while the memory prompt was visible. Before approval, `query_memories` returned one `accepted=false` user memory with key `validation.memory.0605` and content `MEMORY_PROMPT_OK_0605`. After clicking `Accept`, `query_memories` returned the same memory with `accepted=true`; MemoryBrowser text contained `已接受 1`, `待处理 0`, `validation.memory.0605`, and `MEMORY_PROMPT_OK_0605`. Exported trace for session `ses_9159219e979b4061976788bc90353c16` had `event_count=18` with one each of `MemoryProposed` and `MemoryAccepted`; `AssistantMessageCompleted` content was `MEMORY_PROMPT_FINAL_OK_0605`; `tauri-pilot logs --level error` reported `No logs captured`. Temp HOME/project were removed and port `1420` had no listener after shutdown.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Pass. No code fix needed.                                                                                                                                                                                            |
| 2026-06-05 01:24 | `ff3087b0` | GUI permission flow, `fs.write`, project session tool execution | In an isolated temp `HOME`, added a temp git project, created a project draft session, selected `ali-mo-claude` through the GUI model selector, changed approval to `always` through the GUI approval selector, then asked the model to call `fs.write` once for `permission-write-0605.txt`. Verified the file was absent before approval, the inline permission card showed `Allow`/`Deny`, clicking `Allow` unblocked execution, the file content became exactly `PERMISSION_WRITE_OK_0605`, and the assistant replied `PERMISSION_WRITE_FINAL_OK_0605`.                                                                                                                                                                                                 | `ali-mo-claude` (`ali-mo` / `claude-opus-4-6`)                                    | `HOME=/tmp/kairox-permission-home-0605 CARGO_HOME=/Users/chanyu/.cargo RUSTUP_HOME=/Users/chanyu/.rustup bun --filter agent-gui tauri dev --features pilot`; `tauri-pilot` GUI interaction and IPC trace export                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `tauri-pilot ping` returned ok. `get_session_approval_policy` returned `"always"`. Exported trace for session `ses_ba83d301bc974b5184a30f41a5d52e57` had `event_count=18` with one each of `ModelToolCallRequested`, `PermissionRequested`, `PermissionGranted`, `ToolInvocationStarted`, and `ToolInvocationCompleted`; the permission preview was `fs.write({"content":"PERMISSION_WRITE_OK_0605","path":"permission-write-0605.txt"})`; `ToolInvocationCompleted` reported `Written 24 bytes to permission-write-0605.txt`; `tauri-pilot logs --level error` reported `No logs captured`. Temp HOME/project were removed and port `1420` had no listener after shutdown.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Pass. No code fix needed.                                                                                                                                                                                            |
| 2026-06-07 22:30 | `617a5ac5` | Full-feature validation: pilot scenarios + live gpt-5-4 chat    | Created test worktree `test/full-validation-0607` from main (v0.37.0), launched `bun --filter agent-gui tauri dev --features pilot`, ran all 18 pilot e2e scenarios, then performed manual `tauri-pilot` verification of new features added since 2026-06-05: Autonomous task settings panel (#907-#912), Trajectory viewer (#901), Hook templates vs user hooks (#880), model settings (#929/#930), permission double-axis selectors, and live gpt-5-4 chat with streaming and tool calls.                                                                                                                                                                                                                                                                 | `gpt-5-4` (`ali-idealab` / `gpt-5.4-0305-global`)                                 | `bun --filter agent-gui tauri dev --features pilot` in worktree; `tauri-pilot` CLI for all scenario runs and manual interaction; `tauri-pilot run apps/agent-gui/e2e-pilot/*.toml --junit`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | **Pilot scenarios (18 total):** 13 passed on first run, 5 failed. Of the 5 failures: 2 were scenario i18n bugs (hardcoded English assertions vs zh-CN locale — `policy-double-axis.toml` "On Request" vs "按需", `audit-agents.toml` "Disabled" vs "已禁用"); 3 were environment fixture issues (audit-mcp: pilot-mcp server not configured; audit-plugins: marketplace fixture absent; chat-live: github-gpt4o-mini profile not in local config). The 2 i18n bugs were fixed by subagent via PR #931 (`fix(gui): make pilot scenario assertions i18n-aware`), merged as `617a5ac5`. After fix, rerun of `policy-double-axis` (19/19) and `audit-agents` (34/34) both passed 100%. **Manual pilot verification:** Autonomous settings pane navigates correctly, shows empty state "暂无自主任务"; Trajectory viewer tab activates, shows empty state for inactive sessions; Hooks settings correctly separates template buttons (Stop validation, Prompt secret scan, Pre-tool policy) from "用户钩子" section with count badge; Model settings shows 6 profiles with full CRUD; Permission double-axis shows "按需"/"工作区写入" correctly in zh-CN. **Live gpt-5-4 chat:** Model selector shows "Ali Idealab · GPT-5.4 0305 Global", context assembly completed (1504/181616 tokens), 2 tool calls succeeded, assistant responded with exact sentinel `KAIROX_LIVE_TEST_OK`. One pre-existing error log: `Failed to send message: invalid state: unknown model: claude-haiku` (from prior session, not current test). `tauri-pilot logs --level error` showed no new errors during test. Test worktree cleaned, port 1420 released.                                                                                                                                                                                                                                                                                                                                                                                                                        | Pass. Fixed 2 scenario i18n bugs via PR #931. 3 environment-dependent scenario failures are expected without specific fixtures.                                                                                      |
