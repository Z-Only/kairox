#!/usr/bin/env bash
# Start a debug Tauri build with the `pilot` feature, run every TOML scenario
# under apps/agent-gui/e2e-pilot/ via tauri-pilot, then tear down.
#
# Pre-condition: the debug binary already exists at the configured path.
# Build it once with:
#   cargo tauri build --debug --no-bundle --features pilot
# (or the equivalent `cargo build -p agent-gui-tauri --features pilot`).
#
# Usage:
#   scripts/run-pilot-tests.sh [BINARY_PATH]
#
# Environment:
#   PILOT_SCENARIO=name
#       Run one scenario from apps/agent-gui/e2e-pilot/.
#   PILOT_SCENARIOS="name-a,name-b"
#       Run a comma/space separated scenario subset.
#   KAIROX_PILOT_LIVE_MODELS=1
#       Configure the app with GitHub Models and, unless an explicit scenario
#       subset is provided, run only the low-request live model scenarios.
#   KAIROX_PILOT_LIST_SCENARIOS=1
#       Print the selected scenario set and exit without launching the app.
#
# Exit code: 0 on all-pass, 1 on first scenario failure (or env error).

set -euo pipefail

# ─── Paths ─────────────────────────────────────────────────────────────────────

# Resolve the repo root from this script's location and operate from there so
# the relative paths in scenario files (e.g. tauri-pilot-failures/*.png) and
# the workspace target/ dir resolve consistently regardless of caller CWD.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

# Cargo workspaces share a single target/ at the workspace root, NOT under
# apps/agent-gui/src-tauri/target/. The default below reflects that.
APP_BIN="${1:-$REPO_ROOT/target/debug/agent-gui-tauri}"
SCENARIO_DIR="$REPO_ROOT/apps/agent-gui/e2e-pilot"
JUNIT_DIR="$REPO_ROOT/pilot-results"
FAILURE_DIR="$REPO_ROOT/tauri-pilot-failures"
LIVE_MODEL_ENABLED="${KAIROX_PILOT_LIVE_MODELS:-0}"
LIVE_MODEL_PROFILE="${KAIROX_PILOT_MODEL_PROFILE:-github-gpt4o-mini}"
LIVE_MODEL_ID="${KAIROX_PILOT_MODEL_ID:-openai/gpt-4o-mini}"
LIVE_MODEL_BASE_URL="${KAIROX_PILOT_MODEL_BASE_URL:-https://models.github.ai/inference}"
LIVE_MODEL_MAX_TOKENS="${KAIROX_PILOT_MODEL_MAX_TOKENS:-64}"
DEFAULT_LIVE_SCENARIOS=(chat-live attachment-live)
scenarios=()

# ─── Fixture registration ──────────────────────────────────────────────────────

# The app resolves db_dir as $HOME/.kairox (see lib.rs line 44).
# We write pilot fixture sources to that config.toml so the app picks them
# up on startup.
FIXTURE_MARKETPLACE_DIR="$REPO_ROOT/apps/agent-gui/e2e-pilot/fixtures/plugin-marketplace"
FIXTURE_MCP_SERVER_SCRIPT="$REPO_ROOT/crates/agent-mcp/tests/fixtures/minimal-mcp-server.mjs"

PILOT_HOME_CREATED=0
if [[ -n "${KAIROX_PILOT_HOME:-}" ]]; then
    PILOT_HOME="$KAIROX_PILOT_HOME"
else
    PILOT_HOME="$(mktemp -d "${TMPDIR:-/tmp}/kairox-pilot-home.XXXXXX")"
    PILOT_HOME_CREATED=1
fi

CONFIG_DIR="$PILOT_HOME/.kairox"
CONFIG_TOML="$CONFIG_DIR/config.toml"
CONFIG_BACKUP="$CONFIG_DIR/config.toml.pilot-backup"

PROJECT_CONFIG_DIR="$REPO_ROOT/.kairox"
PROJECT_CONFIG_TOML="$PROJECT_CONFIG_DIR/config.toml"
PROJECT_CONFIG_BACKUP="$PROJECT_CONFIG_DIR/config.toml.pilot-backup"

_register_pilot_fixtures() {
    mkdir -p "$CONFIG_DIR"
    # Back up the original config so we can restore it after the run.
    if [[ -f "$CONFIG_TOML" ]]; then
        cp "$CONFIG_TOML" "$CONFIG_BACKUP"
    else
        rm -f "$CONFIG_BACKUP" 2>/dev/null || true
    fi
    python3 - "$CONFIG_TOML" "$FIXTURE_MARKETPLACE_DIR" "$FIXTURE_MCP_SERVER_SCRIPT" <<'PYEOF'
import json
import sys

config_path = sys.argv[1]
fixture_dir = sys.argv[2]
fixture_mcp_server = sys.argv[3]

try:
    with open(config_path) as f:
        content = f.read()
except FileNotFoundError:
    content = ""

def append_section(section: str) -> None:
    global content
    if content.rstrip('\n'):
        content = content.rstrip('\n') + '\n\n'
    content += section

if 'id = "pilot-fixture"' not in content:
    # Enable the local pilot-fixture marketplace source.
    append_section('[[plugin_marketplaces]]\n'
                   'id = "pilot-fixture"\n'
                   'display_name = "Pilot Test Fixtures"\n'
                   f'source = "{fixture_dir}"\n'
                   'enabled = true\n')

# Disable the default GitHub marketplace sources so the catalog loads
# instantly from the local fixture only — no network dependency.
if 'id = "claude-plugins-official"' not in content:
    append_section('[[plugin_marketplaces]]\n'
                   'id = "claude-plugins-official"\n'
                   'display_name = "Claude Plugins Official"\n'
                   'source = "anthropics/claude-plugins-official"\n'
                   'enabled = false\n')

if 'id = "anthropics-claude-code"' not in content:
    append_section('[[plugin_marketplaces]]\n'
                   'id = "anthropics-claude-code"\n'
                   'display_name = "Anthropic Claude Code"\n'
                   'source = "anthropics/claude-code"\n'
                   'enabled = false\n')

if "[mcp_servers.pilot-mcp]" not in content:
    append_section('[mcp_servers.pilot-mcp]\n'
                   'type = "stdio"\n'
                   'command = "node"\n'
                   f'args = [{json.dumps(fixture_mcp_server)}]\n'
                   'enabled = true\n'
                   'description = "Pilot MCP fixture server"\n')

with open(config_path, 'w') as f:
    f.write(content)
print(f"Registered pilot fixture marketplace and MCP server in {config_path}")
PYEOF
}

_backup_project_config() {
    if [[ -f "$PROJECT_CONFIG_TOML" ]]; then
        mkdir -p "$PROJECT_CONFIG_DIR"
        cp "$PROJECT_CONFIG_TOML" "$PROJECT_CONFIG_BACKUP"
    else
        rm -f "$PROJECT_CONFIG_BACKUP" 2>/dev/null || true
    fi
}

_restore_project_config() {
    if [[ -f "$PROJECT_CONFIG_BACKUP" ]]; then
        mv "$PROJECT_CONFIG_BACKUP" "$PROJECT_CONFIG_TOML" 2>/dev/null || true
    else
        rm -f "$PROJECT_CONFIG_TOML" 2>/dev/null || true
        rmdir "$PROJECT_CONFIG_DIR" 2>/dev/null || true
    fi
}

_write_pilot_project_config() {
    mkdir -p "$PROJECT_CONFIG_DIR"
    if [[ "$LIVE_MODEL_ENABLED" = "1" ]]; then
        if [[ -z "${GITHUB_TOKEN:-}" ]]; then
            echo "ERROR: KAIROX_PILOT_LIVE_MODELS=1 requires GITHUB_TOKEN for GitHub Models." >&2
            exit 1
        fi
        cat >"$PROJECT_CONFIG_TOML" <<EOF
[profiles.$LIVE_MODEL_PROFILE]
provider = "openai_compatible"
model_id = "$LIVE_MODEL_ID"
base_url = "$LIVE_MODEL_BASE_URL"
api_key_env = "GITHUB_TOKEN"
temperature = 0

[profiles.$LIVE_MODEL_PROFILE.extra_params]
max_tokens = $LIVE_MODEL_MAX_TOKENS

[features]
hooks = true
EOF
        echo "Configured live GitHub Models pilot profile: $LIVE_MODEL_PROFILE ($LIVE_MODEL_ID)"
        return
    fi

    cat >"$PROJECT_CONFIG_TOML" <<'EOF'
[profiles.fake]
provider = "fake"
model_id = "fake"
response = "hello from pilot"

[features]
hooks = true
EOF
}

_scenario_path_for_name() {
    local scenario_name="$1"
    local scenario_path="$SCENARIO_DIR/${scenario_name%.toml}.toml"
    if [[ ! -f "$scenario_path" ]]; then
        echo "ERROR: requested pilot scenario not found: $scenario_path" >&2
        exit 1
    fi
    printf '%s\n' "$scenario_path"
}

_select_scenarios() {
    local explicit_subset=0

    shopt -s nullglob
    scenarios=("$SCENARIO_DIR"/*.toml)
    shopt -u nullglob

    if [[ -n "${PILOT_SCENARIO:-}" && -n "${PILOT_SCENARIOS:-}" ]]; then
        echo "ERROR: set only one of PILOT_SCENARIO or PILOT_SCENARIOS." >&2
        exit 1
    fi

    if [[ -n "${PILOT_SCENARIO:-}" ]]; then
        explicit_subset=1
        scenarios=("$(_scenario_path_for_name "$PILOT_SCENARIO")")
    elif [[ -n "${PILOT_SCENARIOS:-}" ]]; then
        explicit_subset=1
        scenarios=()
        local scenario_name
        for scenario_name in ${PILOT_SCENARIOS//,/ }; do
            [[ -z "$scenario_name" ]] && continue
            scenarios+=("$(_scenario_path_for_name "$scenario_name")")
        done
    elif [[ "$LIVE_MODEL_ENABLED" = "1" ]]; then
        scenarios=()
        local scenario_name
        for scenario_name in "${DEFAULT_LIVE_SCENARIOS[@]}"; do
            scenarios+=("$(_scenario_path_for_name "$scenario_name")")
        done
    fi

    local filtered=()
    local scenario_base
    for sc in "${scenarios[@]}"; do
        scenario_base="$(basename "$sc")"
        if [[ "$LIVE_MODEL_ENABLED" != "1" && "$scenario_base" == *-live.toml ]]; then
            if [[ "$explicit_subset" -eq 1 ]]; then
                echo "ERROR: $scenario_base requires KAIROX_PILOT_LIVE_MODELS=1." >&2
                exit 1
            fi
            echo "SKIP $scenario_base (live model scenario)" >&2
            continue
        fi
        filtered+=("$sc")
    done
    scenarios=("${filtered[@]}")

    # In non-live CI, skip scenarios that send chat turns through the model.
    # The live pilot job runs the bounded *-live.toml scenarios instead.
    if [[ "${CI:-}" = "true" && "$LIVE_MODEL_ENABLED" != "1" ]]; then
        filtered=()
        for sc in "${scenarios[@]}"; do
            case "$(basename "$sc")" in
                audit-chat.toml | chat-flow.toml)
                    echo "SKIP $(basename "$sc") (CI: no LLM backend available)" >&2
                    ;;
                *)
                    filtered+=("$sc")
                    ;;
            esac
        done
        scenarios=("${filtered[@]}")
    fi
}

_write_pilot_browser_state() {
    local repo_root_json
    repo_root_json="$(python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$REPO_ROOT")"
    _run_with_timeout 10 tauri-pilot eval \
        "(async () => { const repoRoot = $repo_root_json; localStorage.setItem(\"kairox.locale\", \"en\"); localStorage.setItem(\"kairox.left-sidebar-collapsed\", \"false\"); localStorage.setItem(\"kairox.right-sidebar-collapsed\", \"false\"); localStorage.removeItem(\"kairox.last-active-session-id\"); localStorage.setItem(\"kairox.pilot.repoRoot\", repoRoot); const invoke = window.__TAURI_INTERNALS__.invoke; const projects = await invoke(\"list_projects\"); if (!projects.some((project) => project.root_path === repoRoot)) { await invoke(\"add_existing_project\", { path: repoRoot }); } window.location.hash = \"#/workbench\"; window.location.reload(); return repoRoot; })()" >/dev/null
}

_wait_for_app_shell() {
    local ready_message="$1"
    local timeout_message="$2"

    for i in $(seq 1 30); do
        if result="$(_run_with_timeout 5 tauri-pilot eval 'Boolean(document.querySelector("[data-test=\"app-shell\"]"))' 2>/dev/null)" && [[ "$result" = "true" ]]; then
            echo "$ready_message after ${i}s"
            return
        fi
        if [[ "$i" -eq 30 ]]; then
            echo "ERROR: $timeout_message" >&2
            exit 1
        fi
        sleep 1
    done
}

_restore_config() {
    if [[ -f "$CONFIG_BACKUP" ]]; then
        mv "$CONFIG_BACKUP" "$CONFIG_TOML" 2>/dev/null || true
    else
        rm -f "$CONFIG_TOML" 2>/dev/null || true
    fi
    _restore_project_config
    if [[ "$PILOT_HOME_CREATED" -eq 1 ]]; then
        rm -rf "$PILOT_HOME" 2>/dev/null || true
    fi
}

_run_with_timeout() {
    local timeout_secs="$1"
    shift

    "$@" &
    local cmd_pid=$!

    (
        sleep "$timeout_secs"
        kill "$cmd_pid" 2>/dev/null || true
    ) &
    local timer_pid=$!

    local status=0
    if wait "$cmd_pid"; then
        status=0
    else
        status=$?
    fi

    kill "$timer_pid" 2>/dev/null || true
    wait "$timer_pid" 2>/dev/null || true
    return "$status"
}

_port_1420_listening() {
    (echo >/dev/tcp/127.0.0.1/1420) >/dev/null 2>&1
}

_start_vite_if_needed() {
    if _port_1420_listening; then
        echo "Vite dev server already listening on 127.0.0.1:1420"
        return
    fi

    echo "Starting Vite dev server on 127.0.0.1:1420"
    (
        cd "$REPO_ROOT/apps/agent-gui"
        bun run dev
    ) >"$JUNIT_DIR/vite.log" 2>&1 &
    VITE_PID=$!

    for i in $(seq 1 60); do
        if _port_1420_listening; then
            echo "Vite dev server ready after ${i}s"
            return
        fi
        if ! kill -0 "$VITE_PID" 2>/dev/null; then
            echo "ERROR: Vite dev server exited before becoming ready" >&2
            tail -50 "$JUNIT_DIR/vite.log" >&2 || true
            exit 1
        fi
        sleep 1
    done

    echo "ERROR: Vite dev server did not become ready within 60s" >&2
    tail -50 "$JUNIT_DIR/vite.log" >&2 || true
    exit 1
}

if [[ ! -d "$FIXTURE_MARKETPLACE_DIR" ]]; then
    echo "ERROR: fixture marketplace directory not found: $FIXTURE_MARKETPLACE_DIR" >&2
    echo "       Ensure the pilot fixture plugin is present." >&2
    exit 1
fi

if [[ ! -f "$FIXTURE_MCP_SERVER_SCRIPT" ]]; then
    echo "ERROR: fixture MCP server not found: $FIXTURE_MCP_SERVER_SCRIPT" >&2
    echo "       Ensure the MCP fixture server is present." >&2
    exit 1
fi

_select_scenarios

if [[ ${#scenarios[@]} -eq 0 ]]; then
    echo "ERROR: no scenario .toml files under $SCENARIO_DIR" >&2
    exit 1
fi

echo "Selected pilot scenarios:"
for scenario in "${scenarios[@]}"; do
    echo "  - $(basename "$scenario")"
done

if [[ "${KAIROX_PILOT_LIST_SCENARIOS:-0}" = "1" ]]; then
    exit 0
fi

if [[ ! -x "$APP_BIN" ]]; then
    echo "ERROR: Tauri debug binary not found or not executable: $APP_BIN" >&2
    echo "       Build it first with: cargo tauri build --debug --no-bundle --features pilot" >&2
    exit 1
fi

if ! command -v tauri-pilot >/dev/null 2>&1; then
    echo "ERROR: tauri-pilot CLI not on PATH." >&2
    echo "       Install it: cargo install --git https://github.com/mpiton/tauri-pilot tauri-pilot-cli" >&2
    exit 1
fi

# Pre-create the directories that scenarios write into so tauri-pilot doesn't
# fail on a missing path (the screenshot action does not auto-create dirs).
mkdir -p "$JUNIT_DIR" "$FAILURE_DIR"

echo "Registering pilot fixtures:"
echo "  home:        $PILOT_HOME"
echo "  marketplace: $FIXTURE_MARKETPLACE_DIR"
echo "  mcp server:  $FIXTURE_MCP_SERVER_SCRIPT"
_backup_project_config
APP_PID=""
VITE_PID=""
trap 'if [[ -n "${APP_PID:-}" ]]; then kill "$APP_PID" 2>/dev/null || true; wait "$APP_PID" 2>/dev/null || true; fi; if [[ -n "${VITE_PID:-}" ]]; then kill "$VITE_PID" 2>/dev/null || true; wait "$VITE_PID" 2>/dev/null || true; fi; _restore_config' EXIT
_register_pilot_fixtures
_write_pilot_project_config

# ─── Launch the app ────────────────────────────────────────────────────────────

_start_vite_if_needed

HOME="$PILOT_HOME" "$APP_BIN" &
APP_PID=$!

# Wait for the pilot socket to come up (up to 30s). `tauri-pilot ping` exits
# non-zero while the socket isn't listening, which is wrapped in `if` so
# `set -e` doesn't trip.
for i in $(seq 1 30); do
    if _run_with_timeout 5 tauri-pilot ping >/dev/null 2>&1; then
        echo "tauri-pilot connected after ${i}s"
        break
    fi
    if [[ "$i" -eq 30 ]]; then
        echo "ERROR: tauri-pilot ping timed out after 30s" >&2
        exit 1
    fi
    sleep 1
done

# `ping` only verifies the IPC socket is up; the webview itself takes a few
# additional seconds to attach (especially under xvfb on Linux). Without this
# extra probe the first scenario step fails with:
#   RPC error (-32603): Eval failed: No webview available
# We poll a trivial `eval` until it succeeds (up to 30s) so that scenarios
# start with a known-ready webview.
for i in $(seq 1 30); do
    if _run_with_timeout 5 tauri-pilot eval '1' >/dev/null 2>&1; then
        echo "webview ready after ${i}s"
        break
    fi
    if [[ "$i" -eq 30 ]]; then
        echo "ERROR: webview did not become ready within 30s after ping" >&2
        exit 1
    fi
    sleep 1
done

_wait_for_app_shell "initial app shell ready" "app shell did not become ready before browser state reset"
_write_pilot_browser_state
_wait_for_app_shell "browser state applied" "app shell did not return after browser state reset"

# ─── Run every scenario ────────────────────────────────────────────────────────

failed=0
for scenario in "${scenarios[@]}"; do
    name="$(basename "$scenario" .toml)"
    junit="$JUNIT_DIR/$name.xml"
    echo "── Running $name ───────────────────────────────────────────────"
    if [[ "${PILOT_RESET_BETWEEN_SCENARIOS:-1}" != "0" ]]; then
        _write_pilot_browser_state
        _wait_for_app_shell "scenario browser state reset" "app shell did not return after scenario browser state reset"
    fi
    if ! tauri-pilot run "$scenario" --junit "$junit"; then
        failed=1
        echo "FAIL: $name (junit: $junit)"
    fi
done

if [[ $failed -ne 0 ]]; then
    echo "One or more pilot scenarios failed. See $JUNIT_DIR/*.xml and $FAILURE_DIR/." >&2
    exit 1
fi

echo "All pilot scenarios passed."
