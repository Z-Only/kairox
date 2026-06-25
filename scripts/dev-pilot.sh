#!/usr/bin/env bash
# Start the Kairox Dev App with the pilot feature enabled.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
DRY_RUN="${KAIROX_DEV_PILOT_DRY_RUN:-0}"
PING_TIMEOUT_SECS="${KAIROX_DEV_PILOT_TIMEOUT_SECS:-240}"
PING_INTERVAL_SECS="${KAIROX_DEV_PILOT_PING_INTERVAL_SECS:-2}"
VITE_READY_TIMEOUT_SECS="${KAIROX_DEV_PILOT_VITE_TIMEOUT_SECS:-60}"
APP_LOG="${KAIROX_DEV_PILOT_APP_LOG:-/tmp/kairox-dev-pilot-app.log}"
VITE_LOG="${KAIROX_DEV_PILOT_VITE_LOG:-/tmp/kairox-dev-pilot-vite.log}"
TAURI_LOG="${KAIROX_DEV_PILOT_TAURI_LOG:-/tmp/kairox-dev-pilot-tauri.log}"
FALLBACK_DEV_PORT="1420"

DEFAULT_PID=""
VITE_PID=""
TAURI_PID=""
DEFAULT_SOCKET=""
FALLBACK_SOCKET=""
SELECTED_DEV_PORT=""

_is_enabled() {
    case "${1:-}" in
        1 | true | TRUE | yes | YES | on | ON) return 0 ;;
        *) return 1 ;;
    esac
}

_quote() {
    printf "%q" "$1"
}

_print_command() {
    printf "  $"
    local arg
    for arg in "$@"; do
        printf " %s" "$(_quote "$arg")"
    done
    printf "\n"
}

_print_shell_command() {
    printf "  $ %s\n" "$1"
}

_require_command() {
    local name="$1"
    local install_hint="$2"
    if ! command -v "$name" >/dev/null 2>&1; then
        echo "ERROR: required command not found: $name" >&2
        echo "       $install_hint" >&2
        exit 1
    fi
}

_tauri_cli_available() {
    [[ -e "$REPO_ROOT/apps/agent-gui/node_modules/.bin/tauri" ]] ||
        command -v tauri >/dev/null 2>&1
}

_stop_tree() {
    local pid="${1:-}"
    [[ -n "$pid" ]] || return 0
    kill -0 "$pid" 2>/dev/null || return 0

    local child
    while IFS= read -r child; do
        [[ -n "$child" ]] || continue
        _stop_tree "$child"
    done < <(pgrep -P "$pid" 2>/dev/null || true)

    kill "$pid" 2>/dev/null || true
}

_wait_for_pid() {
    local pid="${1:-}"
    [[ -n "$pid" ]] || return 0
    wait "$pid" 2>/dev/null || true
}

_cleanup() {
    _stop_tree "$TAURI_PID"
    _stop_tree "$VITE_PID"
    _stop_tree "$DEFAULT_PID"
    _wait_for_pid "$TAURI_PID"
    _wait_for_pid "$VITE_PID"
    _wait_for_pid "$DEFAULT_PID"
}

trap _cleanup EXIT INT TERM

_tail_log_hint() {
    local log="$1"
    if [[ -f "$log" ]]; then
        echo "       Last log lines from $log:" >&2
        tail -80 "$log" >&2 || true
    else
        echo "       Log file was not created: $log" >&2
    fi
}

_resolve_socket() {
    local identifier="$1"
    KAIROX_DEV_HELPER="$REPO_ROOT/apps/agent-gui/scripts/dev-port.mjs" \
        KAIROX_PILOT_IDENTIFIER="$identifier" \
        node --input-type=module <<'EOF'
const { buildTauriPilotSocketPath } = await import(process.env.KAIROX_DEV_HELPER);
console.log(buildTauriPilotSocketPath(process.env.KAIROX_PILOT_IDENTIFIER, process.env));
EOF
}

_resolve_default_launch() {
    KAIROX_DEV_HELPER="$REPO_ROOT/apps/agent-gui/scripts/dev-port.mjs" \
        KAIROX_DEV_PREFERRED_PORT="${KAIROX_DEV_PORT:-1420}" \
        node --input-type=module <<'EOF'
const {
  buildTauriDevIdentifier,
  buildTauriPilotSocketPath,
  resolveTauriDevPort
} = await import(process.env.KAIROX_DEV_HELPER);
const port = await resolveTauriDevPort({
  ...process.env,
  KAIROX_DEV_PORT: process.env.KAIROX_DEV_PREFERRED_PORT
});
const identifier = buildTauriDevIdentifier(port);
console.log(`${port}\n${identifier}\n${buildTauriPilotSocketPath(identifier, process.env)}`);
EOF
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

_ping_pilot() {
    local socket="$1"
    if command -v tauri-pilot >/dev/null 2>&1; then
        TAURI_PILOT_SOCKET="$socket" tauri-pilot ping >/dev/null 2>&1
        return $?
    fi

    [[ -S "$socket" ]]
}

_dev_port_listening() {
    (echo >/dev/tcp/127.0.0.1/"$FALLBACK_DEV_PORT") >/dev/null 2>&1
}

_print_dev_port_listener() {
    lsof -nP -iTCP:"$FALLBACK_DEV_PORT" -sTCP:LISTEN >&2 || true
}

_wait_for_vite() {
    local waited=0
    echo "Waiting for Vite dev server on 127.0.0.1:${FALLBACK_DEV_PORT}:"

    while [[ "$waited" -le "$VITE_READY_TIMEOUT_SECS" ]]; do
        if _dev_port_listening; then
            echo "Vite ready after ${waited}s."
            return 0
        fi

        if [[ -n "$VITE_PID" ]] && ! kill -0 "$VITE_PID" 2>/dev/null; then
            echo "ERROR: Vite dev server exited before port ${FALLBACK_DEV_PORT} became reachable." >&2
            _tail_log_hint "$VITE_LOG"
            return 1
        fi

        sleep 1
        waited=$((waited + 1))
    done

    echo "ERROR: Vite dev server did not listen on 127.0.0.1:${FALLBACK_DEV_PORT} within ${VITE_READY_TIMEOUT_SECS}s." >&2
    _tail_log_hint "$VITE_LOG"
    return 1
}

_wait_for_pilot() {
    local socket="$1"
    local main_pid="$2"
    local log="$3"
    local label="$4"
    local waited=0

    echo "Waiting for pilot readiness:"
    echo "  socket: $socket"
    if command -v tauri-pilot >/dev/null 2>&1; then
        echo "  check:  TAURI_PILOT_SOCKET=\"$socket\" tauri-pilot ping"
    else
        echo "  check:  socket exists (-S); install tauri-pilot for stronger ping validation"
    fi

    while [[ "$waited" -le "$PING_TIMEOUT_SECS" ]]; do
        if _run_with_timeout 5 _ping_pilot "$socket"; then
            echo "Pilot ready after ${waited}s."
            return 0
        fi

        if [[ -n "$main_pid" ]] && ! kill -0 "$main_pid" 2>/dev/null; then
            echo "WARN: $label exited before pilot became reachable." >&2
            _tail_log_hint "$log"
            return 2
        fi

        sleep "$PING_INTERVAL_SECS"
        waited=$((waited + PING_INTERVAL_SECS))
    done

    echo "WARN: pilot did not become reachable within ${PING_TIMEOUT_SECS}s for $label." >&2
    _tail_log_hint "$log"
    return 3
}

_start_default() {
    echo "Starting default Tauri dev command:"
    _print_shell_command "KAIROX_HOME=$(_quote "$KAIROX_HOME") KAIROX_DEV_PORT=$(_quote "$SELECTED_DEV_PORT") KAIROX_DEV_STRICT_PORT=1 bun --filter agent-gui tauri dev --features pilot"
    (
        cd "$REPO_ROOT"
        KAIROX_HOME="$KAIROX_HOME" \
            KAIROX_DEV_PORT="$SELECTED_DEV_PORT" \
            KAIROX_DEV_STRICT_PORT=1 \
            bun --filter agent-gui tauri dev --features pilot
    ) >"$APP_LOG" 2>&1 &
    DEFAULT_PID=$!
}

_start_fallback() {
    echo "Starting split Vite + Tauri fallback:"
    if _dev_port_listening; then
        echo "ERROR: port ${FALLBACK_DEV_PORT} is already in use before starting fallback Vite." >&2
        echo "       Stop the existing listener or run the default Tauri wrapper once the Tauri CLI is available." >&2
        _print_dev_port_listener
        return 1
    fi

    _print_shell_command "(cd apps/agent-gui && bun run dev)"
    (
        cd "$REPO_ROOT/apps/agent-gui"
        bun run dev
    ) >"$VITE_LOG" 2>&1 &
    VITE_PID=$!
    _wait_for_vite

    _print_shell_command "(cd apps/agent-gui/src-tauri && KAIROX_HOME=$(_quote "$KAIROX_HOME") cargo run --no-default-features --features pilot --)"
    (
        cd "$REPO_ROOT/apps/agent-gui/src-tauri"
        KAIROX_HOME="$KAIROX_HOME" cargo run --no-default-features --features pilot --
    ) >"$TAURI_LOG" 2>&1 &
    TAURI_PID=$!
}

_run_forever_until_exit() {
    local pid="$1"
    local label="$2"
    local status=0
    set +e
    wait "$pid"
    status=$?
    set -e
    echo "$label exited with status $status."
    return "$status"
}

cd "$REPO_ROOT"
_require_command bun "Install Bun and run 'bun install' from the repo root."
_require_command cargo "Install the Rust toolchain with rustup."
_require_command node "Install Node.js; Bun workspace tooling expects it for helper scripts."

if [[ -z "${KAIROX_HOME:-}" ]]; then
    KAIROX_HOME="$(mktemp -d /tmp/kairox-dev-home.XXXXXX)"
    export KAIROX_HOME
    echo "KAIROX_HOME was not set; created $KAIROX_HOME"
else
    export KAIROX_HOME
    echo "Using KAIROX_HOME=$KAIROX_HOME"
fi
echo "HOME is not overridden; current HOME=${HOME:-<unset>}"

DEFAULT_INFO="$(_resolve_default_launch)"
SELECTED_DEV_PORT="$(printf "%s\n" "$DEFAULT_INFO" | sed -n '1p')"
DEFAULT_IDENTIFIER="$(printf "%s\n" "$DEFAULT_INFO" | sed -n '2p')"
DEFAULT_SOCKET="$(printf "%s\n" "$DEFAULT_INFO" | sed -n '3p')"
FALLBACK_IDENTIFIER="dev.kairox.agent"
FALLBACK_SOCKET="$(_resolve_socket "$FALLBACK_IDENTIFIER")"

echo "Default pilot target:"
echo "  port:       $SELECTED_DEV_PORT"
echo "  identifier: $DEFAULT_IDENTIFIER"
echo "  socket:     $DEFAULT_SOCKET"
echo "Fallback pilot target:"
echo "  identifier: $FALLBACK_IDENTIFIER"
echo "  socket:     $FALLBACK_SOCKET"

echo "Default command:"
_print_command bun --filter agent-gui tauri dev --features pilot
echo "Fallback commands:"
_print_shell_command "(cd apps/agent-gui && bun run dev)"
_print_shell_command "(cd apps/agent-gui/src-tauri && KAIROX_HOME=$(_quote "$KAIROX_HOME") cargo run --no-default-features --features pilot --)"

if _is_enabled "$DRY_RUN"; then
    if _tauri_cli_available; then
        echo "Dry run: Tauri CLI is available; default command would be used first."
    else
        echo "Dry run: Tauri CLI is unavailable; split fallback would be used."
    fi
    exit 0
fi

if _tauri_cli_available; then
    _start_default
    if _wait_for_pilot "$DEFAULT_SOCKET" "$DEFAULT_PID" "$APP_LOG" "default Tauri dev command"; then
        echo "Kairox Dev App is running with pilot enabled."
        echo "Press Ctrl-C to stop processes started by this wrapper."
        _run_forever_until_exit "$DEFAULT_PID" "Default Tauri dev command"
        exit $?
    fi

    echo "Falling back because the default Tauri dev command did not expose pilot readiness."
    _cleanup
    DEFAULT_PID=""
else
    echo "Tauri CLI is unavailable; skipping default command and using split fallback."
    echo "Install workspace dependencies with 'bun install' if this was unexpected."
fi

_start_fallback
if _wait_for_pilot "$FALLBACK_SOCKET" "$TAURI_PID" "$TAURI_LOG" "split Tauri cargo command"; then
    echo "Kairox Dev App is running with pilot enabled via split fallback."
    echo "Press Ctrl-C to stop processes started by this wrapper."
    _run_forever_until_exit "$TAURI_PID" "Split Tauri cargo command"
    exit $?
fi

echo "ERROR: split fallback failed to expose pilot readiness." >&2
echo "Diagnostics:" >&2
echo "  KAIROX_HOME=$KAIROX_HOME" >&2
echo "  Vite log:  $VITE_LOG" >&2
echo "  Tauri log: $TAURI_LOG" >&2
echo "  Check port 1420 with: lsof -nP -iTCP:1420 -sTCP:LISTEN" >&2
echo "  Check pilot with: TAURI_PILOT_SOCKET=\"$FALLBACK_SOCKET\" tauri-pilot ping" >&2
_tail_log_hint "$VITE_LOG"
_tail_log_hint "$TAURI_LOG"
exit 1
