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

# ─── Fixture marketplace registration ──────────────────────────────────────────

# The app resolves db_dir as $HOME/.kairox (see lib.rs line 44).
# We write the fixture marketplace source to that config.toml so the
# app picks it up on startup.
FIXTURE_MARKETPLACE_DIR="$REPO_ROOT/apps/agent-gui/e2e-pilot/fixtures/plugin-marketplace"

CONFIG_DIR="$HOME/.kairox"
CONFIG_TOML="$CONFIG_DIR/config.toml"
CONFIG_BACKUP="$CONFIG_DIR/config.toml.pilot-backup"

_register_fixture_marketplace() {
    mkdir -p "$CONFIG_DIR"
    # Back up the original config so we can restore it after the run.
    if [[ -f "$CONFIG_TOML" ]]; then
        cp "$CONFIG_TOML" "$CONFIG_BACKUP"
    fi
    python3 - "$CONFIG_TOML" "$FIXTURE_MARKETPLACE_DIR" <<'PYEOF'
import sys, os
config_path = sys.argv[1]
fixture_dir = sys.argv[2]

try:
    with open(config_path) as f:
        content = f.read()
except FileNotFoundError:
    content = ""

if 'id = "pilot-fixture"' in content:
    print("pilot-fixture marketplace already registered, skipping")
    sys.exit(0)

if content.rstrip('\n'):
    content = content.rstrip('\n') + '\n\n'

# Enable the local pilot-fixture marketplace source.
content += '[[plugin_marketplaces]]\n'
content += 'id = "pilot-fixture"\n'
content += 'display_name = "Pilot Test Fixtures"\n'
content += f'source = "{fixture_dir}"\n'
content += 'enabled = true\n'

# Disable the default GitHub marketplace sources so the catalog loads
# instantly from the local fixture only — no network dependency.
content += '\n[[plugin_marketplaces]]\n'
content += 'id = "claude-plugins-official"\n'
content += 'display_name = "Claude Plugins Official"\n'
content += 'source = "anthropics/claude-plugins-official"\n'
content += 'enabled = false\n'

content += '\n[[plugin_marketplaces]]\n'
content += 'id = "anthropics-claude-code"\n'
content += 'display_name = "Anthropic Claude Code"\n'
content += 'source = "anthropics/claude-code"\n'
content += 'enabled = false\n'

with open(config_path, 'w') as f:
    f.write(content)
print(f"Registered pilot-fixture marketplace source in {config_path}")
PYEOF
}

_restore_config() {
    if [[ -f "$CONFIG_BACKUP" ]]; then
        mv "$CONFIG_BACKUP" "$CONFIG_TOML" 2>/dev/null || true
    else
        rm -f "$CONFIG_TOML" 2>/dev/null || true
    fi
}

if [[ ! -d "$FIXTURE_MARKETPLACE_DIR" ]]; then
    echo "ERROR: fixture marketplace directory not found: $FIXTURE_MARKETPLACE_DIR" >&2
    echo "       Ensure the pilot fixture plugin is present." >&2
    exit 1
fi

echo "Registering fixture marketplace: $FIXTURE_MARKETPLACE_DIR"
_register_fixture_marketplace

# ─── Launch the app ────────────────────────────────────────────────────────────

"$APP_BIN" &
APP_PID=$!
trap '_restore_config; kill "$APP_PID" 2>/dev/null || true; wait "$APP_PID" 2>/dev/null || true' EXIT

# Wait for the pilot socket to come up (up to 30s). `tauri-pilot ping` exits
# non-zero while the socket isn't listening, which is wrapped in `if` so
# `set -e` doesn't trip.
for i in $(seq 1 30); do
    if tauri-pilot ping >/dev/null 2>&1; then
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
    if tauri-pilot eval '1' >/dev/null 2>&1; then
        echo "webview ready after ${i}s"
        break
    fi
    if [[ "$i" -eq 30 ]]; then
        echo "ERROR: webview did not become ready within 30s after ping" >&2
        exit 1
    fi
    sleep 1
done

# ─── Run every scenario ────────────────────────────────────────────────────────

# `tauri-pilot run` accepts exactly one scenario path; iterate explicitly.
shopt -s nullglob
scenarios=("$SCENARIO_DIR"/*.toml)
shopt -u nullglob

# In CI, skip scenarios that require a live LLM backend.
# chat-flow.toml and audit-chat.toml send real messages and wait for the response;
# without a configured model the app fails with an HTTP error
# and the user-message element never appears.
if [ "${CI:-}" = "true" ]; then
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

if [[ ${#scenarios[@]} -eq 0 ]]; then
    echo "ERROR: no scenario .toml files under $SCENARIO_DIR" >&2
    exit 1
fi

failed=0
for scenario in "${scenarios[@]}"; do
    name="$(basename "$scenario" .toml)"
    junit="$JUNIT_DIR/$name.xml"
    echo "── Running $name ───────────────────────────────────────────────"
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
