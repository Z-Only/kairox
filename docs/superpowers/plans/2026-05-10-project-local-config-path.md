# Project-local Config Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move project-local Kairox config discovery from `./kairox.toml` to `./.kairox/config.toml`, update docs/ignore rules, and migrate local config when present.

**Architecture:** Keep config loading unchanged and localize the behavior change to `agent-config` discovery. Add a private path-based discovery helper so unit tests can verify project and user config precedence without mutating process-wide current directory or home directory state. Documentation and ignore rules become thin follow-up changes.

**Tech Stack:** Rust (`agent-config`), TOML config files, Markdown docs, Git ignore rules, `cargo test`.

---

## File Structure

- Modify `crates/agent-config/src/discovery.rs`: replace project-local discovery path and add deterministic unit tests.
- Modify `.gitignore`: ignore `/.kairox/config.toml` while keeping legacy `kairox.toml` ignored.
- Modify `kairox.toml.example`: update setup instructions and discovery order.
- Modify `docs/dev/local-development.md`: update local development config copy command and edit target.
- Local-only migration: move `kairox.toml` to `.kairox/config.toml` if the file exists in the worktree and the destination is absent.

## Task 1: Discovery tests first

**Files:**

- Modify: `crates/agent-config/src/discovery.rs`

- [ ] **Step 1: Replace the existing placeholder test with failing deterministic tests**

In `crates/agent-config/src/discovery.rs`, replace the entire `#[cfg(test)] mod tests` block with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_config(path: &std::path::Path) {
        std::fs::create_dir_all(path.parent().expect("config has parent"))
            .expect("create config parent");
        std::fs::write(path, "[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n")
            .expect("write config");
    }

    #[test]
    fn project_local_config_wins_over_user_config() {
        let project_dir = TempDir::new().expect("project temp dir");
        let home_dir = TempDir::new().expect("home temp dir");
        let project_config = project_dir.path().join(".kairox/config.toml");
        let user_config = home_dir.path().join(".kairox/config.toml");
        write_config(&project_config);
        write_config(&user_config);

        let (path, source) = find_config_from(project_dir.path(), Some(home_dir.path()))
            .expect("project config is discovered");

        assert_eq!(path, project_config);
        assert_eq!(source, ConfigSource::ProjectFile);
    }

    #[test]
    fn user_config_is_used_when_project_config_is_absent() {
        let project_dir = TempDir::new().expect("project temp dir");
        let home_dir = TempDir::new().expect("home temp dir");
        let user_config = home_dir.path().join(".kairox/config.toml");
        write_config(&user_config);

        let (path, source) = find_config_from(project_dir.path(), Some(home_dir.path()))
            .expect("user config is discovered");

        assert_eq!(path, user_config);
        assert_eq!(source, ConfigSource::UserFile);
    }

    #[test]
    fn legacy_project_root_config_is_ignored() {
        let project_dir = TempDir::new().expect("project temp dir");
        let legacy_config = project_dir.path().join("kairox.toml");
        std::fs::write(&legacy_config, "[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n")
            .expect("write legacy config");

        let result = find_config_from(project_dir.path(), None);

        assert!(result.is_none());
    }

    #[test]
    fn no_config_returns_none() {
        let project_dir = TempDir::new().expect("project temp dir");

        let result = find_config_from(project_dir.path(), None);

        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
cargo test -p agent-config discovery
```

Expected: FAIL to compile with an error like `cannot find function find_config_from in this scope`. That is the expected RED because the desired test seam does not exist yet.

## Task 2: Implement project-local discovery

**Files:**

- Modify: `crates/agent-config/src/discovery.rs`

- [ ] **Step 1: Replace discovery implementation with the new path rules**

Replace the top constants and `find_config()` implementation in `crates/agent-config/src/discovery.rs` with:

```rust
//! Configuration file discovery.

use crate::ConfigSource;
use std::path::{Path, PathBuf};

const CONFIG_DIR: &str = ".kairox";
const CONFIG_FILENAME: &str = "config.toml";

/// Find a configuration file by searching in order:
/// 1. Current working directory (`./.kairox/config.toml`)
/// 2. User home directory (`~/.kairox/config.toml`)
///
/// Returns the path and which source it came from, or `None` if no config found.
pub fn find_config() -> Option<(PathBuf, ConfigSource)> {
    let cwd = std::env::current_dir().ok()?;
    find_config_from(&cwd, dirs::home_dir().as_deref())
}

fn find_config_from(cwd: &Path, home: Option<&Path>) -> Option<(PathBuf, ConfigSource)> {
    let project_path = cwd.join(CONFIG_DIR).join(CONFIG_FILENAME);
    if project_path.is_file() {
        return Some((project_path, ConfigSource::ProjectFile));
    }

    if let Some(home_dir) = home {
        let user_path = home_dir.join(CONFIG_DIR).join(CONFIG_FILENAME);
        if user_path.is_file() {
            return Some((user_path, ConfigSource::UserFile));
        }
    }

    None
}
```

Keep the test module from Task 1 below this implementation.

- [ ] **Step 2: Run focused tests and verify GREEN**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
cargo test -p agent-config discovery
```

Expected: PASS for the four discovery tests.

- [ ] **Step 3: Run full crate tests**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
cargo test -p agent-config
```

Expected: PASS for the full `agent-config` crate.

- [ ] **Step 4: Commit discovery change**

Run:

```bash
git add crates/agent-config/src/discovery.rs
git commit -m "feat(config): discover project config under .kairox"
```

## Task 3: Update ignore rules and user-facing setup docs

**Files:**

- Modify: `.gitignore`
- Modify: `kairox.toml.example`
- Modify: `docs/dev/local-development.md`

- [ ] **Step 1: Update `.gitignore`**

Replace the local user config block in `.gitignore`:

```gitignore
# local user config (sensitive — may contain API keys)
kairox.toml
```

with:

```gitignore
# local user config (sensitive — may contain API keys)
kairox.toml
/.kairox/config.toml
```

- [ ] **Step 2: Update `kairox.toml.example` header**

In `kairox.toml.example`, replace the current header instructions:

```text
# Copy this file to `kairox.toml` (project root) or `~/.kairox/config.toml`
# and fill in your profiles. The `kairox.toml` file is git-ignored.
#
# Discovery order:
#   1. ./kairox.toml          (project-level, takes priority)
#   2. ~/.kairox/config.toml  (user-level fallback)
#   3. Built-in defaults      (fake + local-code, plus "fast" if OPENAI_API_KEY is set)
```

with:

```text
# Copy this file to `.kairox/config.toml` (project root) or `~/.kairox/config.toml`
# and fill in your profiles. The `.kairox/config.toml` file is git-ignored.
#
# Discovery order:
#   1. ./.kairox/config.toml  (project-level, takes priority)
#   2. ~/.kairox/config.toml  (user-level fallback)
#   3. Built-in defaults      (fake + local-code, plus "fast" if OPENAI_API_KEY is set)
```

- [ ] **Step 3: Update local development docs**

In `docs/dev/local-development.md`, replace:

```bash
cp kairox.toml.example kairox.toml
cp .env.example .env
# Edit kairox.toml to choose model profiles
# Edit .env to set OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.
```

with:

```bash
mkdir -p .kairox
cp kairox.toml.example .kairox/config.toml
cp .env.example .env
# Edit .kairox/config.toml to choose model profiles
# Edit .env to set OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.
```

- [ ] **Step 4: Verify docs no longer recommend the legacy project path**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
grep -R "cp kairox.toml.example kairox.toml\|./kairox.toml\|Copy this file to \`kairox.toml\`" kairox.toml.example docs/dev crates/agent-config/src/discovery.rs || true
```

Expected: no output. If output remains, inspect it and update only references that still recommend or implement the legacy project-local path.

- [ ] **Step 5: Commit docs and ignore changes**

Run:

```bash
git add .gitignore kairox.toml.example docs/dev/local-development.md
git commit -m "docs(config): document .kairox project config"
```

## Task 4: Migrate local config when present

**Files:**

- Local-only: `kairox.toml`
- Local-only: `.kairox/config.toml`

- [ ] **Step 1: Inspect whether a local config exists in this worktree**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
if [ -f kairox.toml ]; then echo "legacy config exists"; else echo "legacy config missing"; fi
if [ -f .kairox/config.toml ]; then echo "new config exists"; else echo "new config missing"; fi
```

Expected: The command reports whether migration is needed. Because both paths are ignored local config, either result is acceptable.

- [ ] **Step 2: Move legacy config only when safe**

If `kairox.toml` exists and `.kairox/config.toml` does not exist, run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
mkdir -p .kairox
cp kairox.toml /tmp/kairox.toml.before-migration
mv kairox.toml .kairox/config.toml
cmp /tmp/kairox.toml.before-migration .kairox/config.toml
rm /tmp/kairox.toml.before-migration
```

Expected: `cmp` exits 0, proving the contents were preserved.

If both files exist, do not overwrite `.kairox/config.toml`; report the conflict and ask the user how to merge them. If `kairox.toml` is absent, skip this step.

- [ ] **Step 3: Confirm ignored local config is not staged**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
git status --short --ignored .kairox/config.toml kairox.toml | cat
```

Expected: `.kairox/config.toml` appears as ignored (`!!`) when present, and no local config is staged.

## Task 5: Final verification and cleanup

**Files:**

- Verify: `crates/agent-config/src/discovery.rs`
- Verify: `.gitignore`
- Verify: `kairox.toml.example`
- Verify: `docs/dev/local-development.md`

- [ ] **Step 1: Run focused tests**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
cargo test -p agent-config discovery
```

Expected: PASS.

- [ ] **Step 2: Run full crate tests**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
cargo test -p agent-config
```

Expected: PASS.

- [ ] **Step 3: Run formatting check for touched docs and Rust code**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
cargo fmt --all --check
pnpm exec oxfmt --check kairox.toml.example docs/dev/local-development.md docs/superpowers/specs/2026-05-10-project-local-config-path-design.md docs/superpowers/plans/2026-05-10-project-local-config-path.md
```

Expected: both commands pass. If formatting fails, run `cargo fmt --all` or `pnpm exec oxfmt --write <file>` on the failing files, then repeat the checks.

- [ ] **Step 4: Check final Git status**

Run:

```bash
cd /Users/chanyu/AIProjects/kairox/.worktrees/feat-project-local-config-path
git status --short --branch | cat
```

Expected: clean except for intentionally ignored local config files.

- [ ] **Step 5: Request code review**

Use the `requesting-code-review` skill after implementation and verification pass. Ask the reviewer to verify:

- `./kairox.toml` is no longer discovered.
- `./.kairox/config.toml` takes priority over `~/.kairox/config.toml`.
- `.gitignore` ignores only local config, not the entire `.kairox/` directory.
- Documentation no longer recommends the old project-local config path.
