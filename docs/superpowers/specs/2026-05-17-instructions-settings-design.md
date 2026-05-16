# Instructions Settings — Design Spec

Date: 2026-05-17

## Summary

Add a new "Instructions" tab in the settings page to configure User-level and Project-level system instructions. These get concatenated with the built-in System prompt at runtime (System → User → Project), forming the full system prompt sent to the model.

## Motivation

Currently the system prompt is entirely hardcoded in Rust (`agent-runtime/src/agent_loop/mod.rs`). Users cannot customize assistant behavior beyond what the built-in prompt provides. A layered instructions system lets users set global preferences (language, style) at User level and project-specific context (tech stack, conventions) at Project level.

## Data Model

### TOML storage

A single optional `instructions` key at the top level of existing config files:

**`~/.kairox/config.toml`** (User level):

```toml
instructions = "用中文回复，简洁风格..."
```

**`.kairox/config.toml`** (Project level):

```toml
instructions = "这是一个 Rust + Vue 项目..."
```

Empty or absent `instructions` = no custom instructions for that layer.

### Rust types

**`agent-config/src/lib.rs`** — add to `ConfigToml`, `Config`:

```rust
pub instructions: Option<String>,
```

**`agent-core/src/facade/settings.rs`** — new Specta DTOs:

```rust
#[derive(Serialize, Deserialize, specta::Type)]
pub struct InstructionsView {
    pub system: String,
    pub user: Option<String>,
    pub project: Option<String>,
}

#[derive(Serialize, Deserialize, specta::Type)]
pub struct InstructionsUpdateInput {
    pub scope: ConfigScope,
    pub text: String,  // empty = remove
}
```

### Merge logic (`agent-config/src/lib.rs`)

Layer priority low → high: System → User → Project.

```rust
let mut parts: Vec<String> = Vec::new();
parts.push(SYSTEM_PROMPT.to_string());  // always first
if let Some(ref u) = user_instructions { parts.push(u.clone()); }
if let Some(ref p) = project_instructions { parts.push(p.clone()); }
let merged = parts.join("\n\n");
```

## UI Design

### Tab placement

Sixth tab "Instructions" in SettingsLayout.vue, positioned between General and Models.

### Component: `InstructionsSettings.vue`

- Shows `ConfigSourceBar` at top (User/Project toggle, with project picker for Project scope).
- **System Instructions**: read-only textarea with grey background.
- **User Instructions**: editable textarea. Visible in both scopes (editable in User, read-only in Project).
- **Project Instructions**: editable textarea. Only visible when Project scope selected.
- **Effective Preview**: collapsible section showing concatenated result.

### User scope view

```
[ConfigSourceBar: User | Project]

System Instructions (read-only, grey)
┌──────────────────────────────────────┐
│ You are Kairox, a helpful AI...      │
└──────────────────────────────────────┘

User Instructions (editable)
┌──────────────────────────────────────┐
│ 用中文回复，简洁风格...               │
└──────────────────────────────────────┘
Applies to ALL conversations

Effective Preview
┌──────────────────────────────────────┐
│ [System] → [User] → [Project: empty] │
└──────────────────────────────────────┘
```

### Project scope view

```
[ConfigSourceBar: User | Project ▼]

System Instructions (read-only, grey)
┌──────────────────────────────────────┐
│ You are Kairox, a helpful AI...      │
└──────────────────────────────────────┘

User Instructions (read-only, grey — from global config)
┌──────────────────────────────────────┐
│ 用中文回复，简洁风格...               │
└──────────────────────────────────────┘

Project Instructions (editable)
┌──────────────────────────────────────┐
│ 这是一个 Rust + Vue 项目...           │
└──────────────────────────────────────┘
Applies to this project only

Effective Preview
┌──────────────────────────────────────┐
│ [System] → [User] → [Project]        │
└──────────────────────────────────────┘
```

## Frontend Changes

| File                                              | Change                                          |
| ------------------------------------------------- | ----------------------------------------------- |
| `SettingsLayout.vue`                              | Add 6th tab button, update `activeTab` computed |
| `router/routes.ts`                                | Add `/settings/instructions` route              |
| `locales/en.json`, `locales/zh-CN.json`           | Add i18n keys                                   |
| **NEW** `views/settings/InstructionsSettings.vue` | Main component                                  |
| **NEW** or inline state                           | Tauri command bindings for get/upsert           |

## Backend Changes

| Crate             | File                               | Change                                                                           |
| ----------------- | ---------------------------------- | -------------------------------------------------------------------------------- |
| `agent-config`    | `lib.rs`                           | Add `instructions` to `ConfigToml`, `Config`; update merge                       |
| `agent-config`    | `loader.rs`                        | Add `instructions` to `ConfigToml` struct                                        |
| `agent-core`      | `facade/settings.rs`               | Add `InstructionsView`, `InstructionsUpdateInput`                                |
| `agent-runtime`   | **NEW** `instructions_settings.rs` | I/O: read/write instructions from TOML files                                     |
| `agent-runtime`   | `agent_loop/runner.rs`             | Use merged instructions instead of bare `SYSTEM_PROMPT`                          |
| `agent-gui-tauri` | `lib.rs`                           | Register `get_instructions`, `upsert_instructions`, `get_system_prompt` commands |
| `agent-gui-tauri` | `specta.rs`                        | Register new command types                                                       |

### Tauri commands

```rust
#[tauri::command]
async fn get_instructions(
    source: ConfigScope,
    project_id: Option<String>,
) -> Result<InstructionsView, String>;

#[tauri::command]
async fn upsert_instructions(
    input: InstructionsUpdateInput,
    project_id: Option<String>,
) -> Result<(), String>;

#[tauri::command]
async fn get_system_prompt() -> String;
```

## Runtime Behavior

After loading config (which merges instructions across layers), the runner passes the merged string as the system prompt via `ContextRequest.system_prompt`. No change to context assembly priority (P0, never dropped).

The hardcoded `SYSTEM_PROMPT` constant remains as the System layer foundation. It is always included.

## Testing

- **Rust unit**: `instructions_settings.rs` — read/write/empty/remove round-trip
- **Rust integration**: config merge with various combinations of User + Project instructions
- **GUI Vitest**: InstructionsSettings component mount, textarea bindings, save flow
- **Playwright E2E**: navigate to Instructions tab, edit User instructions, switch to Project scope, verify preview

## Non-goals

- Session/Task-level transient instructions (out of scope for this change)
- Per-profile instructions (instructions are cross-profile, per-config-layer)
- Rich text or template variables in instructions
- Instruction file discovery (CLAUDE.md already handled separately via `project_instructions`)
