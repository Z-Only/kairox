# GUI Bug Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five targeted bug fixes: session naming with auto-dedup, model error message fix, image thumbnail loading, archive confirmation icon consistency, and context window display correction.

**Architecture:** Frontend Vue 3 stores/components changes with Rust backend fixes in projection, runner, and error messages. No new IPC commands. No new dependencies.

**Tech Stack:** Rust (agent-models, agent-runtime), Vue 3 / TypeScript (agent-gui), Tauri v2, Vitest

---

## File Map

| File                                                | Role                                                                                                        |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `apps/agent-gui/src/stores/session.ts`              | Task 1 (naming dedup), Task 2 (lastSendError + error display)                                               |
| `apps/agent-gui/src/stores/project.ts`              | Task 1 (project session naming dedup)                                                                       |
| `apps/agent-gui/src-tauri/src/commands.rs`          | Task 1 (backend default title), Task 2 (not used directly — model errors are in agent-models/agent-runtime) |
| `crates/agent-models/src/router.rs`                 | Task 2 (error message text)                                                                                 |
| `crates/agent-runtime/src/facade_runtime.rs`        | Task 2 (error message text)                                                                                 |
| `crates/agent-models/tests/integration.rs`          | Task 2 (test update)                                                                                        |
| `apps/agent-gui/src/components/ChatPanel.vue`       | Task 2 (frontend error display), Task 3 (thumbnails), Task 4 (n/a)                                          |
| `apps/agent-gui/src/components/SessionsSidebar.vue` | Task 4 (archive icons)                                                                                      |
| `apps/agent-gui/src/locales/en.json`                | Task 2 (i18n)                                                                                               |
| `apps/agent-gui/src/locales/zh-CN.json`             | Task 2 (i18n)                                                                                               |
| `crates/agent-core/src/projection.rs`               | Task 5 (project ModelProfileSwitched)                                                                       |
| `crates/agent-runtime/src/agent_loop/runner.rs`     | Task 5 (fix fallback limit resolution)                                                                      |
| `apps/agent-gui/src/components/ContextMeter.vue`    | Task 5 (use modelLimits for display)                                                                        |

---

### Task 1: Default session name — "New Session" with auto-numbering

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs:329`
- Modify: `apps/agent-gui/src/stores/session.ts:35-43, 394-406`
- Modify: `apps/agent-gui/src/stores/project.ts:112-126, 220-230`

- [ ] **Step 1: Change backend default title**

In `apps/agent-gui/src-tauri/src/commands.rs`, line 329, change the title:

```rust
// Before:
let title = format!("Session using {profile}");
// After:
let title = "New Session".to_string();
```

- [ ] **Step 2: Add uniqueSessionTitle helper in session.ts**

Add a helper function in `apps/agent-gui/src/stores/session.ts` (after the imports, around line 34):

```typescript
export function uniqueSessionTitle(base: string, existingTitles: string[]): string {
  if (!existingTitles.includes(base)) return base;
  let n = 1;
  while (existingTitles.includes(`${base} ${n}`)) {
    n++;
  }
  return `${base} ${n}`;
}
```

- [ ] **Step 3: Change temporaryTitleFromFirstMessage default**

In `apps/agent-gui/src/stores/session.ts`, line 37, change the default title:

```typescript
// Before:
if (!trimmedContent) return "New conversation";
// After:
if (!trimmedContent) return "New Session";
```

- [ ] **Step 4: Apply dedup in createSession()**

In `apps/agent-gui/src/stores/session.ts`, in the `createSession` function (around line 394), after `const result = await invoke<...>("start_session", ...)`, add dedup logic. Modify the function:

```typescript
async function createSession(
  profile?: string
): Promise<{ id: string; title: string; profile: string }> {
  const result = await invoke<{ id: string; title: string; profile: string }>("start_session", {
    profile: resolveSessionProfile(profile)
  });

  // Dedup: if the backend returns "New Session", ensure uniqueness
  let title = result.title;
  if (title === "New Session" || title.startsWith("New Session ")) {
    const existingTitles = sessions.value.map((s) => s.title);
    title = uniqueSessionTitle("New Session", existingTitles);
  }

  sessions.value = await listOrdinarySessions();
  currentProfile.value = result.profile;
  resetProjection();
  clearTrace();
  useTaskGraphStore().clearTaskGraph();
  return { ...result, title };
}
```

Note: The session list is refreshed by `listOrdinarySessions()` which fetches from the backend. The dedup title from the frontend will be used for the return value, but the stored title in the backend will still be "New Session". Since `renameSession` later persists the real title, this is acceptable. However, if the user immediately creates another session before the first one gets renamed, the backend title is "New Session" for both. The dedup is frontend-only for display purposes at creation time.

For persistence correctness, we should also rename the session on the backend:

```typescript
async function createSession(
  profile?: string
): Promise<{ id: string; title: string; profile: string }> {
  const result = await invoke<{ id: string; title: string; profile: string }>("start_session", {
    profile: resolveSessionProfile(profile)
  });

  // Dedup: check all existing sessions (including the one just created)
  sessions.value = await listOrdinarySessions();
  const existingTitles = sessions.value.filter((s) => s.id !== result.id).map((s) => s.title);
  let title = "New Session";
  title = uniqueSessionTitle(title, existingTitles);

  // Persist the deduped title
  if (title !== result.title) {
    try {
      await invoke("rename_session", { sessionId: result.id, title });
    } catch (e) {
      console.error("Failed to set deduped session title:", e);
    }
  }

  currentProfile.value = result.profile;
  resetProjection();
  clearTrace();
  useTaskGraphStore().clearTaskGraph();
  return { id: result.id, title, profile: result.profile };
}
```

- [ ] **Step 5: Apply dedup in project.ts createDraftSessionPlaceholder**

In `apps/agent-gui/src/stores/project.ts`, modify `createDraftSessionPlaceholder`:

```typescript
// Change line 119:
title: "New conversation",
// To:
title: "New Session",
```

Then in the `createProjectDraftSession` function (around line 220), add dedup after creating the placeholder:

```typescript
async function createProjectDraftSession(
  projectId: string,
  branch?: string | null
): Promise<ProjectSessionInfo> {
  const sessionId = agent_core.SessionId.new().to_string();
  const project = projects.value.find((p) => p.projectId === projectId);

  // Dedup within the same project
  const projectSessions = sessionsByProject.value.get(projectId) ?? [];
  const existingTitles = projectSessions.map((s) => s.title);
  const baseTitle = "New Session";
  const dedupedTitle = uniqueSessionTitle(baseTitle, existingTitles);

  const draftSession = createDraftSessionPlaceholder(sessionId, project, branch);
  draftSession.title = dedupedTitle;
  // ... rest of function
}
```

Note: `uniqueSessionTitle` should be imported from `session.ts` or moved to a shared utility.

- [ ] **Step 6: Commit Task 1**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src/stores/session.ts apps/agent-gui/src/stores/project.ts
git commit -m "fix(gui): default session name to 'New Session' with auto-numbering dedup"
```

---

### Task 2: Fix "unknown model profile" error message

**Files:**

- Modify: `crates/agent-models/src/router.rs:50`
- Modify: `crates/agent-runtime/src/facade_runtime.rs:517`
- Modify: `crates/agent-models/tests/integration.rs:82`
- Modify: `apps/agent-gui/src/stores/session.ts` (add `lastSendError`)
- Modify: `apps/agent-gui/src/components/ChatPanel.vue:216-218, 246`
- Modify: `apps/agent-gui/src/locales/en.json:69`
- Modify: `apps/agent-gui/src/locales/zh-CN.json:69`

- [ ] **Step 1: Fix Rust error message in router.rs**

In `crates/agent-models/src/router.rs`, line 50:

```rust
// Before:
"unknown model profile: '{}'",
// After:
"unknown model: '{}'",
```

- [ ] **Step 2: Fix Rust error message in facade_runtime.rs**

In `crates/agent-runtime/src/facade_runtime.rs`, line 517:

```rust
// Before:
"unknown model profile: {profile_alias}"
// After:
"unknown model: {profile_alias}"
```

- [ ] **Step 3: Update Rust integration test**

In `crates/agent-models/tests/integration.rs`, line 82:

```rust
// Before:
msg.contains("unknown model profile"),
// After:
msg.contains("unknown model"),
```

- [ ] **Step 4: Verify Rust tests pass**

Run: `cargo test -p agent-models -p agent-runtime --all-targets`
Expected: All tests PASS with updated message text.

- [ ] **Step 5: Add lastSendError to session store**

In `apps/agent-gui/src/stores/session.ts`, add a ref near the other state declarations (around line 127):

```typescript
const lastSendError = ref<string | null>(null);
```

Update `reportSendError` (around line 174):

```typescript
function reportSendError(message: string) {
  lastSendError.value = message;
  projection.value.messages.push({
    role: "assistant",
    content: `[error] ${message}`
  });
  projection.value.token_stream = "";
  isStreaming.value = false;
}
```

Clear `lastSendError` when a new user message is added. In the `applyEvent` function, under `case "UserMessageAdded":` (around line 189):

```typescript
case "UserMessageAdded": {
  lastSendError.value = null;  // <-- add this line
  projection.value.messages.push({
    role: "user",
    content: p.content
  });
  isStreaming.value = true;
  break;
}
```

Also clear it in `resetProjection` (around line 260):

```typescript
function resetProjection() {
  projection.value = emptyProjection();
  lastSendError.value = null; // <-- add this line
  isStreaming.value = false;
}
```

Export `lastSendError` in the return statement (around line 533):

```typescript
return {
  // ... existing exports
  lastSendError, // <-- add
  reportSendError
  // ...
};
```

- [ ] **Step 6: Fix frontend error display in ChatPanel.vue**

In `apps/agent-gui/src/components/ChatPanel.vue`, modify the `selectModelProfile` catch block (around line 216):

```typescript
} catch (e) {
  console.error("Failed to switch model:", e);
  const errMsg = String(e);
  if (errMsg.includes("unknown model")) {
    notify("error", t("errors.modelNotFound", { alias }));
  } else {
    notify("error", t("context.switchModelFailed", { error: errMsg }));
  }
}
```

Also fix the `sendMessage` catch block (line 244-247) to use `lastSendError`:

```typescript
} catch (e) {
  console.error("Failed to send message:", e);
  const errMsg = String(e);
  session.reportSendError(errMsg);
  notify("error", t("chat.sendFailed", { error: errMsg }));
}
```

- [ ] **Step 7: Update i18n keys**

In `apps/agent-gui/src/locales/en.json`, line 69:

```json
// Before:
"profileNotFound": "Unknown model profile: {alias}",
// After:
"modelNotFound": "Model \"{alias}\" is not available",
```

In `apps/agent-gui/src/locales/zh-CN.json`, line 69:

```json
// Before:
"profileNotFound": "未知的模型配置：{alias}",
// After:
"modelNotFound": "模型 \"{alias}\" 不可用",
```

- [ ] **Step 8: Commit Task 2**

```bash
git add crates/agent-models/src/router.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-models/tests/integration.rs apps/agent-gui/src/stores/session.ts apps/agent-gui/src/components/ChatPanel.vue apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json
git commit -m "fix(gui): change 'unknown model profile' to 'unknown model' with friendly frontend error display"
```

---

### Task 3: Fix image thumbnails in attachment chips

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue:2-3, 19-24, 27-40, 87-107, 435-441`

**Approach:** Replace `convertFileSrc()` with `fetch` + `Blob` + `URL.createObjectURL()`. In Tauri v2, `fetch` works with `asset://` URLs, so we can fetch the file data and create a blob URL that the `<img>` tag can load without CSP issues.

- [ ] **Step 1: Replace convertFileSrc with async fetch+blob URL**

In `apps/agent-gui/src/components/ChatPanel.vue`, add a reactive map for thumbnail URLs (after the `attachments` ref, around line 24):

```typescript
const thumbnailUrls = ref<Map<string, string>>(new Map());
```

Replace the `attachmentThumbnailSrc` function (lines 31-33) and `isImageMime` (line 27):

```typescript
function isImageMime(mimeType: string): boolean {
  return mimeType.startsWith("image/");
}

async function loadAttachmentThumbnail(att: Attachment): Promise<void> {
  try {
    const assetUrl = convertFileSrc(att.path);
    const response = await fetch(assetUrl);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const blob = await response.blob();
    const objectUrl = URL.createObjectURL(blob);
    thumbnailUrls.value.set(att.id, objectUrl);
  } catch (e) {
    console.warn(`Failed to load thumbnail for ${att.path}:`, e);
    // thumbnailUrls won't have this id, so template falls back to file icon
  }
}

function getThumbnailUrl(att: Attachment): string | undefined {
  return thumbnailUrls.value.get(att.id);
}
```

- [ ] **Step 2: Trigger thumbnail loading when attachments are added**

In the `pickFiles` function (around line 87), after pushing each attachment:

```typescript
for (const filePath of selected) {
  const name = filePath.split(/[/\\]/).pop() ?? filePath;
  const mimeType = mimeFromExtension(name);
  const att: Attachment = {
    id: crypto.randomUUID(),
    path: filePath,
    name,
    mimeType
  };
  attachments.value = [...attachments.value, att];
  if (isImageMime(mimeType)) {
    loadAttachmentThumbnail(att);
  }
}
```

- [ ] **Step 3: Update template to use new thumbnail source**

In the template, change line 435-437:

```html
<!-- Before: -->
<img
  v-if="isImageMime(att.mimeType)"
  :src="attachmentThumbnailSrc(att)"
  class="attachment-thumbnail"
  :alt="att.name"
  @error="onThumbnailError"
/>

<!-- After: -->
<img
  v-if="isImageMime(att.mimeType) && getThumbnailUrl(att)"
  :src="getThumbnailUrl(att)"
  class="attachment-thumbnail"
  :alt="att.name"
  @error="onThumbnailError"
/>
```

Update the file icon span (line 443) to also show when an image thumbnail failed to load:

```html
<!-- Before: -->
<span v-show="!isImageMime(att.mimeType)" ...>
  <!-- After: -->
  <span v-show="!isImageMime(att.mimeType) || !getThumbnailUrl(att)" ...></span
></span>
```

- [ ] **Step 4: Clean up thumbnail URLs on removal**

In the `removeAttachment` function, add URL cleanup:

```typescript
function removeAttachment(id: string) {
  attachments.value = attachments.value.filter((a) => a.id !== id);
  const url = thumbnailUrls.value.get(id);
  if (url) {
    URL.revokeObjectURL(url);
    thumbnailUrls.value.delete(id);
  }
}
```

Also clear thumbnail URLs when sending a message (in `sendMessage`, after `attachments.value = []`):

```typescript
// Revoke all thumbnail URLs
for (const url of thumbnailUrls.value.values()) {
  URL.revokeObjectURL(url);
}
thumbnailUrls.value.clear();
```

- [ ] **Step 5: Remove unused convertFileSrc import (if no longer needed)**

Check if `convertFileSrc` is used anywhere else in the file. If only in the removed `attachmentThumbnailSrc`, remove it from the import line 2:

```typescript
// Before:
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
// After (if convertFileSrc is still used by loadAttachmentThumbnail, keep it):
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
```

`convertFileSrc` is still used in `loadAttachmentThumbnail`, so keep the import.

- [ ] **Step 6: Commit Task 3**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue
git commit -m "fix(gui): fix image thumbnails in attachment chips using fetch+blob URL"
```

---

### Task 4: Fix archive confirmation icon mismatch

**Files:**

- Modify: `apps/agent-gui/src/components/SessionsSidebar.vue:711-713, 572-574`

The checkmark SVG path from project delete (lines 450-452):

```
d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z"
```

- [ ] **Step 1: Replace regular session archive armed icon**

In `apps/agent-gui/src/components/SessionsSidebar.vue`, lines 711-713, replace the complex SVG path with the checkmark:

```html
<!-- Before (lines 711-713): -->
<path
  d="M7.5 3a.5.5 0 0 1 .5.5v7.87a.5.5 0 0 1-.87.33L4.81 9.36a.75.75 0 1 1 1.02-1.1l.67.62V3.5a.5.5 0 0 1 .5-.5zM4.5 5a.5.5 0 0 1 .5.5v4.13a.5.5 0 0 1-.87.33L1.81 7.36a.75.75 0 0 1 1.02-1.1l.67.62V5.5a.5.5 0 0 1 .5-.5zM10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16zm0-1.5a6.5 6.5 0 1 1 0-13 6.5 6.5 0 0 1 0 13zm2.36-3.97a.75.75 0 0 1-1.06 1.06L9.74 12.03l-1.57 1.56a.75.75 0 1 1-1.06-1.06l1.56-1.57-1.56-1.56a.75.75 0 0 1 1.06-1.06l1.57 1.56 1.56-1.56a.75.75 0 1 1 1.06 1.06l-1.56 1.57 1.56 1.56z"
/>

<!-- After: -->
<path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
```

- [ ] **Step 2: Replace project session archive armed icon**

In `apps/agent-gui/src/components/SessionsSidebar.vue`, lines 572-574, replace the complex SVG path with the same checkmark:

```html
<!-- Before (lines 572-574): -->
<path
  d="M8.5 3.5a.5.5 0 0 0-1 0v1.44a3.5 3.5 0 0 0-3.06 3.5c0 1.7 1.23 3.18 2.95 3.45l.02.01a.5.5 0 0 0 .52-.85A2.51 2.51 0 0 1 5.56 8.44a2.5 2.5 0 0 1 1.94-2.44v3.5a1 1 0 1 0 1.5 0V3.5zM13.06 8.44a2.51 2.51 0 0 1-2.37 2.61.5.5 0 0 0 .52.85h.02a3.51 3.51 0 0 0 2.95-3.46 3.5 3.5 0 0 0-3.06-3.5V9.5a1 1 0 0 1-1.5 0V4.94a3.51 3.51 0 0 0-3.06 3.5c0 1.7 1.23 3.18 2.95 3.45a.5.5 0 0 0 .54-.85 2.51 2.51 0 0 1-2.37-2.61 2.5 2.5 0 0 1 1.94-2.44v7.51a1 1 0 1 0 2 0v-5c.17.03.35.05.53.05a3.5 3.5 0 0 0 3.5-3.5A3.5 3.5 0 0 0 8.5 4.94V3.5a.5.5 0 0 0-1 0v1.44a2.5 2.5 0 0 0-1.94 2.44c0 .36.08.7.22 1.01a.5.5 0 0 0 .5.27.5.5 0 0 0 .38-.43A1.5 1.5 0 0 1 8 6.94v7.07a2 2 0 0 0 4 0v-5c.15.03.3.05.46.05a.5.5 0 0 0 .5-.5.5.5 0 0 0-.46-.5c-.17-.02-.33-.05-.5-.05z"
/>

<!-- After: -->
<path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
```

- [ ] **Step 3: Commit Task 4**

```bash
git add apps/agent-gui/src/components/SessionsSidebar.vue
git commit -m "fix(gui): unify archive confirmation icons with checkmark SVG"
```

---

---

### Task 5: Fix context window display values

**Files:**

- Modify: `crates/agent-core/src/projection.rs:166-168, 40-41`
- Modify: `crates/agent-runtime/src/agent_loop/runner.rs:163-172`
- Modify: `apps/agent-gui/src/components/ContextMeter.vue:21-25, 41-53, 190-215`

- [ ] **Step 1: Fix projection.rs — include ModelProfileSwitched in session projection**

In `crates/agent-core/src/projection.rs`, move `ModelProfileSwitched` from the "not relevant" catch-all (lines 167-168) to active handling. Add handling before the catch-all:

```rust
// After the ContextCompactionFailed handler (around line 165), add:
EventPayload::ModelProfileSwitched {
    context_window,
    output_limit,
    limit_source,
    ..
} => {
    self.model_limits = Some(ProjectedModelLimits {
        context_window: *context_window,
        output_limit: *output_limit,
        source: limit_source.clone(),
    });
}
```

Then remove `ModelProfileSwitched` from the catch-all list:

```rust
// Before (lines 167-168):
EventPayload::WorkspaceOpened { .. }
| EventPayload::ModelProfileSwitched { .. }
| EventPayload::ModelRequestStarted { .. }

// After:
EventPayload::WorkspaceOpened { .. }
| EventPayload::ModelRequestStarted { .. }
```

- [ ] **Step 2: Fix runner.rs — resolve limits from session events when profile not in config**

In `crates/agent-runtime/src/agent_loop/runner.rs`, modify the fallback logic at lines 163-172:

```rust
// Before:
.unwrap_or_else(|| {
    let profile_def = config
        .profiles
        .iter()
        .find(|(alias, _)| alias == &model_profile_alias)
        .map(|(_, def)| def);
    match profile_def {
        Some(def) => agent_config::resolve_limits(def),
        None => agent_models::lookup_limits("fake", "fake"), // pre-0.7 sessions
    }
});

// After:
.unwrap_or_else(|| {
    let profile_def = config
        .profiles
        .iter()
        .find(|(alias, _)| alias == &model_profile_alias)
        .map(|(_, def)| def);
    match profile_def {
        Some(def) => agent_config::resolve_limits(def),
        None => {
            // Profile alias not in current config — extract limits from the
            // last ModelProfileSwitched event (which carries context_window,
            // output_limit, and limit_source directly).
            let from_event = session_events.iter().rev().find_map(|e| {
                if let agent_core::EventPayload::ModelProfileSwitched {
                    context_window,
                    output_limit,
                    limit_source,
                    ..
                } = &e.payload
                {
                    Some(agent_models::ModelLimits {
                        context_window: *context_window,
                        output_limit: *output_limit,
                        source: match limit_source.as_str() {
                            "user_config" => agent_models::LimitSource::UserConfig,
                            "builtin_registry" => agent_models::LimitSource::BuiltinRegistry,
                            "runtime_probe" => agent_models::LimitSource::RuntimeProbe,
                            _ => agent_models::LimitSource::Fallback,
                        },
                    })
                } else {
                    None
                }
            });
            from_event.unwrap_or_else(|| agent_models::lookup_limits("fake", "fake"))
        }
    }
});
```

- [ ] **Step 3: Run Rust tests to verify backend changes**

Run: `cargo test -p agent-core -p agent-runtime --all-targets`
Expected: All tests PASS. If projection tests fail, update expected values.

- [ ] **Step 4: Fix ContextMeter.vue — use modelLimits for display values**

In `apps/agent-gui/src/components/ContextMeter.vue`, add computed properties that derive display values from `modelLimits` when `lastContextUsage` is stale or absent:

Add after `const currentModelContextWindow = computed(...)` (around line 41):

```typescript
// Display budget tokens — uses modelLimits when context usage is stale/absent
const displayBudgetTokens = computed(() => {
  const usage = session.lastContextUsage;
  const limits = session.modelLimits;
  if (!usage && !limits) return 0;
  // If we have modelLimits, calculate the expected budget
  if (limits) {
    const safety = Math.max(2000, Math.floor(limits.output_limit / 10));
    return limits.context_window - (limits.output_limit + safety);
  }
  return usage!.budget_tokens;
});

// Display context window — prefers modelLimits when available
const displayContextWindow = computed(() => {
  if (session.modelLimits) return session.modelLimits.context_window;
  return session.lastContextUsage?.context_window ?? 0;
});

// Whether context usage data matches the current model limits
const contextUsageMatchesModel = computed(() => {
  if (!session.modelLimits || !session.lastContextUsage) return true;
  return session.lastContextUsage.context_window === session.modelLimits.context_window;
});
```

Update the `ratio` computed (line 21-25) to use `displayBudgetTokens`:

```typescript
// Before:
const ratio = computed(() => {
  const u = session.lastContextUsage;
  if (!u || u.budget_tokens === 0) return 0;
  return Math.min(1, u.total_tokens / u.budget_tokens);
});

// After:
const ratio = computed(() => {
  const u = session.lastContextUsage;
  const budget = displayBudgetTokens.value;
  if (!u || budget === 0) return 0;
  return Math.min(1, u.total_tokens / budget);
});
```

Update the `contextWindowSummary` computed (lines 43-53):

```typescript
// Before uses usageContextWindow + currentModelWindow — keep similar but prefer modelLimits
const contextWindowSummary = computed(() => {
  const modelWindow = displayContextWindow.value;
  if (!modelWindow) return t("context.unavailable");
  return formatTokens(modelWindow);
});
```

Update the detail grid template. Change line 201 to use `displayBudgetTokens`:

```html
<!-- Before: -->
<dd>{{ formatTokens(session.lastContextUsage.budget_tokens) }}</dd>

<!-- After: -->
<dd>
  <template v-if="contextUsageMatchesModel"
    >{{ formatTokens(session.lastContextUsage.budget_tokens) }}</template
  >
  <template v-else>
    <span class="estimated-value" :title="t('context.estimatedBudget')"
      >{{ formatTokens(displayBudgetTokens) }}</span
    >
  </template>
</dd>
```

Update line 209 to use `displayContextWindow`:

```html
<!-- Before: -->
<dd>{{ contextWindowSummary }}</dd>

<!-- After: -->
<dd>{{ formatTokens(displayContextWindow) }}</dd>
```

Update line 231 source percentages to use `displayBudgetTokens`:

```html
<!-- Before: -->
formatSourcePercent(tokens, session.lastContextUsage.budget_tokens)

<!-- After: -->
formatSourcePercent(tokens, displayBudgetTokens)
```

- [ ] **Step 5: Add i18n keys for estimated budget tooltip**

In `apps/agent-gui/src/locales/en.json`, `context` section:

```json
"estimatedBudget": "Estimated budget based on model limits. Send a message for exact values."
```

In `apps/agent-gui/src/locales/zh-CN.json`, `context` section:

```json
"estimatedBudget": "基于模型限制的预估预算。发送消息以获取准确数值。"
```

- [ ] **Step 6: Commit Task 5**

```bash
git add crates/agent-core/src/projection.rs crates/agent-runtime/src/agent_loop/runner.rs apps/agent-gui/src/components/ContextMeter.vue apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json
git commit -m "fix(gui): fix context window display using modelLimits, project ModelProfileSwitched, resolve limits from events"
```

---

## Testing

- **Task 1**: Run `pnpm --filter agent-gui run test` to verify store tests pass. Manually verify: create multiple sessions → all get unique "New Session", "New Session 1", etc.
- **Task 2**: Run `cargo test -p agent-models -p agent-runtime --all-targets` for Rust tests. Run `pnpm --filter agent-gui run test` for Vitest.
- **Task 3**: Start `pnpm run tauri dev`, attach an image file, verify thumbnail displays.
- **Task 4**: Visual verification: click archive on a session and project session → both show the same checkmark icon.
- **Task 5**: Run `cargo test -p agent-core -p agent-runtime --all-targets` for Rust tests. Manually verify: switch models → context window max values update immediately. Load saved session → model limits display correctly.

## Final validation

```bash
pnpm run format:check && pnpm run lint && cargo test --workspace --all-targets
```
