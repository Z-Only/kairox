# Packaging & Release Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve Kairox release pipeline with bundle metadata, build-time version embedding, consistent artifact naming, SHA256 checksums, enhanced release script, and an inactive Tauri updater skeleton.

**Architecture:** Pure configuration/CI/script changes plus a small `BuildInfo` module in `agent-core`. No domain logic changes. All changes are backward-compatible — the updater skeleton is disabled by default, and build info is informational only.

**Tech Stack:** Rust (build.rs, agent-core), Tauri 2 config, GitHub Actions YAML, Bash script, justfile

---

## File Structure

| Action | File                                       | Responsibility                                              |
| ------ | ------------------------------------------ | ----------------------------------------------------------- |
| Create | `crates/agent-core/src/build_info.rs`      | BuildInfo struct + from_env()                               |
| Modify | `crates/agent-core/src/lib.rs`             | Export `build_info` module                                  |
| Create | `crates/agent-tui/build.rs`                | Inject KAIROX_VERSION/GIT_HASH/BUILD_TIME                   |
| Modify | `crates/agent-tui/src/main.rs`             | Print version on startup                                    |
| Modify | `apps/agent-gui/src-tauri/build.rs`        | Inject KAIROX_VERSION/GIT_HASH/BUILD_TIME                   |
| Modify | `apps/agent-gui/src-tauri/src/commands.rs` | Add `get_build_info` command                                |
| Modify | `apps/agent-gui/src-tauri/src/lib.rs`      | Register `get_build_info` in invoke handler                 |
| Modify | `apps/agent-gui/src-tauri/src/specta.rs`   | Register `BuildInfoResponse` type + command                 |
| Modify | `apps/agent-gui/src-tauri/tauri.conf.json` | Bundle metadata + updater skeleton                          |
| Modify | `apps/agent-gui/src-tauri/Cargo.toml`      | Add `tauri-plugin-updater` dep + `chrono` build-dep         |
| Modify | `.github/workflows/release-build.yml`      | Artifact naming, checksums, release notes, updater env vars |
| Modify | `scripts/release.sh`                       | --dry-run, --skip-checks, --skip-build, --prerelease        |
| Modify | `justfile`                                 | Update release recipe, add release-dry                      |

---

## Task 1: Tauri Bundle Metadata

**Files:**

- Modify: `apps/agent-gui/src-tauri/tauri.conf.json`

- [ ] **Step 1: Add bundle metadata fields to tauri.conf.json**

Replace the existing `"bundle"` section in `apps/agent-gui/src-tauri/tauri.conf.json` with:

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "category": "DeveloperTool",
    "copyright": "Copyright 2024-2026 Kairox Contributors",
    "shortDescription": "Local-first AI agent workbench",
    "longDescription": "Kairox is a local-first AI agent workbench with a shared Rust core, terminal UI, and desktop GUI for managing AI agent sessions, tasks, and memory.",
    "publisher": "Kairox",
    "macOS": {
      "minimumSystemVersion": "10.15"
    }
  }
}
```

- [ ] **Step 2: Verify JSON is valid**

Run: `python3 -c "import json; json.load(open('apps/agent-gui/src-tauri/tauri.conf.json'))"`  
Expected: No output (valid JSON)

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src-tauri/tauri.conf.json
git commit -m "feat(gui): add bundle metadata — category, copyright, publisher, descriptions"
```

---

## Task 2: BuildInfo Module in agent-core

**Files:**

- Create: `crates/agent-core/src/build_info.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Create build_info.rs**

Create `crates/agent-core/src/build_info.rs`:

```rust
//! Build information embedded at compile time.
//!
//! Each binary crate (agent-tui, agent-gui-tauri) injects `KAIROX_VERSION`,
//! `KAIROX_GIT_HASH`, and `KAIROX_BUILD_TIME` via their `build.rs`.
//! This module reads them with `option_env!` fallbacks so that library-level
//! compilation (which doesn't run those build scripts) still compiles.

/// Build information embedded at compile time.
pub struct BuildInfo {
    pub version: &'static str,
    pub git_hash: &'static str,
    pub build_time: &'static str,
}

impl BuildInfo {
    /// Construct from compile-time env vars injected by the binary crate's `build.rs`.
    ///
    /// Falls back to `CARGO_PKG_VERSION` / `"dev"` / `"unknown"` when the
    /// `KAIROX_*` env vars are absent (e.g. during IDE analysis or when
    /// compiling agent-core as a library without its binary wrapper).
    pub fn from_env() -> Self {
        Self {
            version: option_env!("KAIROX_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
            git_hash: option_env!("KAIROX_GIT_HASH").unwrap_or("dev"),
            build_time: option_env!("KAIROX_BUILD_TIME").unwrap_or("unknown"),
        }
    }
}

impl std::fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({} {})",
            self.version, self.git_hash, self.build_time
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_never_panics() {
        // In test builds the KAIROX_* vars are not set, so we get fallbacks.
        let info = BuildInfo::from_env();
        // version falls back to CARGO_PKG_VERSION which is the workspace version
        assert!(!info.version.is_empty());
        assert_eq!(info.git_hash, "dev");
        assert_eq!(info.build_time, "unknown");
    }

    #[test]
    fn display_format() {
        let info = BuildInfo {
            version: "0.11.0",
            git_hash: "abc1234",
            build_time: "2026-01-01T00:00:00Z",
        };
        assert_eq!(
            info.to_string(),
            "0.11.0 (abc1234 2026-01-01T00:00:00Z)"
        );
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

Add `pub mod build_info;` to `crates/agent-core/src/lib.rs` after the existing module declarations (after `pub mod task_types;`):

```rust
pub mod build_info;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-core -- build_info`  
Expected: 2 tests pass (`from_env_never_panics`, `display_format`)

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/build_info.rs crates/agent-core/src/lib.rs
git commit -m "feat(core): add BuildInfo module for compile-time version/git-hash/build-time embedding"
```

---

## Task 3: TUI build.rs + Version Banner

**Files:**

- Create: `crates/agent-tui/build.rs`
- Modify: `crates/agent-tui/src/main.rs`

- [ ] **Step 1: Create agent-tui build.rs**

Create `crates/agent-tui/build.rs`:

```rust
fn main() {
    inject_build_info();
}

fn inject_build_info() {
    let version = env!("CARGO_PKG_VERSION").to_string();
    println!("cargo:rustc-env=KAIROX_VERSION={}", version);

    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=KAIROX_GIT_HASH={}", git_hash);

    // Use chrono-free approach: delegate to date command for build timestamp
    let build_time = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=KAIROX_BUILD_TIME={}", build_time);

    // Re-run if git HEAD changes
    println!("cargo:rerun-if-env-changed=KAIROX_VERSION");
}
```

Note: Using `date` command instead of `chrono` to avoid adding a build dependency.

- [ ] **Step 2: Add version banner to TUI main.rs**

In `crates/agent-tui/src/main.rs`, after the `// 1. Setup terminal` block and before `// 2. Check size`, add:

```rust
    eprintln!(
        "Kairox TUI {}",
        agent_core::build_info::BuildInfo::from_env()
    );
```

The insertion point is after `let mut terminal = Terminal::new(backend)?;` (line ~119).

- [ ] **Step 3: Run TUI build to verify**

Run: `cargo build -p agent-tui 2>&1 | tail -5`  
Expected: Build succeeds. Then verify the env vars were set:

Run: `cargo run -p agent-tui 2>&1 | head -3`  
Expected: First line contains "Kairox TUI" with version info. (The TUI will fail to start in non-terminal context, but the eprintln should appear.)

- [ ] **Step 4: Commit**

```bash
git add crates/agent-tui/build.rs crates/agent-tui/src/main.rs
git commit -m "feat(tui): add build-info injection and version banner on startup"
```

---

## Task 4: GUI build.rs + get_build_info Command

**Files:**

- Modify: `apps/agent-gui/src-tauri/build.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`

- [ ] **Step 1: Add build info injection to GUI build.rs**

Replace the contents of `apps/agent-gui/src-tauri/build.rs` with:

```rust
fn main() {
    inject_build_info();
    tauri_build::build()
}

fn inject_build_info() {
    let version = env!("CARGO_PKG_VERSION").to_string();
    println!("cargo:rustc-env=KAIROX_VERSION={}", version);

    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=KAIROX_GIT_HASH={}", git_hash);

    let build_time = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=KAIROX_BUILD_TIME={}", build_time);

    println!("cargo:rerun-if-env-changed=KAIROX_VERSION");
}
```

- [ ] **Step 2: Add BuildInfoResponse and get_build_info command to commands.rs**

Add to `apps/agent-gui/src-tauri/src/commands.rs`, after the existing response structs (after `TaskSnapshotResponse`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BuildInfoResponse {
    pub version: String,
    pub git_hash: String,
    pub build_time: String,
}
```

Add the command function at the end of the file (before the `switch_session_inner` helper):

```rust
#[tauri::command]
#[specta::specta]
pub fn get_build_info() -> BuildInfoResponse {
    let info = agent_core::build_info::BuildInfo::from_env();
    BuildInfoResponse {
        version: info.version.to_string(),
        git_hash: info.git_hash.to_string(),
        build_time: info.build_time.to_string(),
    }
}
```

- [ ] **Step 3: Register get_build_info in lib.rs invoke handler**

In `apps/agent-gui/src-tauri/src/lib.rs`, add `crate::commands::get_build_info` to the `tauri::generate_handler![]` macro. The full handler array becomes:

```rust
.invoke_handler(tauri::generate_handler![
    crate::commands::list_profiles,
    crate::commands::get_profile_info,
    crate::commands::initialize_workspace,
    crate::commands::start_session,
    crate::commands::send_message,
    crate::commands::switch_session,
    crate::commands::get_trace,
    crate::commands::list_sessions,
    crate::commands::resolve_permission,
    crate::commands::query_memories,
    crate::commands::delete_memory,
    crate::commands::list_workspaces,
    crate::commands::rename_session,
    crate::commands::delete_session,
    crate::commands::get_profile_detail,
    crate::commands::restore_workspace,
    crate::commands::get_task_graph,
    crate::commands::cancel_session,
    crate::commands::get_permission_mode,
    crate::commands::get_build_info,
])
```

- [ ] **Step 4: Register BuildInfoResponse in specta.rs**

In `apps/agent-gui/src-tauri/src/specta.rs`:

Add `get_build_info` to the `collect_commands![]` macro:

```rust
.commands(collect_commands![
    list_profiles,
    get_profile_info,
    initialize_workspace,
    start_session,
    send_message,
    list_sessions,
    resolve_permission,
    query_memories,
    delete_memory,
    list_workspaces,
    rename_session,
    delete_session,
    get_profile_detail,
    restore_workspace,
    get_task_graph,
    cancel_session,
    get_permission_mode,
    get_build_info,
])
```

Add `.typ::<BuildInfoResponse>()` after the existing `.typ::<TaskSnapshotResponse>()` line:

```rust
.typ::<TaskSnapshotResponse>()
.typ::<BuildInfoResponse>()
```

Also add the import for `BuildInfoResponse` — it's already in scope via `use crate::commands::*;`.

- [ ] **Step 5: Regenerate TypeScript bindings**

Run: `just gen-types`  
Expected: `commands.ts` and `events.ts` are regenerated with no errors.

- [ ] **Step 6: Verify TypeScript bindings contain get_build_info**

Run: `grep -c "get_build_info" apps/agent-gui/src/generated/commands.ts`  
Expected: At least 1 match

- [ ] **Step 7: Run workspace tests**

Run: `cargo test -p agent-gui-tauri`  
Expected: All existing tests pass

- [ ] **Step 8: Commit**

```bash
git add apps/agent-gui/src-tauri/build.rs apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs apps/agent-gui/src-tauri/src/specta.rs apps/agent-gui/src/generated/
git commit -m "feat(gui): add build-info injection and get_build_info Tauri command"
```

---

## Task 5: release.sh Enhancement

**Files:**

- Modify: `scripts/release.sh`
- Modify: `justfile`

- [ ] **Step 1: Rewrite release.sh with CLI options**

Replace the contents of `scripts/release.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail

# ─── Defaults ───
DRY_RUN=false
SKIP_CHECKS=false
SKIP_BUILD=false
PRERELEASE=false

# ─── Parse args ───
while [[ $# -gt 0 ]]; do
  case $1 in
    --dry-run)      DRY_RUN=true; shift ;;
    --skip-checks)  SKIP_CHECKS=true; shift ;;
    --skip-build)   SKIP_BUILD=true; shift ;;
    --prerelease)   PRERELEASE=true; shift ;;
    -*)             echo "Unknown option: $1"; exit 1 ;;
    *)              VERSION="$1"; shift ;;
  esac
done

if [[ -z "${VERSION:-}" ]]; then
  echo "Usage: $0 <version> [--dry-run] [--skip-checks] [--skip-build] [--prerelease]"
  echo ""
  echo "Options:"
  echo "  --dry-run       Print commands without executing"
  echo "  --skip-checks   Skip format check, lint, and test"
  echo "  --skip-build    Skip GUI build verification"
  echo "  --prerelease    Mark as a prerelease (requires manual GitHub mark)"
  exit 1
fi

TAG="v${VERSION}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# ─── Helper ───
run() {
  if [[ "$DRY_RUN" == true ]]; then
    echo "[DRY RUN] $*"
  else
    "$@"
  fi
}

# ─── Step 1: Checks ───
if [[ "$SKIP_CHECKS" == false ]]; then
  echo "[1/7] Running checks"
  run pnpm run format:check
  run pnpm run lint
  run cargo test --workspace --all-targets
else
  echo "[1/7] Skipping checks (--skip-checks)"
fi

# ─── Step 2: GUI build verification ───
if [[ "$SKIP_BUILD" == false ]]; then
  echo "[2/7] Verifying GUI build"
  run bash -c 'cd "$ROOT/apps/agent-gui" && pnpm run build && pnpm run tauri:build'
else
  echo "[2/7] Skipping GUI build (--skip-build)"
fi

# ─── Step 3: Changelog ───
echo "[3/7] Generating CHANGELOG.md"
if command -v git-cliff &>/dev/null; then
  run git cliff --tag "$TAG" -o CHANGELOG.md
  run pnpm exec prettier --write CHANGELOG.md
else
  echo "⚠️  git-cliff not found. Install it: cargo install git-cliff"
  echo "   Skipping CHANGELOG.md generation. CI will still generate Release Notes."
fi

# ─── Step 4: Commit changelog ───
echo "[4/7] Committing CHANGELOG.md"
run git add CHANGELOG.md
if ! git diff --cached --quiet CHANGELOG.md; then
  run git commit -m "chore(release): update CHANGELOG for $TAG"
else
  echo "  No changelog changes to commit"
fi

# ─── Step 5: Tag ───
echo "[5/7] Creating or updating tag ${TAG}"
run git tag -fa "$TAG" -m "$TAG"

# ─── Step 6: Push ───
echo "[6/7] Pushing main and tag ${TAG}"
run git push origin main
run git push origin "$TAG" -f

# ─── Step 7: Done ───
echo "[7/7] Done. Monitor GitHub Actions for CI / Release workflows."
if [[ "$PRERELEASE" == true ]]; then
  echo "📌 This is a prerelease. Mark the GitHub Release as pre-release manually or via CI."
fi
```

- [ ] **Step 2: Update justfile release recipe**

Replace the `release` recipe and add `release-dry` recipe in the `justfile`:

```just
# Prepare a release (version required, e.g.: just release 0.8.0)
release version *FLAGS:
    scripts/release.sh {{ version }} {{ FLAGS }}

# Dry-run a release to preview commands without executing
release-dry version:
    scripts/release.sh {{ version }} --dry-run
```

- [ ] **Step 3: Verify release.sh is executable**

Run: `ls -la scripts/release.sh`  
Expected: File has execute permission. If not, run: `chmod +x scripts/release.sh`

- [ ] **Step 4: Test dry-run mode**

Run: `scripts/release.sh 99.99.99 --dry-run --skip-checks --skip-build`  
Expected: Prints each step with `[DRY RUN]` prefix without executing anything.

- [ ] **Step 5: Commit**

```bash
git add scripts/release.sh justfile
git commit -m "feat(ci): enhance release.sh with --dry-run, --skip-checks, --skip-build, --prerelease options"
```

---

## Task 6: Unified Artifact Naming in CI

**Files:**

- Modify: `.github/workflows/release-build.yml`

- [ ] **Step 1: Update TUI packaging steps**

In `.github/workflows/release-build.yml`, replace the `tui-build` job's packaging steps. Find and replace the "Package TUI (Linux/macOS)" and "Package TUI (Windows)" steps with:

```yaml
- name: Package TUI (Linux/macOS)
  if: runner.os != 'Windows'
  env:
    KAIROX_OS: ${{ runner.os == 'macOS' && 'macos' || 'linux' }}
    KAIROX_ARCH: ${{ runner.arch == 'ARM64' && 'aarch64' || 'x86_64' }}
  run: |
    mkdir -p dist
    tar -czf "dist/Kairox-TUI-${GITHUB_REF_NAME}-${KAIROX_OS}-${KAIROX_ARCH}.tar.gz" \
      -C target/release agent-tui

- name: Package TUI (Windows)
  if: runner.os == 'Windows'
  env:
    KAIROX_ARCH: x86_64
  shell: pwsh
  run: |
    New-Item -ItemType Directory -Force -Path dist | Out-Null
    Compress-Archive -Path target/release/agent-tui.exe `
      -DestinationPath "dist/Kairox-TUI-${env:GITHUB_REF_NAME}-windows-${env:KAIROX_ARCH}.zip" -Force
```

- [ ] **Step 2: Add assetNamePattern to tauri-action**

In the same file, in the `tauri-build` job, find the `tauri-apps/tauri-action@v0` step and update the `with` block to include `assetNamePattern` and `updaterJsonPreferNsis`:

```yaml
- name: Build and upload Tauri app
  uses: tauri-apps/tauri-action@v0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY || '' }}
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD || '' }}
  with:
    projectPath: apps/agent-gui
    tagName: ${{ github.ref_name }}
    releaseName: "Kairox ${{ github.ref_name }}"
    releaseBody: "See the assets to download this version and install."
    assetNamePattern: Kairox_${{ github.ref_name }}_{platform}_{arch}[ext]
    updaterJsonPreferNsis: true
```

- [ ] **Step 3: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-build.yml'))"`  
Expected: No output (valid YAML)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-build.yml
git commit -m "feat(ci): unify artifact naming — Kairox-TUI and Kairox prefixes with os/arch conventions"
```

---

## Task 7: SHA256 Checksums + Release Notes Enhancement

**Files:**

- Modify: `.github/workflows/release-build.yml`

- [ ] **Step 1: Add checksums job**

In `.github/workflows/release-build.yml`, add a new `checksums` job after `tauri-build`:

```yaml
checksums:
  name: Generate SHA256 checksums
  if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')
  needs: [tui-build, tauri-build]
  runs-on: ubuntu-latest
  steps:
    - name: Download all release assets
      uses: robinraju/release-downloader@v1
      with:
        tag: ${{ github.ref_name }}
        fileName: "*"
        outDirPath: dist

    - name: Generate checksums
      run: |
        cd dist
        sha256sum *.tar.gz *.zip *.dmg *.msi *.AppImage* *.deb 2>/dev/null \
          | sort > ../SHA256SUMS.txt
        echo "--- SHA256SUMS.txt ---"
        cat ../SHA256SUMS.txt

    - name: Upload checksums to release
      uses: softprops/action-gh-release@v3
      with:
        tag_name: ${{ github.ref_name }}
        files: SHA256SUMS.txt
```

- [ ] **Step 2: Enhance release notes with installation table**

In the `publish-release` job, replace the "Generate release notes with git-cliff" step with an enhanced version that appends an installation guide:

```yaml
- name: Generate release notes with git-cliff
  id: notes
  run: |
    TAG="${GITHUB_REF_NAME}"
    # Find the previous tag to set the commit range
    PREV_TAG=$(git tag --sort=-version:refname | grep -A1 "^${TAG}$" | tail -1)
    if [ "$PREV_TAG" = "$TAG" ]; then
      # No previous tag, generate from the beginning
      BODY=$(git cliff --strip header "${TAG}")
    else
      BODY=$(git cliff --strip header "${PREV_TAG}..${TAG}")
    fi
    {
      echo "body<<EOF"
      echo "$BODY"
      echo ""
      echo "---"
      echo ""
      echo "## 📦 Installation"
      echo ""
      echo "| Platform | Download |"
      echo "|----------|----------|"
      echo "| macOS (Apple Silicon) | \`Kairox-${TAG}-macos-aarch64.dmg\` |"
      echo "| Linux (x86_64) | \`Kairox-${TAG}-linux-x86_64.AppImage\` or \`.deb\` |"
      echo "| Windows (x86_64) | \`Kairox-${TAG}-windows-x86_64.msi\` or setup \`.exe\` |"
      echo "| Terminal (any) | \`Kairox-TUI-${TAG}-<os>-<arch>.tar.gz\` |"
      echo ""
      echo "Checksums: see the \`SHA256SUMS.txt\` asset."
      echo "EOF"
    } >> "$GITHUB_OUTPUT"
```

- [ ] **Step 3: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-build.yml'))"`  
Expected: No output (valid YAML)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-build.yml
git commit -m "feat(ci): add SHA256 checksums job and installation guide to release notes"
```

---

## Task 8: Tauri Updater Skeleton (Disabled)

**Files:**

- Modify: `apps/agent-gui/src-tauri/Cargo.toml`
- Modify: `apps/agent-gui/src-tauri/tauri.conf.json`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Add tauri-plugin-updater dependency**

In `apps/agent-gui/src-tauri/Cargo.toml`, add to the `[dependencies]` section:

```toml
tauri-plugin-updater = "2"
```

- [ ] **Step 2: Add updater config to tauri.conf.json**

In `apps/agent-gui/src-tauri/tauri.conf.json`, add the `createUpdaterArtifacts` field inside the `"bundle"` object (after `"macOS"`):

```json
"createUpdaterArtifacts": "v2Compatible"
```

Add a new top-level `"plugins"` section (after the `"app"` section):

```json
"plugins": {
  "updater": {
    "pubkey": "",
    "endpoints": [
      "https://github.com/Z-Only/kairox/releases/latest/download/latest.json"
    ],
    "active": false
  }
}
```

- [ ] **Step 3: Register the updater plugin in lib.rs**

In `apps/agent-gui/src-tauri/src/lib.rs`, add the `.plugin()` call after the `.setup()` block and before `.invoke_handler()`:

```rust
        .plugin(tauri_plugin_updater::Builder::new().build())
```

- [ ] **Step 4: Verify the GUI compiles**

Run: `cargo check -p agent-gui-tauri 2>&1 | tail -5`  
Expected: Build succeeds with no errors

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/Cargo.toml apps/agent-gui/src-tauri/tauri.conf.json apps/agent-gui/src-tauri/src/lib.rs
git commit -m "feat(gui): add Tauri updater plugin skeleton (disabled) for future auto-update support"
```

---

## Task 9: Final Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace --all-targets`  
Expected: All tests pass

- [ ] **Step 2: Run format check**

Run: `pnpm run format:check`  
Expected: No formatting errors

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`  
Expected: No warnings

- [ ] **Step 4: Regenerate and check TypeScript types**

Run: `just check-types`  
Expected: Types are in sync

- [ ] **Step 5: Verify release.sh dry-run**

Run: `scripts/release.sh 99.99.99 --dry-run --skip-checks --skip-build`  
Expected: All 7 steps printed with `[DRY RUN]` prefix

- [ ] **Step 6: Final commit with ROADMAP update**

Update `ROADMAP.md` to check off the completed item:

```markdown
- ✅ Improve packaging outputs and release metadata (updater support)
```

```bash
git add ROADMAP.md
git commit -m "docs: mark packaging and release optimization as complete in ROADMAP"
```
