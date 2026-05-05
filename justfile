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
    pnpm run format:check

# Run all linters (clippy + eslint + stylelint)
lint:
    pnpm run lint

# Run all Rust tests
test:
    cargo test --workspace --all-targets

# Run GUI (Vitest) tests
test-gui:
    pnpm --filter agent-gui run test

# Run everything: format check + lint + test (the full CI gate)
check: fmt-check lint test
    @echo "✅ All checks passed"

# ─── Formatting ────────────────────────────────────────────────

# Auto-format all code (Rust + web)
fmt:
    pnpm run format

# ─── Development ───────────────────────────────────────────────

# Run the TUI app
tui:
    cargo run -p agent-tui

# Run the GUI dev server (Vite hot-reload)
gui-dev:
    pnpm --filter agent-gui run dev

# Run the Tauri desktop app in dev mode (Vite + native window)
tauri-dev:
    pnpm --filter agent-gui run tauri:dev

# Build GUI web assets
gui-build:
    pnpm --filter agent-gui run build

# Build Tauri desktop app
tauri-build:
    pnpm --filter agent-gui run tauri:build

# ─── Release ───────────────────────────────────────────────────

# Prepare a release (version required, e.g.: just release 0.8.0)
release version *FLAGS:
    scripts/release.sh {{ version }} {{ FLAGS }}

# Dry-run a release to preview commands without executing
release-dry version:
    scripts/release.sh {{ version }} --dry-run

# Generate changelog for a tag (e.g.: just changelog v0.7.0)
changelog tag:
    git cliff --tag {{ tag }} -o CHANGELOG.md && pnpm prettier --write CHANGELOG.md

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
    git worktree add ../kairox-{{ name }} -b {{ name }} main
    cd ../kairox-{{ name }} && pnpm install
    @echo "✅ Worktree created at ../kairox-{{ name }}"

# ─── Type sync check ──────────────────────────────────────────

# Check that generated TypeScript types are in sync with Rust definitions
check-types:
    just gen-types
    git diff --exit-code apps/agent-gui/src/generated/ || (echo "❌ Generated types are out of sync! Run 'just gen-types' and commit the result." && exit 1)
    @echo "✅ Generated types are in sync"

# ─── Code generation ──────────────────────────────────────────

# Regenerate TypeScript bindings from Tauri commands and event types via specta
gen-types:
    cargo run -p agent-gui-tauri --bin export-specta -- apps/agent-gui/src/generated/commands.ts
    cargo run -p agent-gui-tauri --bin export-events -- apps/agent-gui/src/generated/events.ts
    @echo "✅ TypeScript bindings regenerated"

# ─── E2E / Integration tests ──────────────────────────────────

# Run GUI frontend E2E tests with Playwright (requires dev server)
test-e2e:
    pnpm --filter agent-gui run test:e2e

# Run GUI frontend E2E tests with Playwright (headed mode for debugging)
test-e2e-headed:
    pnpm --filter agent-gui run test:e2e:headed

# Run GUI frontend E2E tests with Playwright (UI mode)
test-e2e-ui:
    pnpm --filter agent-gui run test:e2e:ui

# Run TUI app logic integration tests (no terminal required)
test-tui:
    cargo test -p agent-tui --test app_logic

# Run full-stack runtime integration tests
test-fullstack:
    cargo test -p agent-runtime --test full_stack

# Run all test layers: unit + integration + fullstack + TUI
test-all: test test-tui test-fullstack test-gui
    @echo "✅ All tests passed"
