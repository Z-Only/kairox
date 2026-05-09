# Native Skills System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Kairox's local-first native Skills MVP using `SKILL.md` as the canonical skill format.

**Architecture:** Add a focused `agent-skills` crate for discovery, validation, and document loading. Expose skills through `agent-core` mirror DTOs/events and implement activation/injection in `agent-runtime`, then wire GUI/TUI surfaces through existing facade and IPC patterns.

**Tech Stack:** Rust workspace crates, Tokio, serde/serde_yaml-compatible parsing through workspace deps to add, Tauri 2 commands, Specta type generation, Vue 3 + Pinia.

---

## File Structure

- Create `crates/agent-skills/Cargo.toml`: crate manifest using workspace versioning.
- Create `crates/agent-skills/src/lib.rs`: public module exports and error/result types.
- Create `crates/agent-skills/src/types.rs`: `SkillId`, `SkillMetadata`, activation/source/permission types.
- Create `crates/agent-skills/src/frontmatter.rs`: parse `SKILL.md` YAML frontmatter and body.
- Create `crates/agent-skills/src/registry.rs`: filesystem and in-memory registries with precedence handling.
- Modify `Cargo.toml`: add `crates/agent-skills` workspace member and `serde_yaml = "0.9"` to `[workspace.dependencies]`.
- Modify `crates/agent-core/src/facade.rs`: add skills DTOs and `AppFacade` methods.
- Modify `crates/agent-core/src/events.rs`: add skill events and `event_type()` arms.
- Modify `crates/agent-core/src/context_types.rs`: add `ContextSource::Skill`.
- Modify `crates/agent-core/src/lib.rs`: export new skills DTOs from the existing `pub use facade::{ ... }` block.
- Modify `crates/agent-runtime/Cargo.toml`: depend on `agent-skills`.
- Create `crates/agent-runtime/src/skills.rs`: runtime conversion, activation state, prompt block helpers.
- Modify `crates/agent-runtime/src/lib.rs`: export the new skills module.
- Modify `crates/agent-runtime/src/facade_runtime.rs`: store skill registry/state and implement facade methods.
- Modify `crates/agent-runtime/src/agent_loop.rs`: pass active skill blocks into context assembly.
- Modify `crates/agent-memory/src/context.rs`: include active skills in assembled context.
- Modify `apps/agent-gui/src-tauri/src/commands.rs`: add skills IPC commands.
- Modify `apps/agent-gui/src-tauri/src/specta.rs`: register skill commands and DTOs.
- Modify `apps/agent-gui/src-tauri/src/lib.rs`: register skill commands in `generate_handler!`.
- Create `apps/agent-gui/src/stores/skills.ts`: Pinia store for list/detail/activation.
- Modify `apps/agent-gui/src/views/SettingsView.vue`: add Skills tab or panel.
- Modify `apps/agent-gui/e2e/tauri-mock.js`: mock new skill commands.
- Modify `crates/agent-tui/src/components/chat.rs`: parse `:skills` and `:skill ...` slash-style commands from chat input.
- Modify `crates/agent-tui/src/components/mod.rs`: add skill command enum variants.
- Modify `crates/agent-tui/src/main.rs`: dispatch skill commands through `AppFacade`.
- Modify `crates/agent-tui/tests/app_logic.rs`: add command dispatch tests for skill commands.

## Task 1: Create `agent-skills` crate with TDD parser tests

**Files:**

- Create: `crates/agent-skills/Cargo.toml`
- Create: `crates/agent-skills/src/lib.rs`
- Create: `crates/agent-skills/src/types.rs`
- Create: `crates/agent-skills/src/frontmatter.rs`
- Modify: `Cargo.toml`

**Files:**
**Files:**

- No planned source modifications. If verification exposes a concrete failure, modify only the file named in that failing command's output and rerun the same command.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_frontmatter_and_body() {
        let raw = r#"---
name: test-driven-rust
description: Use when implementing Rust changes with test-first development.
version: 0.1.0
kairox:
  activation:
    mode: suggest
    keywords:
      - rust
      - tdd
  permissions:
    tools:
      - fs.read
    can_request_tools:
      - fs.write
---

# Test-driven Rust

Write a failing test first.
"#;

        let parsed = parse_skill_markdown(raw).expect("valid skill markdown");

        assert_eq!(parsed.frontmatter.name, "test-driven-rust");
        assert_eq!(
            parsed.frontmatter.description,
            "Use when implementing Rust changes with test-first development."
        );
        assert_eq!(parsed.frontmatter.version.as_deref(), Some("0.1.0"));
        assert_eq!(parsed.body_markdown.trim(), "# Test-driven Rust\n\nWrite a failing test first.");
        assert_eq!(parsed.activation.mode, SkillActivationMode::Suggest);
        assert_eq!(parsed.activation.keywords, vec!["rust", "tdd"]);
        assert_eq!(parsed.permissions.tools, vec!["fs.read"]);
        assert_eq!(parsed.permissions.can_request_tools, vec!["fs.write"]);
    }

    #[test]
    fn rejects_missing_required_name() {
        let raw = "---\ndescription: Use when testing.\n---\n\n# Body\n";
        let error = parse_skill_markdown(raw).expect_err("name is required");
        assert!(error.to_string().contains("name"));
    }

    #[test]
    fn rejects_missing_required_description() {
        let raw = "---\nname: testing\n---\n\n# Body\n";
        let error = parse_skill_markdown(raw).expect_err("description is required");
        assert!(error.to_string().contains("description"));
    }
}
```

- [ ] **Step 2: Run parser tests and verify they fail**

Run: `cargo test -p agent-skills frontmatter -- --nocapture`

Expected: failure because `agent-skills` crate and parsing implementation do not exist yet.

- [ ] **Step 3: Add crate manifest and workspace member**

Create `crates/agent-skills/Cargo.toml`:

```toml
[package]
name = "agent-skills"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

Modify root `Cargo.toml` workspace members and dependencies to include:

```toml
"crates/agent-skills",
```

```toml
serde_yaml = "0.9"
```

- [ ] **Step 4: Implement core types**

Create `crates/agent-skills/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SkillId(String);

impl SkillId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SkillId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceKind {
    Builtin,
    User,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillActivationMode {
    Manual,
    Suggest,
    Auto,
}

impl Default for SkillActivationMode {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkillActivation {
    pub mode: SkillActivationMode,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkillPermissionDeclaration {
    pub tools: Vec<String>,
    pub can_request_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSource {
    pub kind: SkillSourceKind,
    pub root: PathBuf,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: SkillSource,
    pub activation: SkillActivation,
    pub permissions: SkillPermissionDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDocument {
    pub metadata: SkillMetadata,
    pub body_markdown: String,
}
```

- [ ] **Step 5: Implement frontmatter parser**

Create `crates/agent-skills/src/lib.rs`:

```rust
pub mod frontmatter;
pub mod types;

pub use frontmatter::{parse_skill_markdown, ParsedSkillMarkdown};
pub use types::*;

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("SKILL.md must start with YAML frontmatter delimited by ---")]
    MissingFrontmatter,
    #[error("skill frontmatter field `{field}` is required")]
    MissingRequiredField { field: &'static str },
    #[error("failed to parse skill frontmatter: {0}")]
    InvalidFrontmatter(String),
    #[error("I/O error while reading skill: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SkillError>;
```

Implement `parse_skill_markdown` in `frontmatter.rs` with `serde_yaml`. Define serde helper structs for the YAML frontmatter, parse the text between the first two `---` delimiters, return defaults for absent `kairox` fields, and reject missing `name` or `description`.

- [ ] **Step 6: Run tests and verify pass**

Run: `cargo test -p agent-skills frontmatter -- --nocapture`

Expected: all `frontmatter` tests pass.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/agent-skills
git commit -m "feat(runtime): add skill frontmatter parser"
```

## Task 2: Implement filesystem skill registry and precedence

**Files:**

- Create: `crates/agent-skills/src/registry.rs`
- Modify: `crates/agent-skills/src/lib.rs`

- [ ] **Step 1: Write failing registry tests**

Add tests in `registry.rs` that create temporary built-in/user/workspace roots with `SKILL.md` files. Include these cases:

```rust
#[tokio::test]
async fn workspace_skill_overrides_user_and_builtin_with_same_name() {
    let temp = tempfile::tempdir().unwrap();
    let builtin = temp.path().join("builtin");
    let user = temp.path().join("user");
    let workspace = temp.path().join("workspace");

    write_skill(&builtin, "testing", "Builtin description").await;
    write_skill(&user, "testing", "User description").await;
    write_skill(&workspace, "testing", "Workspace description").await;

    let registry = FileSkillRegistry::discover(vec![
        SkillRoot::new(SkillSourceKind::Builtin, builtin.clone()),
        SkillRoot::new(SkillSourceKind::User, user.clone()),
        SkillRoot::new(SkillSourceKind::Workspace, workspace.clone()),
    ])
    .await
    .unwrap();

    let skills = registry.list();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].description, "Workspace description");
    assert_eq!(skills[0].source.kind, SkillSourceKind::Workspace);
}

#[tokio::test]
async fn load_document_returns_body_for_effective_skill() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    write_skill_with_body(&root, "testing", "Use when testing.", "# Testing\n\nDo the work.").await;

    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        root,
    )])
    .await
    .unwrap();

    let metadata = registry.list().into_iter().next().unwrap();
    let document = registry.load_document(&metadata.id).await.unwrap();

    assert_eq!(document.body_markdown.trim(), "# Testing\n\nDo the work.");
}
```

- [ ] **Step 2: Run registry tests and verify fail**

Run: `cargo test -p agent-skills registry -- --nocapture`

Expected: failure because registry implementation is missing.

- [ ] **Step 3: Implement registry trait and filesystem registry**

Implement:

```rust
#[async_trait::async_trait]
pub trait SkillRegistry: Send + Sync {
    fn list(&self) -> Vec<SkillMetadata>;
    fn get(&self, id: &SkillId) -> Option<SkillMetadata>;
    async fn load_document(&self, id: &SkillId) -> Result<SkillDocument>;
}

#[derive(Debug, Clone)]
pub struct SkillRoot {
    pub kind: SkillSourceKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FileSkillRegistry {
    skills: BTreeMap<SkillId, SkillMetadata>,
}
```

Discovery must scan direct children of each root, require `<child>/SKILL.md`, parse it, and use root order as precedence so later roots replace earlier roots for the same skill `name`.

- [ ] **Step 4: Run registry tests and verify pass**

Run: `cargo test -p agent-skills registry -- --nocapture`

Expected: all registry tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-skills
git commit -m "feat(runtime): add filesystem skill registry"
```

## Task 3: Add core skill DTOs, events, and context source

**Files:**

- Modify: `crates/agent-core/src/facade.rs`
- Modify: `crates/agent-core/src/events.rs`
- Modify: `crates/agent-core/src/context_types.rs`
- Modify: `crates/agent-core/tests/event_roundtrip.rs`

- [ ] **Step 1: Write failing core serialization tests**

Add event roundtrip tests for `SkillActivated`, `SkillDeactivated`, and `SkillSuggested` in `crates/agent-core/tests/event_roundtrip.rs`:

```rust
#[test]
fn skill_activated_roundtrips() {
    let event = make_event(EventPayload::SkillActivated {
        skill_id: "workspace:test-driven-rust".into(),
        name: "test-driven-rust".into(),
        source: "workspace".into(),
        activation_mode: "manual".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    let decoded: DomainEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.event_type, "SkillActivated");
    assert_eq!(decoded, event);
}
```

Add a context source test in `context_types.rs`:

```rust
assert_eq!(serde_json::to_value(ContextSource::Skill).unwrap(), "skill");
```

- [ ] **Step 2: Run tests and verify fail**

Run: `cargo test -p agent-core skill -- --nocapture`

Expected: failure because the skill variants do not exist yet.

- [ ] **Step 3: Add facade DTOs**

In `facade.rs`, add mirror DTOs near marketplace DTOs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: String,
    pub activation_mode: String,
    pub keywords: Vec<String>,
    pub tools: Vec<String>,
    pub can_request_tools: Vec<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillDetail {
    pub view: SkillView,
    pub body_markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ActivateSkillRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DeactivateSkillRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ActiveSkillView {
    pub skill_id: String,
    pub name: String,
    pub source: String,
    pub activation_mode: String,
}
```

Add default `AppFacade` methods returning empty values so existing implementors remain source-compatible during incremental work.

- [ ] **Step 4: Add event variants and context source**

Add `ContextSource::Skill` and these `EventPayload` variants:

```rust
SkillDiscovered { skill_id: String, name: String, source: String },
SkillValidationFailed { path: String, error: String },
SkillActivated { skill_id: String, name: String, source: String, activation_mode: String },
SkillDeactivated { skill_id: String, name: String, source: String },
SkillSuggested { skill_id: String, name: String, reason: String },
```

Update `event_type()` with matching arms.

- [ ] **Step 5: Run tests and verify pass**

Run: `cargo test -p agent-core skill -- --nocapture`

Expected: skill event and context source tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core
git commit -m "feat(core): add skill facade types and events"
```

## Task 4: Add runtime skill state and context injection

**Files:**

- Modify: `crates/agent-runtime/Cargo.toml`
- Create: `crates/agent-runtime/src/skills.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/agent_loop.rs`
- Modify: `crates/agent-memory/src/context.rs`
- Create: `crates/agent-runtime/tests/skills.rs` for runtime integration tests.

- [ ] **Step 1: Write failing runtime tests**

Create tests that construct a `LocalRuntime` with an in-memory or filesystem skill registry and verify:

```rust
#[tokio::test]
async fn manual_activation_lists_active_skill_for_session() {
    let runtime = runtime_with_one_workspace_skill("test-driven-rust").await;
    let workspace = runtime.open_workspace("/tmp/kairox-skills-test".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let skill = runtime.list_skills().await.unwrap().remove(0);
    runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            skill_id: skill.id.clone(),
        })
        .await
        .unwrap();

    let active = runtime.list_active_skills(session_id).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "test-driven-rust");
}
```

Add a context assembler unit test that passes `active_skills` and asserts the message list contains `<active_skills>` and `ContextSource::Skill`.

- [ ] **Step 2: Run tests and verify fail**

Run: `cargo test -p agent-runtime skills -- --nocapture`

Expected: failure because runtime skills support is not implemented.

- [ ] **Step 3: Extend `ContextRequest`**

In `crates/agent-memory/src/context.rs`, add:

```rust
pub active_skills: Vec<String>,
```

Insert active skills after system prompt and before tool definitions:

```rust
if !request.active_skills.is_empty() {
    let block = format!(
        "<active_skills>\n{}\n</active_skills>",
        request.active_skills.join("\n")
    );
    let tokens = self.count_tokens(&block);
    sections.push((ContextSource::Skill, block, tokens));
}
```

Update the drop-priority comment and `find_lowest_priority_drop` so `Skill` is high priority: below `System`, above `ToolDefinitions`.

- [ ] **Step 4: Implement runtime skill helpers**

Create `crates/agent-runtime/src/skills.rs` with:

```rust
pub fn render_active_skill_block(name: &str, source: &str, body_markdown: &str) -> String {
    format!(
        "<skill name=\"{}\" source=\"{}\">\n{}\n</skill>",
        name, source, body_markdown
    )
}
```

Add conversion helpers from `agent_skills::SkillMetadata` to `agent_core::SkillView` and `ActiveSkillView`.

- [ ] **Step 5: Store registry and activation state in `LocalRuntime`**

Add fields:

```rust
skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
```

Add builder method:

```rust
pub fn with_skill_registry(mut self, registry: Arc<dyn agent_skills::SkillRegistry>) -> Self {
    self.skill_registry = Some(registry);
    self
}
```

- [ ] **Step 6: Implement `AppFacade` skill methods**

In `impl AppFacade for LocalRuntime`, implement `list_skills`, `get_skill`, `activate_skill`, `deactivate_skill`, and `list_active_skills`. Emit `SkillActivated` and `SkillDeactivated` via existing event append/broadcast helpers if workspace/session IDs are available.

- [ ] **Step 7: Pass active skill blocks into agent loop context**

Extend `AgentLoopDeps` with a reference to active skills and registry, then load active skill documents before calling `ContextAssembler::assemble`. The assembled `ContextRequest` must include rendered skill blocks.

- [ ] **Step 8: Run runtime tests and verify pass**

Run: `cargo test -p agent-runtime skills -- --nocapture`

Expected: runtime skills tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-runtime crates/agent-memory
git commit -m "feat(runtime): activate skills and inject context"
```

## Task 5: Wire production discovery roots

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `crates/agent-tui/src/main.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs` to call the finalized `with_skill_registry` builder in production wiring.

- [ ] **Step 1: Write failing wiring tests or smoke assertions**

Add a runtime unit test for a helper that builds roots from `home`, `workspace`, and optional built-ins:

```rust
#[test]
fn skill_roots_use_user_and_workspace_locations() {
    let home = PathBuf::from("/home/user");
    let workspace = PathBuf::from("/workspace/project");
    let roots = build_default_skill_roots(&home, &workspace);

    assert!(roots.iter().any(|root| root.path.ends_with(".config/kairox/skills")));
    assert!(roots.iter().any(|root| root.path.ends_with(".kairox/skills")));
}
```

- [ ] **Step 2: Run test and verify fail**

Run: `cargo test -p agent-runtime skill_roots -- --nocapture`

Expected: failure because the helper does not exist.

- [ ] **Step 3: Implement discovery root helper**

Implement `build_default_skill_roots(home, workspace)` in `agent-runtime/src/skills.rs`, returning user then workspace roots. Built-ins can be an empty root list until built-in skills are added.

- [ ] **Step 4: Wire GUI startup**

In `apps/agent-gui/src-tauri/src/lib.rs`, after `cwd` and `db_dir` are known, build a filesystem registry:

```rust
let skill_roots = agent_runtime::skills::build_default_skill_roots(&db_dir, &cwd);
let skill_registry = agent_skills::FileSkillRegistry::discover(skill_roots)
    .await
    .expect("Failed to discover skills");
```

Then add `.with_skill_registry(std::sync::Arc::new(skill_registry))` before `.with_builtin_tools(cwd).await`.

- [ ] **Step 5: Wire TUI startup**

In `crates/agent-tui/src/main.rs`, use the same helper with the resolved workspace path and user config directory, then call `with_skill_registry` on `LocalRuntime`.

- [ ] **Step 6: Run targeted compile check**

Run: `cargo check -p agent-runtime -p agent-tui -p agent-gui-tauri`

Expected: compile succeeds.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/src/lib.rs crates/agent-tui/src/main.rs crates/agent-runtime/src/skills.rs
git commit -m "feat(runtime): discover local skill roots"
```

## Task 6: Add GUI IPC commands and generated bindings

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Generated: `apps/agent-gui/src/generated/commands.ts`
- Generated: `apps/agent-gui/src/generated/events.ts`

- [ ] **Step 1: Add failing command compile expectations**

Add command functions in `commands.rs` with `#[tauri::command]` and `#[specta::specta]` signatures for:

```rust
pub async fn list_skills(state: State<'_, GuiState>) -> Result<Vec<agent_core::SkillView>, String>
pub async fn get_skill_detail(state: State<'_, GuiState>, skill_id: String) -> Result<agent_core::SkillDetail, String>
pub async fn activate_skill(state: State<'_, GuiState>, skill_id: String) -> Result<agent_core::ActiveSkillView, String>
pub async fn deactivate_skill(state: State<'_, GuiState>, skill_id: String) -> Result<(), String>
pub async fn list_active_skills(state: State<'_, GuiState>) -> Result<Vec<agent_core::ActiveSkillView>, String>
```

- [ ] **Step 2: Register commands**

Add the commands to `collect_commands![]` in `specta.rs` and `generate_handler![]` in `lib.rs`.

- [ ] **Step 3: Run type generation**

Run: `just gen-types`

Expected: generated TypeScript bindings include the new skill commands and skill event payloads.

- [ ] **Step 4: Run type sync check**

Run: `just check-types`

Expected: no generated binding drift after regeneration.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/specta.rs apps/agent-gui/src-tauri/src/lib.rs apps/agent-gui/src/generated
git commit -m "feat(gui): expose skill commands"
```

## Task 7: Add GUI Skills store and Settings UI

**Files:**

- Create: `apps/agent-gui/src/stores/skills.ts`
- Modify: `apps/agent-gui/src/views/SettingsView.vue`
- Modify: `apps/agent-gui/e2e/tauri-mock.js`
- Add or modify GUI tests under `apps/agent-gui/src/**/__tests__` following existing patterns.

- [ ] **Step 1: Write failing store tests**

Create a test that mocks generated command functions and verifies `loadSkills()` populates state:

```ts
it("loads discovered skills", async () => {
  const store = useSkillsStore();
  await store.loadSkills();
  expect(store.skills[0].name).toBe("test-driven-rust");
});
```

- [ ] **Step 2: Run GUI test and verify fail**

Run: `pnpm --filter agent-gui test -- skills`

Expected: failure because `useSkillsStore` does not exist.

- [ ] **Step 3: Implement `useSkillsStore`**

Create a setup-store with explicit imports because plain `.ts` files are not auto-imported:

```ts
import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  activateSkill,
  deactivateSkill,
  getSkillDetail,
  listActiveSkills,
  listSkills
} from "@/generated/commands";

export const useSkillsStore = defineStore("skills", () => {
  const skills = ref([]);
  const activeSkills = ref([]);
  const selectedSkill = ref(null);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const hasSkills = computed(() => skills.value.length > 0);

  async function loadSkills() {
    loading.value = true;
    error.value = null;
    try {
      skills.value = await listSkills();
      activeSkills.value = await listActiveSkills();
    } catch (caughtError) {
      error.value = caughtError instanceof Error ? caughtError.message : String(caughtError);
    } finally {
      loading.value = false;
    }
  }

  return { skills, activeSkills, selectedSkill, loading, error, hasSkills, loadSkills };
});
```

Use these generated command imports after `just gen-types`: `activateSkill`, `deactivateSkill`, `getSkillDetail`, `listActiveSkills`, and `listSkills`. If `just gen-types` emits different names, rename the Rust command functions to produce these camelCase exports and rerun `just gen-types`.

- [ ] **Step 4: Add Settings Skills panel**

In `SettingsView.vue`, follow the existing settings tab pattern. Add a Skills tab/panel that shows name, description, source, activation mode, validation error, and buttons for activate/deactivate.

- [ ] **Step 5: Update Tauri mock**

In `apps/agent-gui/e2e/tauri-mock.js`, add handlers for `list_skills`, `get_skill_detail`, `activate_skill`, `deactivate_skill`, and `list_active_skills` returning deterministic fake data.

- [ ] **Step 6: Run GUI tests**

Run: `pnpm --filter agent-gui test -- skills`

Expected: skills store/UI tests pass.

- [ ] **Step 7: Run GUI lint for touched files**

Run: `pnpm --filter agent-gui lint`

Expected: lint succeeds without duplicate import warnings.

- [ ] **Step 8: Commit**

```bash
git add apps/agent-gui/src/stores/skills.ts apps/agent-gui/src/views/SettingsView.vue apps/agent-gui/e2e/tauri-mock.js
git commit -m "feat(gui): add skills settings UI"
```

## Task 8: Add minimal TUI skill commands

**Files:**

- Modify after inspecting current command architecture: `crates/agent-tui/src/app.rs`, `crates/agent-tui/src/keybindings.rs`, `crates/agent-tui/src/components/trace.rs`, or command parser files used by the current TUI.
- Tests: `crates/agent-tui/tests/app_logic.rs`.

- [ ] **Step 1: Inspect current TUI command handling**

Read the command parsing and app logic files before editing. Identify where existing commands such as compaction, model switching, or session actions are handled.

- [ ] **Step 2: Write failing TUI tests**

Add tests for these user-visible commands:

```text
:skills
:skill show test-driven-rust
:skill activate test-driven-rust
:skill deactivate test-driven-rust
```

The tests should assert that the app calls facade skill methods and renders a status/trace message.

- [ ] **Step 3: Run TUI tests and verify fail**

Run: `just test-tui`

Expected: failure because skill commands are not implemented.

- [ ] **Step 4: Implement minimal command handling**

Implement only the four commands above. Do not build a full TUI skills browser in the MVP.

- [ ] **Step 5: Run TUI tests and verify pass**

Run: `just test-tui`

Expected: TUI app logic tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-tui
git commit -m "feat(tui): add minimal skill commands"
```

## Task 9: End-to-end verification and cleanup

**Files:**

- No planned source modifications. If verification exposes a concrete failure, modify only the file named in that failing command's output and rerun the same command.

- [ ] **Step 1: Run Rust tests**

Run: `cargo test --workspace --all-targets`

Expected: all Rust tests pass.

- [ ] **Step 2: Run formatting check**

Run: `pnpm run format:check`

Expected: Rust, Markdown, JSON, and web formatting checks pass.

- [ ] **Step 3: Run lint**

Run: `pnpm run lint`

Expected: clippy, oxlint, and stylelint pass.

- [ ] **Step 4: Run type sync**

Run: `just check-types`

Expected: generated TypeScript bindings are in sync.

- [ ] **Step 5: Run GUI tests**

Run: `just test-gui`

Expected: GUI Vitest suite passes.

- [ ] **Step 6: Review skill safety requirements**

Verify in code review that:

- `SKILL.md` is the only required manifest.
- `skill.toml` is not required or parsed for MVP behavior.
- Scripts under `scripts/` are never executed automatically.
- Skill permission declarations are informational only.
- All tool calls still go through `ToolRegistry` and `PermissionEngine`.

- [ ] **Step 7: Final commit if verification changed files**

```bash
git status --short
git add Cargo.toml crates/agent-skills crates/agent-core crates/agent-runtime crates/agent-memory crates/agent-tui apps/agent-gui
git commit -m "test: verify native skills system"
```

Run the commit command only when `git status --short` shows source or generated-file changes from verification fixes. If `git status --short` prints no output, record the clean status in the handoff and do not create an empty commit.
