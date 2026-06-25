# Kairox — task runner
# Install just: cargo install just
# List all tasks: just --list

set dotenv-load := false

# Default: list available tasks
default:
    @just --list

# ─── Quick checks ──────────────────────────────────────────────

# Run all format checks (Rust + web)
fmt-check:
    bun run format:check

# Run all linters (clippy + oxlint + stylelint)
lint:
    bun run lint

# Run all Rust tests
test:
    cargo test --workspace --all-targets

# Run GUI (Vitest) tests
test-gui:
    bun --filter agent-gui test
    bun --filter agent-gui test:scripts

# Run Rust and GUI coverage gates
coverage: coverage-rust coverage-web

# Run Rust source-based coverage with branch-first thresholds
coverage-rust:
    bun run coverage:rust

# Run GUI coverage with Vitest V8 thresholds
coverage-web:
    bun run coverage:web

# Run everything: format check + lint + test (the full CI gate)
check: fmt-check lint test
    @echo "✅ All checks passed"

# ─── Formatting ────────────────────────────────────────────────

# Auto-format all code (Rust + web)
fmt:
    bun run format

# ─── Development ───────────────────────────────────────────────

# Run the TUI app
tui:
    cargo run -p agent-tui

# Run the GUI dev server (Vite hot-reload)
gui-dev: gen-types
    bun --filter agent-gui dev

# Run the Tauri desktop app in dev mode (Vite + native window)
tauri-dev: gen-types
    bun --filter agent-gui tauri:dev

# Run the Tauri desktop app with pilot enabled and split fallback diagnostics
dev-pilot:
    bun run dev:pilot

# Build GUI web assets
gui-build: gen-types
    bun --filter agent-gui build

# Build Tauri desktop app
tauri-build: gen-types
    bun --filter agent-gui tauri:build

# Build Tauri desktop app without generating installer bundles
tauri-build-fast: gen-types
    bun --filter agent-gui tauri build --no-bundle

# Build GUI web assets and print the largest generated files
gui-size: gui-build
    @du -sh apps/agent-gui/dist
    @find apps/agent-gui/dist -type f -exec ls -lh {} \; | sort -k5 -hr | head -30 | cat

# Print release binary sizes when the binaries have already been built
rust-size:
    @test -f target/release/agent-tui && ls -lh target/release/agent-tui || echo "target/release/agent-tui not built"
    @test -f target/release/agent-gui-tauri && ls -lh target/release/agent-gui-tauri || echo "target/release/agent-gui-tauri not built"

# ─── Release ────────────────────────────────────────────────────

# Prepare a release (version required, e.g.: just release 0.8.0)
release version *FLAGS:
    scripts/release.sh {{ version }} {{ FLAGS }}

# Dry-run a release to preview commands without executing
release-dry version:
    scripts/release.sh {{ version }} --dry-run

# Generate changelog for a tag (e.g.: just changelog v0.7.0)
changelog tag:
    git cliff --tag {{ tag }} -o CHANGELOG.md && bunx oxfmt --write CHANGELOG.md

# ─── Version ───────────────────────────────────────────────────

# Bump version across all config files (e.g.: just bump-version 0.8.0)
bump-version version:
    @echo "Bumping version to {{ version }}..."
    sed -i '' 's/^version = ".*"/version = "{{ version }}"/' Cargo.toml
    sed -i '' 's/"version": ".*"/"version": "{{ version }}"/' apps/agent-gui/package.json
    sed -i '' 's/"version": ".*"/"version": "{{ version }}"/' apps/agent-gui/src-tauri/tauri.conf.json
    sed -i '' 's/"version": ".*"/"version": "{{ version }}"/' package.json
    cargo generate-lockfile
    node scripts/release-version-docs.mjs --write
    @echo "✅ Version bumped to {{ version }} in Cargo.toml, Cargo.lock, package.json (root), apps/agent-gui/package.json, tauri.conf.json, docs/current-release.json, and current release docs"
    @echo "⚠️  Remember to review the diff and commit"

# ─── Worktree ──────────────────────────────────────────────────

# Create a git worktree for isolated branch development
worktree name:
    @safe_name="$(printf '%s' '{{ name }}' | sed 's#[^A-Za-z0-9._-]#-#g')" ; \
        mkdir -p .worktrees ; \
        git check-ignore -q .worktrees || { echo "❌ .worktrees/ must be ignored before creating worktrees"; exit 1; } ; \
        git worktree add ".worktrees/$safe_name" -b "{{ name }}" main ; \
        cd ".worktrees/$safe_name" && bun install ; \
        echo "✅ Worktree created at .worktrees/$safe_name for branch {{ name }}"

# ─── Type sync check ──────────────────────────────────────────

# Check that generated TypeScript types are in sync with Rust definitions
check-types:
    just gen-types
    git diff --exit-code apps/agent-gui/src/generated/ || (echo "❌ Generated types are out of sync! Run 'just gen-types' and commit the result." && exit 1)
    @echo "✅ Generated types are in sync"

# ─── Code generation ──────────────────────────────────────────

# Regenerate TypeScript bindings from Tauri commands and event types via specta
gen-types:
    cargo run -p agent-gui-tauri --features typegen --bin export-specta -- apps/agent-gui/src/generated/commands.ts
    cargo run -p agent-gui-tauri --features typegen --bin export-events -- apps/agent-gui/src/generated/events.ts
    bunx oxfmt --write apps/agent-gui/src/generated/commands.ts apps/agent-gui/src/generated/events.ts
    @echo "✅ TypeScript bindings regenerated"

# ─── E2E / Integration tests ──────────────────────────────────

# Run GUI frontend E2E tests with Playwright (requires dev server)
test-e2e: gen-types
    bun --filter agent-gui test:e2e

# Run GUI frontend E2E tests with Playwright (headed mode for debugging)
test-e2e-headed: gen-types
    bun --filter agent-gui test:e2e:headed

# Run GUI frontend E2E tests with Playwright (UI mode)
test-e2e-ui: gen-types
    bun --filter agent-gui test:e2e:ui

# Run deterministic TUI test layers (no real terminal required)
test-tui:
    cargo test -p agent-tui

# Run the real PTY TUI smoke test used by CI
test-tui-pty:
    cargo build -p agent-tui
    KAIROX_TUI_BIN=target/debug/agent-tui cargo test -p agent-tui --test terminal_pty_smoke -- --ignored --nocapture

# Run full-stack runtime integration tests
test-fullstack:
    cargo test -p agent-runtime --test full_stack

# Run all test layers: unit + integration + fullstack + TUI
test-all: test test-tui test-fullstack test-gui
    @echo "✅ All tests passed"

# Run MCP-related unit and integration tests
test-mcp:
    cargo test -p agent-mcp --all-targets
    cargo test -p agent-tools -- mcp
    cargo test -p agent-config -- mcp
    cargo test -p agent-runtime --test mcp_integration
    @echo "✅ MCP tests passed"

# Run live model integration tests against GitHub Models (requires GITHUB_TOKEN;
# the test self-skips with a notice when GITHUB_TOKEN is absent, so this is
# safe to invoke locally without configuring credentials)
test-live:
    cargo test -p agent-runtime --features live-model-tests --test live_model_tests -- --nocapture
    @echo "✅ Live model tests passed (or skipped without GITHUB_TOKEN)"

# Run the deterministic kairox-eval smoke fixtures (fake profile, no
# network): the base smoke fixture, the tool-call fixture, and the
# compaction fixture. Each produces results.jsonl + summary.json under
# target/eval-smoke/<name>/.
#
# After building, HOME is redirected to a per-run temp dir for the
# binary invocations so the recipe ignores any user-level
# `~/.kairox/config.toml` overrides and uses the built-in fake-profile
# defaults; this keeps local runs equivalent to CI. HOME is left intact
# for `cargo build` itself so rustup and the cargo cache work normally.
eval-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet -p agent-eval --bin kairox-eval
    KAIROX_EVAL_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    KAIROX_EVAL_BIN="${KAIROX_EVAL_TARGET_DIR}/debug/kairox-eval"
    KAIROX_EVAL_HOME="$(mktemp -d)"
    KAIROX_EVAL_WS="$(mktemp -d)"
    trap 'rm -rf "$KAIROX_EVAL_HOME" "$KAIROX_EVAL_WS"' EXIT
    mkdir -p target/eval-smoke/base
    HOME="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/smoke.jsonl \
        --output target/eval-smoke/base/results.jsonl \
        --summary target/eval-smoke/base/summary.json \
        --workspace "$KAIROX_EVAL_WS"
    mkdir -p target/eval-smoke/tool-call
    HOME="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/smoke-tool-call.jsonl \
        --output target/eval-smoke/tool-call/results.jsonl \
        --summary target/eval-smoke/tool-call/summary.json \
        --workspace "$KAIROX_EVAL_WS" \
        --fake-emit-tool-call \
        --wait-timeout-ms 5000
    mkdir -p target/eval-smoke/compaction
    HOME="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/smoke-compaction.jsonl \
        --output target/eval-smoke/compaction/results.jsonl \
        --summary target/eval-smoke/compaction/summary.json \
        --workspace "$KAIROX_EVAL_WS" \
        --auto-compact-threshold 0.001 \
        --seed-synthetic-pairs 4 \
        --wait-timeout-ms 5000

# Run the executable noop guard fixture that fails intent-only completions.
# It requires both a deterministic tool invocation and a workspace artifact.
eval-noop-guard:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet -p agent-eval --bin kairox-eval
    KAIROX_EVAL_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    KAIROX_EVAL_BIN="${KAIROX_EVAL_TARGET_DIR}/debug/kairox-eval"
    KAIROX_EVAL_HOME="$(mktemp -d)"
    KAIROX_EVAL_WS="$(mktemp -d)"
    trap 'rm -rf "$KAIROX_EVAL_HOME" "$KAIROX_EVAL_WS"' EXIT
    mkdir -p target/eval-noop-guard
    HOME="$KAIROX_EVAL_HOME" USERPROFILE="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/noop-guard.jsonl \
        --output target/eval-noop-guard/results.jsonl \
        --summary target/eval-noop-guard/summary.json \
        --workspace "$KAIROX_EVAL_WS" \
        --profile fake \
        --fake-emit-tool-call \
        --fake-tool-id fs.write \
        --fake-tool-arguments '{"path":"target/noop-guard/output.txt","content":"ok\n"}' \
        --wait-timeout-ms 5000

# Run the extended kairox-eval fixtures (expectations-extended + multi-turn
# + trajectory). Same fake-profile setup as eval-smoke; no network needed.
eval-extended:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet -p agent-eval --bin kairox-eval
    KAIROX_EVAL_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    KAIROX_EVAL_BIN="${KAIROX_EVAL_TARGET_DIR}/debug/kairox-eval"
    KAIROX_EVAL_HOME="$(mktemp -d)"
    KAIROX_EVAL_WS="$(mktemp -d)"
    trap 'rm -rf "$KAIROX_EVAL_HOME" "$KAIROX_EVAL_WS"' EXIT
    mkdir -p target/eval-extended/expectations
    HOME="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/expectations-extended.jsonl \
        --output target/eval-extended/expectations/results.jsonl \
        --summary target/eval-extended/expectations/summary.json \
        --workspace "$KAIROX_EVAL_WS"
    mkdir -p target/eval-extended/multi-turn
    HOME="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/multi-turn.jsonl \
        --output target/eval-extended/multi-turn/results.jsonl \
        --summary target/eval-extended/multi-turn/summary.json \
        --workspace "$KAIROX_EVAL_WS"
    mkdir -p target/eval-extended/trajectory
    HOME="$KAIROX_EVAL_HOME" "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/trajectory.jsonl \
        --output target/eval-extended/trajectory/results.jsonl \
        --summary target/eval-extended/trajectory/summary.json \
        --workspace "$KAIROX_EVAL_WS" \
        --fake-emit-tool-call \
        --wait-timeout-ms 5000

# Run live model eval scenarios against a real model profile.
# Not included in CI; run manually to measure real model quality.
# Override the profile via: just eval-live <profile>
# Uses the config's default profile if none specified.
eval-live profile="":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet -p agent-eval --bin kairox-eval
    KAIROX_EVAL_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    KAIROX_EVAL_BIN="${KAIROX_EVAL_TARGET_DIR}/debug/kairox-eval"
    KAIROX_EVAL_WS="$(mktemp -d)"
    trap 'rm -rf "$KAIROX_EVAL_WS"' EXIT
    echo "# Kairox Eval Workspace" > "$KAIROX_EVAL_WS/README.md"
    mkdir -p target/eval-live
    PROFILE_ARG=""
    if [ -n "{{profile}}" ]; then
        PROFILE_ARG="--profile {{profile}}"
    fi
    "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/live-smoke.jsonl \
        --output target/eval-live/results.jsonl \
        --report target/eval-live/report.json \
        --workspace "$KAIROX_EVAL_WS" \
        $PROFILE_ARG \
        --tag live \
        --enable-mcp

# Run a live vibe-coding eval in a disposable Kairox worktree.
# By default the programming project is pinned to the pre-regression baseline
# commit 4371a71d068e94da1016b632ea2db2378a0582b2, not the current HEAD.
# Override KAIROX_EVAL_PROJECT_COMMIT only for ad hoc local experiments.
# Defaults to the stable live-model profile used for local model quality checks.
# Override the profile via: just eval-vibe-coding <profile>
eval-vibe-coding profile="kairox-live":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet -p agent-eval --bin kairox-eval
    KAIROX_EVAL_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    KAIROX_EVAL_BIN="${KAIROX_EVAL_TARGET_DIR}/debug/kairox-eval"
    KAIROX_EVAL_TMP="$(mktemp -d)"
    KAIROX_EVAL_WS="$KAIROX_EVAL_TMP/kairox-vibe-worktree"
    KAIROX_EVAL_PROJECT_COMMIT="${KAIROX_EVAL_PROJECT_COMMIT:-4371a71d068e94da1016b632ea2db2378a0582b2}"
    cleanup() {
        git worktree remove --force "$KAIROX_EVAL_WS" >/dev/null 2>&1 || true
        rm -rf "$KAIROX_EVAL_TMP"
    }
    trap cleanup EXIT
    git cat-file -e "$KAIROX_EVAL_PROJECT_COMMIT^{commit}"
    git worktree add --detach "$KAIROX_EVAL_WS" "$KAIROX_EVAL_PROJECT_COMMIT"
    mkdir -p target/eval-vibe-coding
    "$KAIROX_EVAL_BIN" \
        run \
        --scenarios crates/agent-eval/fixtures/live-vibe-coding.jsonl \
        --output target/eval-vibe-coding/results.jsonl \
        --report target/eval-vibe-coding/report.json \
        --workspace "$KAIROX_EVAL_WS" \
        --profile "{{profile}}" \
        --approval-policy on_request \
        --tag vibe-coding \
        --scenario-timeout-ms 300000 \
        --allow-post-run-commands \
        --fail-fast

# Run the tauri-pilot E2E scenarios under apps/agent-gui/e2e-pilot/.
# Requires the tauri-pilot CLI on PATH; install via:
#     cargo install --git https://github.com/mpiton/tauri-pilot tauri-pilot-cli
# On Linux you typically need to wrap this recipe in `xvfb-run -a just
# test-pilot`; on macOS the Tauri window will appear briefly during the run.
test-pilot:
    scripts/run-pilot-tests.sh
    @echo "✅ tauri-pilot E2E scenarios passed"

# Run the bounded tauri-pilot scenarios that exercise GitHub Models through
# the desktop app. Requires GITHUB_TOKEN with GitHub Models access.
test-pilot-live:
    KAIROX_PILOT_LIVE_MODELS=1 scripts/run-pilot-tests.sh
    @echo "✅ live tauri-pilot E2E scenarios passed"
