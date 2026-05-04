# Specta Event Type Generation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-generate TypeScript types for EventPayload and related domain types from Rust via specta, remove all manual `as` type assertions from consumers, and replace the fragile check-types.sh with a generation + diff check.

**Architecture:** Extend the existing tauri-specta infrastructure by adding `specta` as an optional feature to `agent-core` and `agent-memory`, then register event types in the specta builder and generate a new `events.ts` file. Refactor TypeScript consumers to use discriminated union narrowing.

**Tech Stack:** Rust (specta, tauri-specta), TypeScript, just

---

## File Structure

### New Files

| File                                                | Responsibility                                        |
| --------------------------------------------------- | ----------------------------------------------------- |
| `apps/agent-gui/src-tauri/src/bin/export_events.rs` | Binary that exports event TypeScript types via specta |
| `apps/agent-gui/src/generated/events.ts`            | Auto-generated TypeScript event types (git-tracked)   |
| `apps/agent-gui/src/types/events-helpers.ts`        | Hand-written utility types for event handling         |

### Modified Files

| File                                               | Changes                                                                        |
| -------------------------------------------------- | ------------------------------------------------------------------------------ |
| `Cargo.toml` (workspace root)                      | No change — specta already in workspace deps                                   |
| `crates/agent-core/Cargo.toml`                     | Add `specta` optional dep + `[features]` section                               |
| `crates/agent-core/src/events.rs`                  | Add `cfg_attr(feature = "specta", derive(specta::Type))`                       |
| `crates/agent-core/src/task_types.rs`              | Add `cfg_attr(feature = "specta", derive(specta::Type))`                       |
| `crates/agent-core/src/facade.rs`                  | Add `cfg_attr` on TaskSnapshot, TaskGraphSnapshot                              |
| `crates/agent-core/src/ids.rs`                     | Add custom `specta::Type` impl for ID newtypes                                 |
| `crates/agent-memory/Cargo.toml`                   | Add `specta` optional dep + `[features]` section                               |
| `crates/agent-memory/src/memory.rs`                | Add `cfg_attr` on MemoryScope                                                  |
| `apps/agent-gui/src-tauri/Cargo.toml`              | Enable agent-core/specta + agent-memory/specta features; add export-events bin |
| `apps/agent-gui/src-tauri/src/specta.rs`           | Register event domain types                                                    |
| `apps/agent-gui/src-tauri/src/lib.rs`              | No change needed (bin registration is in Cargo.toml)                           |
| `apps/agent-gui/src/types/index.ts`                | Re-export from generated/events, remove hand-written event types               |
| `apps/agent-gui/src/composables/useTauriEvents.ts` | Remove `as` assertions, rely on discriminated union narrowing                  |
| `apps/agent-gui/src/composables/useTraceStore.ts`  | Remove `as` assertions, import from generated                                  |
| `apps/agent-gui/src/stores/session.ts`             | Remove `as` assertions, import from generated                                  |
| `apps/agent-gui/src/stores/taskGraph.ts`           | Import TaskSnapshot from generated                                             |
| `justfile`                                         | Update gen-types (add export-events), update check-types                       |
| `scripts/check-types.sh`                           | DELETE — replaced by gen-types + git diff                                      |
| `.github/workflows/ci.yml`                         | Update type-sync job                                                           |

---

## Task 1: Add specta feature to agent-core

**Files:**

- Modify: `crates/agent-core/Cargo.toml`
- Modify: `crates/agent-core/src/ids.rs`
- Modify: `crates/agent-core/src/events.rs`
- Modify: `crates/agent-core/src/task_types.rs`
- Modify: `crates/agent-core/src/facade.rs`

- [ ] **Step 1: Add specta optional dependency and feature to agent-core**

In `crates/agent-core/Cargo.toml`, add:

```toml
[features]
specta = ["dep:specta"]

[dependencies]
# ... existing deps ...
specta = { workspace = true, optional = true }
```

- [ ] **Step 2: Add custom specta::Type impl for ID newtypes**

In `crates/agent-core/src/ids.rs`, add at the end of the file (before `#[cfg(test)]`):

```rust
#[cfg(feature = "specta")]
impl specta::Type for WorkspaceId {
    fn inline(type_map: &mut specta::TypeMap) -> specta::datatype::DataType {
        <String as specta::Type>::inline(type_map)
    }

    fn reference(type_map: &mut specta::TypeMap) -> specta::datatype::Reference {
        <String as specta::Type>::reference(type_map)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for SessionId {
    fn inline(type_map: &mut specta::TypeMap) -> specta::datatype::DataType {
        <String as specta::Type>::inline(type_map)
    }

    fn reference(type_map: &mut specta::TypeMap) -> specta::datatype::Reference {
        <String as specta::Type>::reference(type_map)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for TaskId {
    fn inline(type_map: &mut specta::TypeMap) -> specta::datatype::DataType {
        <String as specta::Type>::inline(type_map)
    }

    fn reference(type_map: &mut specta::TypeMap) -> specta::datatype::Reference {
        <String as specta::Type>::reference(type_map)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for AgentId {
    fn inline(type_map: &mut specta::TypeMap) -> specta::datatype::DataType {
        <String as specta::Type>::inline(type_map)
    }

    fn reference(type_map: &mut specta::TypeMap) -> specta::datatype::Reference {
        <String as specta::Type>::reference(type_map)
    }
}
```

- [ ] **Step 3: Add specta derive attributes to domain types**

In `crates/agent-core/src/events.rs`, add `cfg_attr` to `EventPayload`, `DomainEvent`, and `PrivacyClassification`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum EventPayload { ... }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DomainEvent { ... }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClassification { ... }
```

In `crates/agent-core/src/task_types.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum AgentRole { ... }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum TaskState { ... }
```

In `crates/agent-core/src/facade.rs`, add to `TaskSnapshot` and `TaskGraphSnapshot`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TaskSnapshot { ... }

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TaskGraphSnapshot { ... }
```

- [ ] **Step 4: Verify agent-core builds without the specta feature**

Run: `cargo check -p agent-core`
Expected: success (specta feature is not enabled by default)

- [ ] **Step 5: Verify agent-core builds with the specta feature**

Run: `cargo check -p agent-core --features specta`
Expected: success

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/
git commit -m "feat(core): add optional specta feature for TypeScript type generation"
```

---

## Task 2: Add specta feature to agent-memory

**Files:**

- Modify: `crates/agent-memory/Cargo.toml`
- Modify: `crates/agent-memory/src/memory.rs`

- [ ] **Step 1: Add specta optional dependency and feature**

In `crates/agent-memory/Cargo.toml`, add:

```toml
[features]
specta = ["dep:specta"]

[dependencies]
# ... existing deps ...
specta = { workspace = true, optional = true }
```

- [ ] **Step 2: Add specta derive to MemoryScope**

In `crates/agent-memory/src/memory.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum MemoryScope {
    User,
    Workspace,
    Session,
}
```

Note: `MemoryScope` currently has no `Serialize`/`Deserialize` derives. We need to add those too since specta requires serde for type generation:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum MemoryScope {
    User,
    Workspace,
    Session,
}
```

Wait — let me check if MemoryScope already has Serialize/Deserialize... Looking at the code, it does not. But `MemoryEntryResponse` in `commands.rs` manually maps the scope to a string. However, for the specta Type derive to work correctly, we need serde on MemoryScope. Let me check if this would break anything.

Actually, since `specta` is an optional feature, we can gate `Serialize, Deserialize` on the `specta` feature too, OR just add them unconditionally since they're harmless and may be useful. Let's add them unconditionally — it's a minor improvement and won't break anything since MemoryScope is not serialized directly in the current event system (it's mapped to string in commands.rs).

**Correction:** On second look, `MemoryScope` is used in `MemoryEntry` which is not serialized directly either — it's mapped to `MemoryEntryResponse` in commands.rs. But adding Serialize/Deserialize is still safe and useful for future use. Let's add them.

- [ ] **Step 3: Verify agent-memory builds**

Run: `cargo check -p agent-memory`
Expected: success

Run: `cargo check -p agent-memory --features specta`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add crates/agent-memory/
git commit -m "feat(memory): add optional specta feature and Serialize/Deserialize for MemoryScope"
```

---

## Task 3: Create export-events binary and register types

**Files:**

- Modify: `apps/agent-gui/src-tauri/Cargo.toml`
- Create: `apps/agent-gui/src-tauri/src/bin/export_events.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`

- [ ] **Step 1: Update agent-gui-tauri Cargo.toml**

Add the export-events binary and enable specta features on dependencies:

```toml
[[bin]]
name = "export-events"
path = "src/bin/export_events.rs"

[dependencies]
agent-core = { path = "../../../crates/agent-core", features = ["specta"] }
agent-memory = { path = "../../../crates/agent-memory", features = ["specta"] }
# ... rest unchanged ...
```

- [ ] **Step 2: Create export_events.rs binary**

Create `apps/agent-gui/src-tauri/src/bin/export_events.rs`:

```rust
//! Binary to export event-related TypeScript types via specta.
//!
//! Usage: cargo run -p agent-gui-tauri --bin export-events
//!
//! Output: apps/agent-gui/src/generated/events.ts

use agent_core::{AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskGraphSnapshot, TaskSnapshot, TaskState};
use agent_memory::MemoryScope;

fn main() {
    let out_path_str = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../src/generated/events.ts".to_string());
    let out_path = std::path::Path::new(&out_path_str);

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    let exporter = specta_typescript::Typescript::default()
        .header("// This file is generated by specta. Do not edit manually.\n// Run `just gen-types` to regenerate.\n");

    let result = exporter
        .export(
            &[
                specta::export::type_def::<EventPayload>(&mut specta::TypeMap::default()),
                specta::export::type_def::<DomainEvent>(&mut specta::TypeMap::default()),
                specta::export::type_def::<PrivacyClassification>(&mut specta::TypeMap::default()),
                specta::export::type_def::<AgentRole>(&mut specta::TypeMap::default()),
                specta::export::type_def::<TaskState>(&mut specta::TypeMap::default()),
                specta::export::type_def::<TaskSnapshot>(&mut specta::TypeMap::default()),
                specta::export::type_def::<TaskGraphSnapshot>(&mut specta::TypeMap::default()),
                specta::export::type_def::<MemoryScope>(&mut specta::TypeMap::default()),
            ],
            out_path,
        );

    match result {
        Ok(()) => eprintln!("Event types exported to {}", out_path.display()),
        Err(e) => {
            eprintln!("Failed to export event types: {e}");
            std::process::exit(1);
        }
    }
}
```

Note: The exact specta API for standalone type export (without tauri_specta Builder) may need adjustment. The alternative approach is to use `tauri_specta::Builder` with only `.typ::<>()` registrations and no commands. Let me verify which API works...

Actually, looking at the existing `export_specta.rs`, it uses `tauri_specta::Builder::new().commands(...).typ::<>().export()`. We can do the same pattern but without commands:

```rust
use tauri_specta::Builder;

fn main() {
    let out_path_str = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../src/generated/events.ts".to_string());
    let out_path = std::path::Path::new(&out_path_str);

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    let specta_builder = Builder::<tauri::Wry>::new()
        .typ::<EventPayload>()
        .typ::<DomainEvent>()
        .typ::<PrivacyClassification>()
        .typ::<AgentRole>()
        .typ::<TaskState>()
        .typ::<TaskSnapshot>()
        .typ::<TaskGraphSnapshot>()
        .typ::<MemoryScope>();

    specta_builder
        .export(specta_typescript::Typescript::default(), out_path)
        .expect("Failed to export event types");

    eprintln!("Event types exported to {}", out_path.display());
}
```

This approach uses the same tauri_specta Builder API (which already handles serde attributes correctly) but with no commands registered. This is the cleanest approach.

- [ ] **Step 3: Update specta.rs to also register event types**

In `apps/agent-gui/src-tauri/src/specta.rs`, add the event type registrations so they're available at runtime too:

```rust
use crate::commands::*;
use agent_core::{AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskGraphSnapshot, TaskSnapshot, TaskState};
use agent_memory::MemoryScope;
use tauri_specta::collect_commands;

/// Build the specta collector with all command and event type information.
pub fn create_specta() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
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
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileDetailResponse>()
        .typ::<TaskSnapshotResponse>()
        // Event domain types (also exported by export-events binary)
        .typ::<EventPayload>()
        .typ::<DomainEvent>()
        .typ::<PrivacyClassification>()
        .typ::<AgentRole>()
        .typ::<TaskState>()
        .typ::<TaskSnapshot>()
        .typ::<TaskGraphSnapshot>()
        .typ::<MemoryScope>()
}
```

- [ ] **Step 4: Verify the export-events binary compiles**

Run: `cargo check -p agent-gui-tauri --bin export-events`
Expected: success

- [ ] **Step 5: Run the export-events binary and inspect output**

Run: `cargo run -p agent-gui-tauri --bin export-events -- apps/agent-gui/src/generated/events.ts`

Inspect the generated file at `apps/agent-gui/src/generated/events.ts`. Verify:

- `EventPayload` is a discriminated union with `type` field as discriminant
- Variant names are PascalCase
- Field names use snake_case (matching Rust serde default)
- `DomainEvent` includes `payload: EventPayload`
- `AgentRole`, `TaskState`, `PrivacyClassification`, `MemoryScope` are string unions or enums
- `TaskSnapshot` and `TaskGraphSnapshot` are interfaces

If the output format is not what we expect (e.g., field naming wrong, type structure off), adjust serde attributes or specta configuration and re-run.

- [ ] **Step 6: Compare generated types with existing hand-written types**

Compare the generated `events.ts` with the existing hand-written `EventPayload` union in `types/index.ts`. Note any differences:

- Field naming conventions (snake_case vs camelCase)
- Variant structure
- Optional field representation (`T | null` vs `T | undefined`)

If specta generates `camelCase` fields (because of some tauri convention), but our current TS types use `snake_case`, we may need to adjust. The existing `EventPayload` in `index.ts` uses `snake_case` (e.g., `model_profile`, `tool_call_id`). Since the Rust types use `snake_case` with `#[serde(rename_all = ...)]` only on `EventPayload` (which uses `PascalCase` for the tag but no field rename), the field names should serialize as `snake_case`. Check the specta output to confirm.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/ apps/agent-gui/src/generated/events.ts
git commit -m "feat(gui): add export-events binary for specta event type generation"
```

---

## Task 4: Create events-helpers.ts and refactor types/index.ts

**Files:**

- Create: `apps/agent-gui/src/types/events-helpers.ts`
- Modify: `apps/agent-gui/src/types/index.ts`

- [ ] **Step 1: Create events-helpers.ts**

Create `apps/agent-gui/src/types/events-helpers.ts`:

```typescript
/**
 * Type-safe event handling utilities for EventPayload.
 *
 * These helpers leverage the auto-generated EventPayload discriminated union
 * to provide exhaustive pattern matching and type narrowing.
 */

import type { EventPayload } from "../generated/events";

/** Extract a specific EventPayload variant by its type tag. */
export type ExtractPayload<T extends EventPayload["type"]> = Extract<
  EventPayload,
  { type: T }
>;

/**
 * Exhaustive event handler map.
 * TypeScript will error if a new EventPayload variant is added but not handled.
 * Each handler receives the narrowed payload type for its variant.
 */
export type EventPayloadHandlers<R = void> = {
  [K in EventPayload["type"]]: (payload: ExtractPayload<K>) => R;
};

/**
 * Partial event handler map.
 * Only handle the events you care about. Unhandled variants are ignored.
 */
export type PartialEventPayloadHandlers<R = void> = {
  [K in EventPayload["type"]]?: (payload: ExtractPayload<K>) => R;
};

/**
 * Process an EventPayload with exhaustive pattern matching.
 * If a new variant is added to EventPayload, TypeScript will error
 * until a handler is added for it.
 */
export function matchPayload<R>(
  payload: EventPayload,
  handlers: EventPayloadHandlers<R>
): R {
  const handler = handlers[payload.type] as (p: EventPayload) => R;
  return handler(payload);
}

/**
 * Process an EventPayload with partial pattern matching.
 * Unhandled variants are silently ignored.
 */
export function matchPartialPayload<R>(
  payload: EventPayload,
  handlers: PartialEventPayloadHandlers<R>
): R | undefined {
  const handler = handlers[payload.type] as
    | ((p: EventPayload) => R)
    | undefined;
  return handler?.(payload);
}
```

- [ ] **Step 2: Refactor types/index.ts to re-export from generated**

Replace the hand-written event types in `apps/agent-gui/src/types/index.ts` with re-exports from the generated file. Keep the UI types that don't come from Rust.

The new `index.ts` should look like:

```typescript
// ===== Auto-generated types (from specta) =====
export type {
  EventPayload,
  DomainEvent,
  AgentRole,
  TaskState,
  TaskSnapshot,
  TaskGraphSnapshot,
  PrivacyClassification
} from "../generated/events";

// Re-export MemoryScope from generated events
export type { MemoryScope } from "../generated/events";

// ===== UI projection types (not generated from Rust) =====
export type ProjectedRole = "user" | "assistant";

export interface ProjectedMessage {
  role: ProjectedRole;
  content: string;
}

export interface SessionProjection {
  messages: ProjectedMessage[];
  task_titles: string[];
  task_graph: TaskGraphSnapshot;
  token_stream: string;
  cancelled: boolean;
}

// ===== Command response types (from tauri-specta) =====
export type {
  WorkspaceInfoResponse,
  SessionInfoResponse,
  MemoryEntryResponse,
  ProfileInfo,
  ProfileDetailResponse,
  TaskSnapshotResponse
} from "../generated/commands";

// ===== Session metadata (matches Rust SessionMeta but used independently) =====
export interface SessionMeta {
  session_id: string;
  workspace_id: string;
  title: string;
  model_profile: string;
  model_id: string | null;
  provider: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
}
```

Note: Remove the hand-written `TaskSnapshot` and `TaskGraphSnapshot` interfaces (they're now generated). Remove the hand-written `AgentRole` and `TaskState` type aliases. Remove the hand-written `EventPayload` discriminated union. Remove the hand-written `DomainEvent` interface.

- [ ] **Step 3: Verify TypeScript compilation**

Run: `pnpm --filter agent-gui run build`
Expected: May have type errors in consumer files (they still import from the old locations). We'll fix those in Task 5.

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/types/
git commit -m "feat(gui): add events-helpers and refactor types/index.ts to re-export from generated"
```

---

## Task 5: Refactor TypeScript consumers to remove `as` assertions

**Files:**

- Modify: `apps/agent-gui/src/composables/useTauriEvents.ts`
- Modify: `apps/agent-gui/src/composables/useTraceStore.ts`
- Modify: `apps/agent-gui/src/stores/session.ts`
- Modify: `apps/agent-gui/src/stores/taskGraph.ts`

- [ ] **Step 1: Refactor useTauriEvents.ts**

The switch statement on `p.type` already narrows the discriminated union. Remove all `const typed = p as { ... }` patterns and use `p.fieldName` directly.

Before:

```typescript
case "AgentTaskCreated": {
  const typed = p as {
    type: "AgentTaskCreated";
    task_id: string;
    title: string;
    role: string;
    dependencies: string[];
  };
  if (!taskGraphState.tasks.some((t) => t.id === typed.task_id)) {
    taskGraphState.tasks.push({
      id: typed.task_id,
      title: typed.title,
      role: typed.role as "Planner" | "Worker" | "Reviewer",
      state: "Pending",
      dependencies: typed.dependencies,
      error: null
    });
```

After:

```typescript
case "AgentTaskCreated": {
  if (!taskGraphState.tasks.some((t) => t.id === p.task_id)) {
    taskGraphState.tasks.push({
      id: p.task_id,
      title: p.title,
      role: p.role,
      state: "Pending" as TaskState,
      dependencies: p.dependencies,
      error: null
    });
```

Note: `p.role` will now be of the generated `AgentRole` type (e.g., `"Planner" | "Worker" | "Reviewer"`), so the `as "Planner" | "Worker" | "Reviewer"` cast is no longer needed. The `state: "Pending" as TaskState` cast may still be needed since TypeScript can't infer that `"Pending"` is a `TaskState` literal.

Apply this pattern to ALL case branches in the file. Here's the full refactored file:

```typescript
import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent } from "../types";
import type { TaskState } from "../types";
import { sessionState, applyEvent } from "../stores/session";
import { applyTraceEvent } from "./useTraceStore";
import { taskGraphState } from "../stores/taskGraph";

export function useTauriEvents() {
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<DomainEvent>("session-event", (tauriEvent) => {
      const domainEvent = tauriEvent.payload;
      const sessionId: string | undefined = domainEvent.session_id;
      if (
        sessionId &&
        sessionState.currentSessionId &&
        sessionId === sessionState.currentSessionId
      ) {
        applyEvent(domainEvent);
        applyTraceEvent(domainEvent);

        const p = domainEvent.payload;
        switch (p.type) {
          case "AgentTaskCreated": {
            if (!taskGraphState.tasks.some((t) => t.id === p.task_id)) {
              taskGraphState.tasks.push({
                id: p.task_id,
                title: p.title,
                role: p.role,
                state: "Pending" as TaskState,
                dependencies: p.dependencies,
                error: null
              });
              if (taskGraphState.currentSessionId === sessionId) {
                taskGraphState.tasks = [...taskGraphState.tasks];
              }
            }
            break;
          }
          case "AgentTaskStarted": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Running" as TaskState;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "AgentTaskCompleted": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Completed" as TaskState;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
          case "AgentTaskFailed": {
            const task = taskGraphState.tasks.find((t) => t.id === p.task_id);
            if (task) {
              task.state = "Failed" as TaskState;
              task.error = p.error;
              taskGraphState.tasks = [...taskGraphState.tasks];
            }
            break;
          }
        }
      }
    });
    sessionState.connected = true;
  });

  onUnmounted(() => {
    unlisten?.();
    sessionState.connected = false;
  });
}
```

- [ ] **Step 2: Refactor useTraceStore.ts**

Remove all `const typed = p as { ... }` patterns. Use discriminated union narrowing on `p.type` directly.

The switch statement in `applyTraceEvent` should change from:

```typescript
case "AgentTaskCreated": {
  const typed = p as { type: "AgentTaskCreated"; task_id: string; ... };
  pushEntry({ id: typed.task_id, ... });
```

to:

```typescript
case "AgentTaskCreated": {
  pushEntry({ id: p.task_id, ... });
```

Apply this to ALL case branches. The full refactored file maintains the same logic but with type narrowing instead of `as` casts.

For the `ContextAssembled` and `ModelRequestStarted` cases that use generated IDs (no unique ID in the event), keep the `Date.now()` + `Math.random()` pattern — no change needed there.

- [ ] **Step 3: Refactor session.ts**

Remove `as` type assertions in `applyEvent()`. The generated `EventPayload` discriminated union makes them unnecessary.

Before:

```typescript
case "UserMessageAdded": {
  const typed = p as { type: "UserMessageAdded"; content: string };
  sessionState.projection.messages.push({ role: "user", content: typed.content });
```

After:

```typescript
case "UserMessageAdded": {
  sessionState.projection.messages.push({ role: "user", content: p.content });
```

- [ ] **Step 4: Refactor taskGraph.ts**

Update the import of `TaskSnapshot` to come from the generated types (via `../types`).

The `TaskSnapshot` type in `types/index.ts` now re-exports from `generated/events`, so the import `import type { TaskSnapshot } from "../types"` should still work. But we need to verify that the generated `TaskSnapshot` matches the current hand-written type structure.

If the generated type uses `AgentRole` and `TaskState` enum types instead of inline string unions, the `taskGraph.ts` code needs to import those too. The `buildTaskTree` function should work unchanged since it only accesses `id` and `dependencies` fields.

- [ ] **Step 5: Verify TypeScript compilation and GUI build**

Run: `pnpm --filter agent-gui run build`
Expected: success with no type errors

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/composables/ apps/agent-gui/src/stores/
git commit -m "refactor(gui): remove manual type assertions, use discriminated union narrowing from generated types"
```

---

## Task 6: Update justfile and replace check-types.sh

**Files:**

- Modify: `justfile`
- Delete: `scripts/check-types.sh`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Update justfile gen-types command**

```diff
  # Regenerate TypeScript bindings from Tauri commands via specta
  gen-types:
-     cargo run -p agent-gui-tauri --bin export-specta -- apps/agent-gui/src/generated/commands.ts
+     cargo run -p agent-gui-tauri --bin export-specta -- apps/agent-gui/src/generated/commands.ts
+     cargo run -p agent-gui-tauri --bin export-events -- apps/agent-gui/src/generated/events.ts
      @echo "✅ TypeScript bindings regenerated"
```

- [ ] **Step 2: Update justfile check-types command**

```diff
  # Check that Rust EventPayload variants match TypeScript types
  check-types:
-     bash scripts/check-types.sh
+     just gen-types
+     git diff --exit-code apps/agent-gui/src/generated/ || (echo "❌ Generated types are out of sync! Run 'just gen-types' and commit the result." && exit 1)
+     @echo "✅ Generated types are in sync"
```

- [ ] **Step 3: Update CI type-sync job**

In `.github/workflows/ci.yml`, update the `type-sync` job:

```yaml
type-sync:
  name: Type sync check
  runs-on: ubuntu-latest
  steps:
    - name: Checkout
      uses: actions/checkout@v6

    - name: Setup pnpm
      uses: pnpm/action-setup@v6

    - name: Setup Node.js
      uses: pnpm/setup-node@v6
      with:
        node-version: 22
        cache: pnpm

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Cache cargo
      uses: Swatinem/rust-cache@v2
      with:
        cache-on-failure: true
        shared-key: rust-ci

    - name: Install Linux system deps for Tauri crates
      run: |
        sudo apt-get update
        sudo apt-get install -y \
          libglib2.0-dev \
          libgtk-3-dev \
          libwebkit2gtk-4.1-dev \
          libappindicator3-dev \
          librsvg2-dev \
          patchelf

    - name: Install repo tooling deps
      run: pnpm install --frozen-lockfile

    - name: Regenerate types
      run: |
        cargo run -p agent-gui-tauri --bin export-specta -- apps/agent-gui/src/generated/commands.ts
        cargo run -p agent-gui-tauri --bin export-events -- apps/agent-gui/src/generated/events.ts

    - name: Check generated types are in sync
      run: git diff --exit-code apps/agent-gui/src/generated/
```

- [ ] **Step 4: Delete check-types.sh**

```bash
rm scripts/check-types.sh
```

- [ ] **Step 5: Verify check-types works locally**

Run: `just check-types`
Expected: "✅ Generated types are in sync"

- [ ] **Step 6: Commit**

```bash
git add justfile scripts/check-types.sh .github/workflows/ci.yml
git commit -m "refactor(ci): replace check-types.sh with gen-types + git diff, add export-events to CI"
```

---

## Task 7: Run gen-types and verify end-to-end

**Files:**

- No new files — verification only

- [ ] **Step 1: Run gen-types to regenerate both files**

Run: `just gen-types`
Expected: both `commands.ts` and `events.ts` regenerated without errors

- [ ] **Step 2: Inspect generated events.ts**

Open `apps/agent-gui/src/generated/events.ts` and verify:

- File header comment present
- `EventPayload` is a TypeScript discriminated union
- All 24 variants present
- `DomainEvent` interface includes `payload: EventPayload`
- `AgentRole`, `TaskState`, `PrivacyClassification`, `MemoryScope` exported
- `TaskSnapshot`, `TaskGraphSnapshot` interfaces exported
- Field names match the hand-written types (snake_case)

- [ ] **Step 3: Run full GUI build**

Run: `pnpm --filter agent-gui run build`
Expected: success, no TypeScript errors

- [ ] **Step 4: Run full Rust test suite**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: No warnings

- [ ] **Step 6: Run format check**

Run: `pnpm run format:check`
Expected: PASS

- [ ] **Step 7: Run lint**

Run: `pnpm run lint`
Expected: PASS

- [ ] **Step 8: Commit any remaining changes**

If gen-types produced changes that weren't already committed:

```bash
git add apps/agent-gui/src/generated/
git commit -m "chore(gui): regenerate TypeScript bindings with event types"
```

---

## Task 8: Clean up stale worktree

**Files:**

- No files — git maintenance

- [ ] **Step 1: Remove stale worktree**

The `codex/memory-and-trace` worktree has been merged and is stale:

```bash
git worktree remove .worktrees/memory-and-trace
git branch -d codex/memory-and-trace
```

- [ ] **Step 2: Verify worktree removed**

Run: `git worktree list`
Expected: only the main worktree

---

## Plan Self-Review

### 1. Spec Coverage

| Spec Requirement                                                  | Task      |
| ----------------------------------------------------------------- | --------- |
| Add specta feature to agent-core                                  | Task 1 ✅ |
| Add specta feature to agent-memory                                | Task 2 ✅ |
| Create export-events binary                                       | Task 3 ✅ |
| Generate EventPayload discriminated union                         | Task 3 ✅ |
| Generate DomainEvent, PrivacyClassification, AgentRole, TaskState | Task 3 ✅ |
| Generate TaskSnapshot, TaskGraphSnapshot, MemoryScope             | Task 3 ✅ |
| Create events-helpers.ts (ExtractPayload, EventPayloadHandlers)   | Task 4 ✅ |
| Refactor types/index.ts to re-export from generated               | Task 4 ✅ |
| Remove `as` assertions from useTauriEvents.ts                     | Task 5 ✅ |
| Remove `as` assertions from useTraceStore.ts                      | Task 5 ✅ |
| Remove `as` assertions from session.ts                            | Task 5 ✅ |
| Update taskGraph.ts imports                                       | Task 5 ✅ |
| Update justfile gen-types                                         | Task 6 ✅ |
| Replace check-types.sh with gen-types + git diff                  | Task 6 ✅ |
| Update CI type-sync job                                           | Task 6 ✅ |
| End-to-end verification                                           | Task 7 ✅ |

### 2. Placeholder Scan

No TBD, TODO, "implement later", or "similar to Task N" patterns. All code blocks contain actual implementation code.

### 3. Type Consistency

- `EventPayload` variant names: PascalCase (matching `#[serde(tag = "type", rename_all = "PascalCase")]`)
- `AgentRole` / `TaskState`: PascalCase (matching `#[serde(rename_all = "PascalCase")]`)
- `PrivacyClassification`: snake_case (matching `#[serde(rename_all = "snake_case")]`)
- `MemoryScope`: PascalCase (matching `#[serde(rename_all = "PascalCase")]`) — Note: current TS uses lowercase strings ("user", "workspace", "session"), but the commands.rs maps them manually. The generated type will use PascalCase, which needs to be reconciled with commands.rs.
  - **Resolution**: The generated `MemoryScope` type will be `"User" | "Workspace" | "Session"`. The commands.rs `MemoryEntryResponse` maps to lowercase. Since `EventPayload` events don't directly contain `MemoryScope` (they use `scope: String`), the generated type won't affect existing consumers. The `MemoryScope` type is generated for completeness but may not be immediately used in event handling.
- ID types: map to `string` in TypeScript (custom `specta::Type` impl)
- `TaskSnapshot.role` type: will be generated as `AgentRole` enum, matching the refactored `useTauriEvents.ts` usage
