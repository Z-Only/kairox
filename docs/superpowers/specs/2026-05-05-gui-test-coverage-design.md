# GUI Test Coverage Expansion — Design Spec

**Date:** 2026-05-05
**Status:** Approved
**Scope:** Expand GUI (Vue 3 + TypeScript) test coverage from 8 tests to ~126 tests, covering all 7 logic modules and 11 Vue components, including component interaction tests with @vue/test-utils.

---

## Problem

The Kairox GUI has 29 source files (~3200 lines) but only 2 test files with 8 tests total. The `session.test.ts` covers `applyEvent`/`setProjection`/`resetProjection`, and `TraceTimeline.test.ts` is a placeholder (`expect(true).toBe(true)`). This means:

1. **No protection for the most complex logic** — `useTraceStore` has 25+ switch branches handling every event type, with dedup, status transitions, and state mutations. A single typo can break the trace panel silently.
2. **No store IPC testing** — `memory.ts`, `session.ts` (delete/rename/recover) all call `invoke()` but their error handling and state updates are completely untested.
3. **No component interaction testing** — ChatPanel's send/cancel, SessionsSidebar's CRUD flows, MemoryBrowser's filter/search/delete are all untested. Manual QA is the only validation.
4. **No test infrastructure for components** — Vitest has no jsdom environment, no Vue plugin, no `@vue/test-utils`. Writing component tests is currently impossible.

## Goal

Achieve comprehensive test coverage for all GUI logic and component layers using Vitest + @vue/test-utils + jsdom. Every logic module gets unit tests. Every interactive component gets mount + interaction tests. All Tauri IPC is mocked via `vi.mock`.

## Design Decisions

| Decision            | Choice                                                                                   | Rationale                                                                                     |
| ------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Test framework      | Vitest (already in devDeps)                                                              | No new framework; matches project tooling                                                     |
| Component testing   | @vue/test-utils + jsdom                                                                  | Industry standard for Vue 3; shallow/deep mount + interaction simulation                      |
| Tauri mock strategy | `vi.mock('@tauri-apps/api/core')` and `vi.mock('@tauri-apps/api/event')` at module level | Clean, no Tauri dependency; each test configures invoke return values                         |
| Test file location  | Colocated with source (`*.test.ts` next to `*.ts` / `*.vue`)                             | Matches existing convention; easy to find                                                     |
| Coverage reporting  | `@vitest/coverage-v8`                                                                    | Optional but valuable for CI visibility                                                       |
| Vitest globals      | `true`                                                                                   | Matches `vitest/globals` pattern used in existing tests                                       |
| Approach            | Logic layer first, then components                                                       | Logic tests are fast and catch the most bugs; component tests build on the same mock patterns |

## Test Infrastructure

### New dependencies

| Package               | Purpose                                              |
| --------------------- | ---------------------------------------------------- |
| `@vue/test-utils`     | Mount Vue components, find elements, simulate events |
| `jsdom`               | Browser environment simulation for Vitest            |
| `@vitest/coverage-v8` | Coverage reporting (optional)                        |

### vitest.config.ts

Create `apps/agent-gui/vitest.config.ts`:

```typescript
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,vue}"],
      exclude: ["src/generated/**", "src/env.d.ts"]
    }
  }
});
```

### Mock patterns

**Tauri invoke mock** (used in store and component tests):

```typescript
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);
```

**Tauri event listen mock** (used in component tests):

```typescript
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
```

**Notification mock** (used in store tests that trigger notifications):

```typescript
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));
```

## Logic Layer Test Coverage

### useTraceStore.ts (~25 tests)

| Group                     | Count | What's tested                                                                                     |
| ------------------------- | ----- | ------------------------------------------------------------------------------------------------- |
| AgentTaskCreated          | 2     | Creates entry; dedup by task_id                                                                   |
| UserMessageAdded          | 2     | Creates entry; truncation at 80 chars                                                             |
| ContextAssembled          | 2     | Creates entry; sources→outputPreview                                                              |
| ModelRequestStarted       | 1     | Creates running entry                                                                             |
| ModelTokenDelta           | 1     | Skipped (no entry)                                                                                |
| AssistantMessageCompleted | 3     | Updates running model→completed; creates assistant entry if no running model; dedup by message_id |
| ModelToolCallRequested    | 1     | Creates running tool call                                                                         |
| ToolInvocationStarted     | 1     | Creates running invocation                                                                        |
| ToolInvocationCompleted   | 2     | Updates to completed with durationMs/outputPreview; no crash on missing entry                     |
| ToolInvocationFailed      | 2     | Updates to failed; no crash on missing entry                                                      |
| Permission lifecycle      | 3     | Requested→pending, Granted→completed, Denied→failed                                               |
| Memory lifecycle          | 3     | Proposed→pending, Accepted→completed, Rejected→failed+reason                                      |
| clearTrace                | 1     | Clears entries and dedup set                                                                      |
| density                   | 1     | Density state is mutable                                                                          |

### taskGraph.ts (~6 tests)

| Group               | Count | What's tested             |
| ------------------- | ----- | ------------------------- |
| Empty list          | 1     | Returns []                |
| Single root         | 1     | Single node tree          |
| Linear chain A→B→C  | 1     | Nested tree               |
| Parallel children   | 1     | Root with 2 children      |
| Multiple roots      | 1     | Independent trees         |
| Dangling dependency | 1     | No crash, treated as root |

### useNotifications.ts (~6 tests)

| Group                | Count | What's tested                         |
| -------------------- | ----- | ------------------------------------- |
| addNotification      | 2     | Pushes to array; id auto-increments   |
| dismissNotification  | 2     | Removes by id; no crash on missing id |
| auto-dismiss         | 1     | vi.useFakeTimers, 8s auto-removal     |
| type differentiation | 1     | error/warning/info types preserved    |

### events-helpers.ts (~5 tests)

| Group                        | Count | What's tested                                           |
| ---------------------------- | ----- | ------------------------------------------------------- |
| matchPayload exhaustive      | 2     | Routes each variant to correct handler; type narrowing  |
| matchPartialPayload          | 2     | Handles specified variant; returns undefined for others |
| ExtractPayload compile check | 1     | ts-expect verifies type narrowing                       |

### markdown.ts (~5 tests)

| Group                    | Count | What's tested                                      |
| ------------------------ | ----- | -------------------------------------------------- |
| Plain text               | 1     | Renders as `<p>`                                   |
| Code block with language | 2     | hljs wrapping; unknown language fallback to escape |
| Inline code              | 1     | Backtick renders `<code>`                          |
| XSS protection           | 1     | html:false, `<script>` escaped                     |

### memory.ts store (~6 tests)

| Group                    | Count | What's tested                               |
| ------------------------ | ----- | ------------------------------------------- |
| loadMemories success     | 2     | filter=all invokes correctly; loading state |
| loadMemories failure     | 1     | invoke reject → addNotification("error")    |
| deleteMemoryItem success | 1     | Removes from array                          |
| deleteMemoryItem failure | 1     | invoke reject → addNotification("error")    |
| setMemoryFilter          | 1     | Updates filter and triggers loadMemories    |

### session.ts IPC interactions (~7 tests)

| Group                         | Count | What's tested                                |
| ----------------------------- | ----- | -------------------------------------------- |
| deleteSession success         | 1     | Removes from sessions array                  |
| deleteSession current session | 1     | Auto-switches to first remaining             |
| deleteSession failure         | 1     | addNotification("error")                     |
| renameSession success         | 1     | Local title updated                          |
| renameSession failure         | 1     | addNotification("error")                     |
| recoverSessions success       | 1     | Restores workspaceId + sessions + projection |
| recoverSessions no workspace  | 1     | Returns false                                |

**Logic layer subtotal: ~60 tests**

## Component Layer Test Coverage

### ChatPanel.vue (~12 tests)

| Group                    | Count | What's tested                                                 |
| ------------------------ | ----- | ------------------------------------------------------------- |
| Render messages          | 2     | User message rendered; assistant markdown rendered            |
| Streaming state          | 2     | token_stream shows text + cursor; cancelled shows [cancelled] |
| Send message             | 3     | Enter sends; Send button sends; empty input blocked           |
| Shift+Enter              | 1     | No send on Shift+Enter                                        |
| Cancel button            | 2     | Visible during streaming; invokes cancel_session              |
| Send failure             | 1     | invoke reject → reportSendError + addNotification             |
| Streaming disables input | 1     | textarea disabled when isStreaming                            |

### NotificationToast.vue (~5 tests)

| Group                | Count | What's tested                        |
| -------------------- | ----- | ------------------------------------ |
| Empty state          | 1     | Container not rendered               |
| Render notifications | 2     | Shows last 3; type→CSS class mapping |
| Dismiss button       | 1     | Calls dismissNotification            |
| Icon mapping         | 1     | error→✕, warning→⚠, info→ℹ           |

### SessionsSidebar.vue (~10 tests)

| Group            | Count | What's tested                                                  |
| ---------------- | ----- | -------------------------------------------------------------- |
| Session list     | 1     | Renders session titles                                         |
| Switch session   | 2     | Calls switch_session invoke; resets projection/trace/taskGraph |
| Delete session   | 2     | Shows ConfirmDialog; confirms → delete_session invoke          |
| Rename session   | 2     | Double-click enters edit; Enter submits rename_session invoke  |
| New session      | 2     | Shows new session panel; submits create_session invoke         |
| Profile dropdown | 1     | Shows available profiles                                       |

### MemoryBrowser.vue (~8 tests)

| Group              | Count | What's tested                                                |
| ------------------ | ----- | ------------------------------------------------------------ |
| Render memory list | 1     | Displays memories with scope icons                           |
| Scope filter       | 2     | Clicking filter changes active state; calls loadMemories     |
| Search             | 1     | Enter triggers loadMemories                                  |
| Empty state        | 1     | "No memories" displayed                                      |
| Delete memory      | 2     | Click 🗑️ shows ConfirmDialog; confirm → delete_memory invoke |
| Loading state      | 1     | "Loading..." displayed                                       |

### ConfirmDialog.vue (~4 tests)

| Group          | Count | What's tested                        |
| -------------- | ----- | ------------------------------------ |
| Render props   | 1     | Title and message displayed          |
| Confirm button | 1     | Emits "confirm"                      |
| Cancel button  | 1     | Emits "cancel"                       |
| Danger style   | 1     | confirmDanger prop adds danger class |

### TraceEntry.vue (~7 tests)

| Group            | Count | What's tested                                    |
| ---------------- | ----- | ------------------------------------------------ |
| Collapsed        | 1     | Details hidden                                   |
| Expanded         | 2     | rawEvent visible; click toggles expanded         |
| Status icons     | 2     | running→⟳, completed→✓, failed→✕, pending→⏳     |
| Duration display | 1     | Shows "Xms" when durationMs present              |
| Kind styling     | 1     | Different CSS classes for tool/permission/memory |

### TraceTimeline.vue (~4 tests)

| Group          | Count | What's tested                                             |
| -------------- | ----- | --------------------------------------------------------- |
| Default tab    | 1     | Trace tab active                                          |
| Tab switching  | 2     | Tasks tab shows TaskSteps; Memory tab shows MemoryBrowser |
| Density toggle | 1     | L1→L2→L3 cycle                                            |

### TaskSteps.vue (~5 tests)

| Group         | Count | What's tested                            |
| ------------- | ----- | ---------------------------------------- |
| Empty graph   | 1     | Empty state shown                        |
| Task tree     | 1     | Tree structure rendered with indentation |
| State badges  | 2     | Each TaskState gets correct badge color  |
| Error display | 1     | Failed task shows error message          |

### PermissionPrompt.vue (~3 tests)

| Group          | Count | What's tested                                        |
| -------------- | ----- | ---------------------------------------------------- |
| Render request | 1     | Shows tool_id and preview                            |
| Approve        | 1     | Emits / invokes decide_permission with approve:true  |
| Deny           | 1     | Emits / invokes decide_permission with approve:false |

### StatusBar.vue (~2 tests)

| Group                 | Count | What's tested                                      |
| --------------------- | ----- | -------------------------------------------------- |
| Fetch permission mode | 2     | Mount invokes get_permission_mode; displays result |

**Component layer subtotal: ~60 tests**

## Test Execution Summary

| Layer     | Modules/Components | Tests    |
| --------- | ------------------ | -------- |
| Logic     | 7 modules          | ~60      |
| Component | 10 components      | ~60      |
| Existing  | 2 files            | 8 (kept) |
| **Total** |                    | **~128** |

## Out of Scope

- E2E tests requiring a running Tauri backend
- Visual regression / screenshot diff testing
- Performance benchmarks
- Testing auto-generated files (`src/generated/`)
- Refactoring existing `session.test.ts` tests (they stay as-is)

## Risks and Mitigations

| Risk                                                      | Mitigation                                                        |
| --------------------------------------------------------- | ----------------------------------------------------------------- |
| jsdom lacks `IntersectionObserver`, `ResizeObserver`      | Polyfill or skip affected tests                                   |
| Vue 3 reactive gotchas in test teardown                   | Use `vi.restoreAllMocks()` + reset reactive state in `beforeEach` |
| Component tests fragile to template changes               | Focus on behavior (click→invoke) not CSS selectors                |
| `useTauriEvents` uses `onMounted`/`onUnmounted` lifecycle | Requires mount/unmount cycle in test; use @vue/test-utils         |
| `setTimeout` in useNotifications                          | Use `vi.useFakeTimers()` per test, restore in `afterEach`         |
