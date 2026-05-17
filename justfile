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
    @echo "✅ Version bumped to {{ version }} in Cargo.toml, Cargo.lock, package.json (root), apps/agent-gui/package.json, tauri.conf.json"
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

# Run TUI app logic integration tests (no terminal required)
test-tui:
    cargo test -p agent-tui --test app_logic

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

# Build the Tauri debug binary with the pilot plugin enabled and run the
# tauri-pilot E2E scenarios under apps/agent-gui/e2e-pilot/. Requires the
# tauri-pilot CLI on PATH; install via:
#     cargo install --git https://github.com/mpiton/tauri-pilot --tag v0.5.1 tauri-pilot-cli
# On Linux you typically need to wrap this recipe in `xvfb-run -a just
# test-pilot`; on macOS the Tauri window will appear briefly during the run.
test-pilot:
    bun --filter agent-gui tauri build --debug --no-bundle --features pilot
    scripts/run-pilot-tests.sh
    @echo "✅ tauri-pilot E2E scenarios passed"
