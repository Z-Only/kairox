# Kairox Native Skills System Design

## Status

Accepted for implementation planning.

## Context

Kairox is a local-first AI agent workbench with a shared Rust core, a Tauri + Vue GUI, and a ratatui TUI. The current architecture is event-sourced and facade-driven:

- `agent-core` owns shared domain types, events, IDs, and the `AppFacade` trait.
- `agent-runtime` owns orchestration, session lifecycle, context assembly integration, tool registration, MCP lifecycle, and permission decisions.
- `agent-config` owns TOML config loading and profile discovery.
- `agent-tools` owns the tool registry, built-in tools, and permission engine.
- `agent-mcp` owns MCP clients and the marketplace catalog pattern.
- `agent-gui` and `agent-tui` consume runtime functionality through facade-style boundaries.

The roadmap has matured through model switching, context management, memory, MCP, marketplace, GUI audit, and pilot E2E coverage. The next recommended capability is a native Skills system: reusable agent instructions that can be discovered, activated, injected into context, and managed locally.

This design intentionally follows the emerging Agent Skills convention rather than inventing a required Kairox-specific manifest format. A skill is a directory containing a required `SKILL.md` file. `skill.toml` is not required for the MVP; it may be introduced later as an optional extension manifest for marketplace or plugin packaging.

## Goals

- Add a local-first Skills system that works without a remote service.
- Use `SKILL.md` as the canonical skill entry point.
- Support built-in, user-level, and workspace-level skills.
- Load lightweight metadata without eagerly injecting full skill content into every prompt.
- Activate skills explicitly or through conservative suggestions.
- Keep all tool execution subject to Kairox's existing `PermissionEngine`.
- Expose Skills through `AppFacade` so GUI and TUI do not depend on runtime internals.
- Emit domain events for skill lifecycle and activation changes.
- Leave room for future marketplace/plugin distribution without blocking the MVP.

## Non-goals

- No remote Skills marketplace in the MVP.
- No automatic installation or update flow in the MVP.
- No signed skill packages in the MVP.
- No automatic execution of scripts stored inside a skill directory.
- No hidden bypass around tool permissions.
- No requirement for `skill.toml` in the MVP.
- No complex ranking model for skill suggestions in the MVP.

## Recommended Approach

Build a new `agent-skills` crate and wire it through the existing facade/runtime/UI architecture.

The MVP should use `SKILL.md` frontmatter for all required metadata. Kairox-specific fields can live under a `kairox` namespace inside the same frontmatter. This keeps Kairox compatible with the broader `SKILL.md` ecosystem while allowing runtime-specific metadata such as activation mode and permission declarations.

Example:

```markdown
---
name: test-driven-rust
description: Use when implementing Rust features or bug fixes that need test-first development.
version: 0.1.0
kairox:
  activation:
    mode: suggest
    keywords:
      - rust
      - test
      - tdd
  permissions:
    tools:
      - fs.read
      - search.ripgrep
    can_request_tools:
      - fs.write
      - patch.apply
---

# Test-driven Rust

Skill instructions go here.
```

## Skill Directory Format

A skill directory has one required file:

```text
skill-name/
└── SKILL.md
```

Optional supporting files may be present:

```text
skill-name/
├── SKILL.md
├── references/
├── scripts/
├── templates/
├── schemas/
└── assets/
```

Rules:

- Directory names should use kebab-case.
- `SKILL.md` is case-sensitive and required.
- `SKILL.md` must begin with YAML frontmatter.
- `name` and `description` are required frontmatter fields.
- `version` is optional for MVP but recommended.
- `kairox` frontmatter fields are optional.
- Supporting files are data and references by default; scripts are never executed automatically.
- `skill.toml` is reserved as a future optional extension manifest, not part of MVP validation.

## Discovery Sources

Skills are discovered from three source layers, ordered by precedence:

1. Built-in skills shipped with Kairox.
2. User skills under a Kairox user config directory.
3. Workspace skills under the active workspace.

Recommended default locations:

```text
builtin:   embedded or packaged with Kairox
user:      ~/.config/kairox/skills/<skill-name>/SKILL.md
workspace: <workspace>/.kairox/skills/<skill-name>/SKILL.md
```

If the same `name` appears in multiple layers, workspace overrides user, and user overrides built-in. The registry should preserve source metadata so UI can explain where the effective skill came from and whether it shadows another skill.

## Core Crate Design

Add `crates/agent-skills`.

Primary responsibilities:

- Discover skill directories from configured roots.
- Parse `SKILL.md` frontmatter.
- Validate required metadata.
- Load full skill content on demand.
- Represent activation and permission declarations.
- Return deterministic registry snapshots for runtime and UI.

Primary types:

```rust
pub struct SkillId(String);
pub struct SkillName(String);

pub enum SkillSourceKind {
    Builtin,
    User,
    Workspace,
}

pub enum SkillActivationMode {
    Manual,
    Suggest,
    Auto,
}

pub struct SkillMetadata {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: SkillSource,
    pub activation: SkillActivation,
    pub permissions: SkillPermissionDeclaration,
}

pub struct SkillDocument {
    pub metadata: SkillMetadata,
    pub body_markdown: String,
}

pub trait SkillRegistry {
    fn list(&self) -> Vec<SkillMetadata>;
    fn get(&self, id: &SkillId) -> Option<SkillMetadata>;
    fn load_document(&self, id: &SkillId) -> Result<SkillDocument>;
}
```

The crate should expose a filesystem-backed registry implementation for runtime use and a small in-memory registry for tests.

## Domain and Facade Design

Extend `agent-core` with mirror DTOs and events so UI crates remain decoupled from `agent-skills` internals.

Facade methods:

```rust
async fn list_skills(&self) -> Result<Vec<SkillView>>;
async fn get_skill(&self, skill_id: SkillId) -> Result<SkillDetail>;
async fn set_skill_activation(&self, request: SetSkillActivationRequest) -> Result<SkillView>;
async fn list_active_skills(&self, session_id: SessionId) -> Result<Vec<ActiveSkillView>>;
async fn activate_skill(&self, request: ActivateSkillRequest) -> Result<ActiveSkillView>;
async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> Result<()>;
```

Domain events:

- `SkillDiscovered`
- `SkillValidationFailed`
- `SkillActivationChanged`
- `SkillActivated`
- `SkillDeactivated`
- `SkillSuggested`

Events should include enough metadata for trace display and debugging, but should not persist full skill bodies by default.

## Runtime Behavior

`LocalRuntime` owns the effective skill registry and session activation state.

Activation modes:

- `Manual`: listed and available, but only injected after explicit activation.
- `Suggest`: runtime may emit suggestions based on metadata keywords and user intent, but does not inject automatically.
- `Auto`: runtime may activate automatically when conservative deterministic matching succeeds.

MVP default should be `Manual` unless the skill frontmatter explicitly declares another mode. `Auto` should be treated conservatively and can be disabled globally by config.

Skill injection should happen during context assembly, after system/base instructions and before session/user conversation content. The injected block should be clearly delimited so the model can distinguish active skill instructions from user input.

Example prompt block:

```text
<active_skills>
<skill name="test-driven-rust" source="workspace">
...SKILL.md body...
</skill>
</active_skills>
```

The runtime should avoid repeatedly injecting large skill content when it is not needed. The MVP can use straightforward session-level active skill state. More advanced token budgeting and summarization can be added later.

## Permission and Safety Model

Skills are instructions, not authority. A skill may request or recommend tool use, but all tool execution must still flow through existing Kairox tool invocation and permission checks.

Rules:

- A skill cannot grant itself permissions.
- Declared permissions are informational and may be used by UI for warnings.
- Tool invocation continues to use `ToolRegistry` and `PermissionEngine`.
- Scripts inside `scripts/` are not executed automatically.
- If a future command executes a skill-provided script, it must be modeled as a normal tool request and gated by permissions.
- Workspace skills should be treated as workspace-controlled content and shown with source labels in UI.
- Remote or downloaded skills are out of scope for MVP.

## Configuration

Extend Kairox config with optional skill settings.

Example:

```toml
[skills]
enabled = true
auto_activate = false
user_dir = "~/.config/kairox/skills"

[skills.activation]
manual = ["test-driven-rust"]
disabled = ["experimental-skill"]
```

Configuration should not be required for the default MVP path. If `[skills]` is absent, Kairox should still load built-in and workspace skills when the feature is enabled by default.

## GUI Design

Add a Skills section to `SettingsView.vue` or a dedicated Skills settings route if the tab becomes too large.

MVP UI capabilities:

- List discovered skills.
- Show name, description, source, activation mode, and validation status.
- Show effective skill when a name is shadowed by a higher-precedence source.
- Show `SKILL.md` detail in a read-only view.
- Enable or disable manual activation defaults.
- Show active skills for the current session.

Follow existing GUI patterns:

- Tauri command in `commands.rs` calls `AppFacade`.
- Specta exports generated TypeScript bindings.
- Pinia store under `apps/agent-gui/src/stores/` manages skills state.
- Components use native HTML and existing shared CSS classes.
- Add E2E mock support if new commands are consumed by Playwright tests.

## TUI Design

The TUI should expose a minimal set of commands before a full skills browser exists.

Suggested MVP commands:

- List skills.
- Show skill detail.
- Activate skill for current session.
- Deactivate skill for current session.

Trace output should include skill activation and suggestion events so terminal users can understand why extra instructions entered the prompt.

## Testing Strategy

Unit tests:

- Parse valid `SKILL.md` frontmatter.
- Reject missing `name` or `description`.
- Reject missing `SKILL.md`.
- Validate source precedence.
- Validate deterministic ordering.
- Validate `kairox.activation` parsing.
- Validate that `skill.toml` is ignored for MVP unless a future extension explicitly enables it.

Runtime tests:

- Registry loads built-in, user, and workspace roots.
- Workspace skill overrides user and built-in skills with the same name.
- Manual activation injects skill content into session context.
- Suggest mode emits `SkillSuggested` without injecting content.
- Deactivation removes skill content from subsequent context assembly.
- Permission declarations do not bypass `PermissionEngine`.

GUI tests:

- Store lists skills through Tauri command mock.
- Settings view renders discovered skills and validation errors.
- Activation mode changes call the expected command.
- E2E mock includes new commands used by the Skills UI.

Type sync:

- Run `just gen-types` after adding facade-facing Tauri commands or event payloads.
- `just check-types` must pass before merge.

## Implementation Phasing

### Phase 1: Core registry

- Add `agent-skills` crate.
- Parse `SKILL.md` frontmatter.
- Implement discovery across configured roots.
- Add tests for validation, precedence, and ordering.

### Phase 2: Core/runtime integration

- Add `agent-core` DTOs, facade methods, and events.
- Wire registry into `LocalRuntime`.
- Add session-level activation state.
- Inject active skill bodies into context assembly.
- Add runtime tests.

### Phase 3: GUI/TUI surfaces

- Add Tauri commands and Specta bindings.
- Add GUI store and Skills settings UI.
- Add basic TUI commands or panels.
- Update mocks and UI tests.

### Phase 4: Polish and future hooks

- Add built-in starter skills if desired.
- Improve validation messages.
- Add source shadowing display.
- Document the skill authoring format.
- Leave optional `skill.toml` support as a future extension.

## Future Extensions

Potential follow-up work:

- Remote skills catalog.
- Signed skill packages.
- Optional `skill.toml` or `plugin.toml` manifest for packaged distribution.
- Skill dependency declarations.
- Per-skill token budgets.
- Skill suggestion ranking based on conversation context.
- Import/export UI.
- Skill marketplace integration alongside the existing MCP marketplace.

## Open Decisions Resolved

- Kairox will use `SKILL.md` as the canonical format.
- `skill.toml` will not be required in the MVP.
- `skill.toml` remains a possible future extension for marketplace/plugin packaging.
- Skills are local-first in the MVP.
- Skills do not bypass the existing permission system.

## Acceptance Criteria

- Users can place a valid `SKILL.md` under the configured skills directory and see it listed in Kairox.
- Users can activate and deactivate a skill for a session.
- Activated skill content is injected into the model context in a clearly delimited block.
- Suggest-mode skills can emit suggestions without automatically changing prompt content.
- Invalid skills produce validation errors visible through the facade/UI.
- Workspace skills override user and built-in skills with the same name.
- Tool permissions still require normal Kairox approval paths.
- GUI and TUI access skill state through facade boundaries, not direct runtime internals.
