# GUI Core Interaction Polish — Design Spec

**Date:** 2026-05-05
**Status:** Approved
**Scope:** Five features to make the Kairox GUI a daily-usable application: cancel session, error notifications, memory browser, code syntax highlighting, and real status bar.

---

## Problem

The Kairox GUI (v0.10.0) has functional session management, chat streaming, trace visualization, and task graph inspection, but several critical interaction gaps make it unsuitable for daily use:

1. **No way to cancel a running session** — `AppFacade::cancel_session` exists in Rust but has no Tauri command or UI button. When the agent loop runs too long or produces unwanted output, users must force-quit the app.

2. **Errors are invisible** — `send_message` spawns a background task that only prints to stderr on failure. The user sees nothing. The hack that emits a synthetic `AgentTaskFailed` JSON event is fragile and doesn't cover all error paths.

3. **No memory browser** — `query_memories` and `delete_memory` Tauri commands exist, but no UI component exposes them. Users can accept/reject memory proposals when they appear in the PermissionCenter, but cannot review, search, or manage stored memories.

4. **Code blocks have no syntax highlighting** — `markdown.ts` configures `highlight.js` but the CSS theme is missing and `<pre class="hljs">` has no styling in the chat panel.

5. **Status bar shows hardcoded "mode: interactive"** — The actual `PermissionMode` is `Interactive` but is hard-coded in `StatusBar.vue`, not read from runtime state.

## Goal

Close all five interaction gaps with minimal, non-breaking changes that follow existing patterns (reactive stores, Tauri command bridge, component composition).

## Design Decisions

| Decision                  | Choice                                               | Rationale                                                                           |
| ------------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Approach                  | Incremental enhancement (Approach A)                 | Lowest risk; each feature is independently deliverable; no store refactoring needed |
| Notification system       | Lightweight composable, not a full event bus         | Sufficient for error toasts; aligns with Vue 3 reactivity patterns                  |
| Cancel button location    | ChatPanel input area, replaces Send during streaming | Standard UX pattern (ChatGPT, Claude); no layout changes needed                     |
| Memory browser location   | Third tab in right sidebar (Trace / Tasks / Memory)  | Follows existing tab pattern; doesn't require new layout zones                      |
| Syntax highlighting       | Add highlight.js CSS theme import                    | `markdown.ts` already wires the highlighter; only CSS is missing                    |
| Status bar mode           | Query via new Tauri command on mount                 | Simple, no event subscription needed for a rarely-changing value                    |
| Permission mode switching | Out of scope                                         | YAGNI; display only for now                                                         |

## Feature 1: Cancel Session

### Tauri command

Add `cancel_session` to `commands.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn cancel_session(state: State<'_, GuiState>) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };
    state.runtime.cancel_session(workspace_id, session_id)
        .await
        .map_err(|e| e.to_string())
}
```

Register in `lib.rs` `invoke_handler` and `specta.rs` `collect_commands![]`.

### Frontend: ChatPanel cancel button

When `sessionState.isStreaming` is true, replace the "Send" button with a "Cancel" button (red styling). Clicking it invokes `cancel_session`. The existing `SessionCancelled` event handler in `session.ts` already sets `isStreaming = false` and `cancelled = true`, so the UI updates automatically.

```vue
<!-- In ChatPanel.vue template -->
<button
  v-if="sessionState.isStreaming"
  class="cancel-button"
  @click="cancelSession"
>
  Cancel
</button>
<button
  v-else
  class="send-button"
  :disabled="!inputText.trim()"
  @click="sendMessage"
>
  Send
</button>
```

```typescript
async function cancelSession() {
  try {
    await invoke("cancel_session");
  } catch (e) {
    console.error("Failed to cancel session:", e);
    addNotification("error", `Cancel failed: ${e}`);
  }
}
```

### Cleanup: send_message error handling

Replace the current `tokio::spawn` + `eprintln!` + manual JSON emit hack in `commands.rs` with a proper error event pattern. The background task should emit a typed error via `app_handle.emit()`:

```rust
// In send_message command, replace the spawn body:
tokio::spawn(async move {
    let result = runtime
        .send_message(agent_core::SendMessageRequest {
            workspace_id,
            session_id,
            content,
        })
        .await;

    if let Err(e) = result {
        eprintln!("[commands] send_message failed: {e}");
        // Emit a structured error event the frontend can handle
        let payload = serde_json::json!({
            "type": "SendMessageError",
            "error": e.to_string(),
            "session_id": session_id_str
        });
        let _ = app_handle.emit("session-error", &payload);
    }
});
```

A separate `"session-error"` Tauri event channel keeps error notifications decoupled from the domain event stream.

## Feature 2: Error Notifications

### Notification composable

New file: `apps/agent-gui/src/composables/useNotifications.ts`

```typescript
import { reactive } from "vue";

export interface Notification {
  id: string;
  type: "error" | "warning" | "info";
  message: string;
  timestamp: number;
}

export const notifications = reactive<Notification[]>([]);

let nextId = 0;

export function addNotification(
  type: Notification["type"],
  message: string
): void {
  const id = `notif-${nextId++}`;
  notifications.push({ id, type, message, timestamp: Date.now() });
  // Auto-dismiss after 8 seconds
  setTimeout(() => dismissNotification(id), 8000);
}

export function dismissNotification(id: string): void {
  const idx = notifications.findIndex((n) => n.id === id);
  if (idx !== -1) notifications.splice(idx, 1);
}
```

### NotificationToast component

New file: `apps/agent-gui/src/components/NotificationToast.vue`

Fixed-position overlay in the bottom-right corner. Shows up to 3 notifications, newest on top. Each has type-specific color (error=red, warning=amber, info=blue), message, and dismiss button.

### Integration points

1. **`App.vue`** — Add `NotificationToast` component and listen for `"session-error"` Tauri events:

```typescript
import { listen } from "@tauri-apps/api/event";
import { addNotification } from "./composables/useNotifications";

onMounted(async () => {
  await listen<{ type: string; error: string; session_id: string }>(
    "session-error",
    (event) => {
      addNotification("error", event.payload.error);
    }
  );
});
```

2. **`useTauriEvents.ts`** — On `AgentTaskFailed` events with non-empty `error`, also call `addNotification("error", ...)`.
3. **`ChatPanel.vue`** — `cancelSession` error path calls `addNotification`.
4. **`SessionsSidebar.vue`** — `switchSession`, `createSession`, `deleteSession` error paths call `addNotification`.

## Feature 3: Memory Browser

### Tab in right sidebar

`TraceTimeline.vue` already has a tab group (Trace / Tasks). Add a third tab: "Memory".

### MemoryBrowser component

New file: `apps/agent-gui/src/components/MemoryBrowser.vue`

Features:

- Calls `query_memories` on mount and when switching sessions
- Displays memories in a list grouped by scope (session / user / workspace)
- Each memory item shows: scope badge, key (if present), content preview, accepted status
- Delete button per item (reuse `ConfirmDialog`)
- Scope filter buttons (All / Session / User / Workspace)
- Search input that calls `query_memories` with keywords
- Auto-refresh: listens for `MemoryAccepted` events via `useTauriEvents`

### Store

New file: `apps/agent-gui/src/stores/memory.ts`

```typescript
import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface MemoryItem {
  id: string;
  scope: string;
  key: string | null;
  content: string;
  accepted: boolean;
}

export const memoryState = reactive({
  memories: [] as MemoryItem[],
  loading: false,
  filter: "all" as "all" | "session" | "user" | "workspace",
  searchQuery: ""
});

export async function loadMemories(): Promise<void> {
  memoryState.loading = true;
  try {
    const scope = memoryState.filter === "all" ? null : memoryState.filter;
    const keywords = memoryState.searchQuery
      ? memoryState.searchQuery.split(/\s+/).filter(Boolean)
      : null;
    memoryState.memories = await invoke("query_memories", {
      scope,
      keywords,
      limit: 100
    });
  } catch (e) {
    console.error("Failed to load memories:", e);
  } finally {
    memoryState.loading = false;
  }
}

export async function deleteMemoryItem(id: string): Promise<void> {
  try {
    await invoke("delete_memory", { id });
    memoryState.memories = memoryState.memories.filter((m) => m.id !== id);
  } catch (e) {
    console.error("Failed to delete memory:", e);
  }
}
```

### Tab integration in TraceTimeline

The existing `rightPanelTab` ref becomes `"trace" | "tasks" | "memory"`. Add a third tab button and conditionally render `MemoryBrowser` when selected.

## Feature 4: Code Syntax Highlighting

### Problem

`markdown.ts` already configures `highlight.js` with language detection and produces `<pre class="hljs"><code>...</code></pre>`. However:

1. No highlight.js CSS theme is imported
2. `ChatPanel.vue` renders markdown via `v-html="renderMarkdown(msg.content)"` but the `.hljs` class has no styles

### Solution

1. **Add highlight.js theme CSS** — Import a dark theme (e.g., `github-dark`) in `main.ts`:

```typescript
import "highlight.js/styles/github-dark.css";
```

This adds all `.hljs`-scoped styles globally.

2. **Add chat-specific code block styles** in `ChatPanel.vue` scoped styles:

```css
.message-content :deep(pre.hljs) {
  margin: 8px 0;
  border-radius: 6px;
  padding: 12px;
  overflow-x: auto;
  font-size: 13px;
  line-height: 1.5;
}

.message-content :deep(code) {
  font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
}

.message-content :deep(:not(pre) > code) {
  background: #f0f0f0;
  padding: 2px 4px;
  border-radius: 3px;
  font-size: 12px;
}
```

3. No changes to `markdown.ts` — it's already correctly configured.

## Feature 5: Real Status Bar

### Tauri command

Add `get_permission_mode` to `commands.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn get_permission_mode(state: State<'_, GuiState>) -> Result<String, String> {
    Ok(format!("{:?}", state.runtime.permission_mode()))
}
```

This requires adding a `permission_mode()` accessor to `LocalRuntime` (it already stores `PermissionEngine` which holds the mode). A simple delegation:

```rust
// In facade_runtime.rs
impl<S, M> LocalRuntime<S, M> {
    pub fn permission_mode(&self) -> PermissionMode {
        self.permission_engine.mode()
    }
}
```

### Frontend: StatusBar update

Query on mount and display:

```typescript
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const permissionMode = ref("Interactive");

onMounted(async () => {
  try {
    permissionMode.value = await invoke("get_permission_mode");
  } catch {
    permissionMode.value = "Interactive";
  }
});
```

Replace the hardcoded `mode: interactive` with `mode: {{ permissionMode }}`.

Mode display names for friendliness:

| Internal    | Display     |
| ----------- | ----------- |
| ReadOnly    | read-only   |
| Suggest     | suggest     |
| Agent       | agent       |
| Autonomous  | autonomous  |
| Interactive | interactive |

## File Changes Summary

### New files

| File                                                  | Purpose                               |
| ----------------------------------------------------- | ------------------------------------- |
| `apps/agent-gui/src/composables/useNotifications.ts`  | Reactive notification state + helpers |
| `apps/agent-gui/src/components/NotificationToast.vue` | Toast overlay component               |
| `apps/agent-gui/src/components/MemoryBrowser.vue`     | Memory browser panel                  |
| `apps/agent-gui/src/stores/memory.ts`                 | Memory store + query/delete actions   |

### Modified files

| File                                               | Changes                                                                                     |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `apps/agent-gui/src-tauri/src/commands.rs`         | Add `cancel_session`, `get_permission_mode` commands; improve `send_message` error handling |
| `apps/agent-gui/src-tauri/src/lib.rs`              | Register new commands in `invoke_handler`                                                   |
| `apps/agent-gui/src-tauri/src/specta.rs`           | Register new commands in `collect_commands![]`                                              |
| `crates/agent-runtime/src/facade_runtime.rs`       | Add `pub fn permission_mode()` accessor                                                     |
| `apps/agent-gui/src/components/ChatPanel.vue`      | Replace Send with Cancel button during streaming; import `addNotification`                  |
| `apps/agent-gui/src/components/StatusBar.vue`      | Query and display real permission mode                                                      |
| `apps/agent-gui/src/components/TraceTimeline.vue`  | Add "Memory" tab; import `MemoryBrowser`                                                    |
| `apps/agent-gui/src/composables/useTauriEvents.ts` | Call `addNotification` on `AgentTaskFailed`                                                 |
| `apps/agent-gui/src/stores/session.ts`             | Call `addNotification` on error paths                                                       |
| `apps/agent-gui/src/App.vue`                       | Add `NotificationToast`; listen for `"session-error"` Tauri event                           |
| `apps/agent-gui/src/main.ts`                       | Import highlight.js CSS theme                                                               |
| `apps/agent-gui/src/generated/commands.ts`         | Auto-regenerated by `just gen-types`                                                        |
| `apps/agent-gui/src/generated/events.ts`           | Auto-regenerated (if event types change)                                                    |

## Risks and Mitigations

| Risk                                                                          | Mitigation                                                                                                                                                                         |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cancel_session` may not stop a long-running model request immediately        | The `SessionCancelled` event is appended to the event store and will stop the agent loop on next iteration; this is the existing design. The UI immediately shows cancelled state. |
| `"session-error"` Tauri event is not a typed DomainEvent                      | It's intentionally a lightweight error channel. If we later want type-safe error events, we can add an `EventPayload::SendMessageError` variant, but YAGNI now.                    |
| Memory browser queries all memories on mount; could be slow with many entries | The `limit: 100` cap and keyword search prevent excessive data. Pagination can be added later.                                                                                     |
| highlight.js CSS theme import adds ~30KB                                      | Tree-shaking only includes imported styles; `github-dark.css` is a small file (~2KB gzipped). Acceptable for a desktop app.                                                        |
| `get_permission_mode` is a one-shot query, not reactive                       | Permission mode rarely changes (set at startup). If runtime mode-switching is added later, an event can be emitted then.                                                           |
