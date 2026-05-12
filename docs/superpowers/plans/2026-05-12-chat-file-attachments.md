# Chat File Attachments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add file attachment support to the GUI chat input — system file picker, thumbnail/icon previews, removable chips, and file content sent to the AI model.

**Architecture:** Frontend (Vue) handles file selection and preview UI. The Tauri command reads file bytes, formats content (images as base64 data URIs, text files as labeled blocks), and passes enriched content through the existing `runtime.send_message()` path. No changes to the runtime or agent loop are needed.

**Tech Stack:** Vue 3 + TypeScript, Tauri 2, Rust, `tauri-plugin-dialog` (already installed)

---

### File Structure Map

| File                                          | Role                                                                               |
| --------------------------------------------- | ---------------------------------------------------------------------------------- |
| `crates/agent-core/src/facade.rs`             | New `AttachmentInfo` type, update `SendMessageRequest`                             |
| `apps/agent-gui/src-tauri/src/commands.rs`    | Updated `send_message` command — accepts attachments, reads files, formats content |
| `apps/agent-gui/src/components/ChatPanel.vue` | Attachment UI (row, chips, thumbnails, remove), file picker, modified send         |
| `apps/agent-gui/src/generated/commands.ts`    | Auto-regenerated via `just gen-types`                                              |

---

### Task 1: Add AttachmentInfo type and update SendMessageRequest

**Files:**

- Modify: `crates/agent-core/src/facade.rs:339-345`

- [ ] **Step 1: Add AttachmentInfo struct and update SendMessageRequest**

Insert `AttachmentInfo` before `SendMessageRequest` (after line 338), and add `attachments` field:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// Metadata for a file attached to a user message.
pub struct AttachmentInfo {
    /// Absolute filesystem path.
    pub path: String,
    /// Display filename.
    pub name: String,
    /// MIME type (e.g. "image/png", "application/pdf").
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to send a user message to an active session.
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
    pub attachments: Vec<AttachmentInfo>,
}
```

- [ ] **Step 2: Fix the NoopFacade test impl**

In the `NoopFacade` (line ~1006), update `send_message` — the `request` already destructures with `let _ = request;` so it compiles as-is. No change needed.

- [ ] **Step 3: Build check**

```bash
cargo build -p agent-core 2>&1
```

Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/facade.rs
git commit -m "feat(core): add AttachmentInfo type and update SendMessageRequest"
```

---

### Task 2: Update Tauri send_message command

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs:344-383`

- [ ] **Step 1: Replace the send_message command**

Replace lines 344-383 with the updated command that accepts attachments, reads files, and formats content:

````rust
#[tauri::command]
#[specta::specta]
pub async fn send_message(
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };

    let enriched = enrich_content_with_attachments(&content, &attachments).await;

    let session_id_str = session_id.to_string();
    let runtime = state.runtime.clone();
    tokio::spawn(async move {
        let result = runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id,
                session_id,
                content: enriched,
                attachments,
            })
            .await;

        if let Err(e) = result {
            eprintln!("[commands] send_message failed: {e}");
            let payload = serde_json::json!({
                "type": "SendMessageError",
                "error": e.to_string(),
                "session_id": session_id_str
            });
            let _ = app_handle.emit("session-error", &payload);
        }
    });

    Ok(())
}

/// Read attachment files and format their content into the message.
///
/// - Images: base64-encoded data URIs appended to the content.
/// - Text files: content wrapped in markdown code blocks with filename headers.
/// - Other binaries: filename reference only.
async fn enrich_content_with_attachments(content: &str, attachments: &[agent_core::AttachmentInfo]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for att in attachments {
        let mime = att.mime_type.as_str();
        if mime.starts_with("image/") {
            match std::fs::read(&att.path) {
                Ok(bytes) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    parts.push(format!(
                        "![{}](data:{};base64,{})",
                        att.name, mime, b64
                    ));
                }
                Err(e) => {
                    eprintln!("[commands] failed to read image {}: {e}", att.path);
                    parts.push(format!("[image: {} (read error)]", att.name));
                }
            }
        } else if is_text_mime(mime) {
            match std::fs::read_to_string(&att.path) {
                Ok(text) => {
                    let ext = std::path::Path::new(&att.name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    parts.push(format!(
                        "```{}\n// file: {}\n{}\n```",
                        ext, att.name, text
                    ));
                }
                Err(e) => {
                    eprintln!("[commands] failed to read file {}: {e}", att.path);
                    parts.push(format!("[file: {} (read error)]", att.name));
                }
            }
        } else {
            parts.push(format!("[attached file: {}]", att.name));
        }
    }

    if parts.is_empty() {
        content.to_string()
    } else if content.trim().is_empty() {
        parts.join("\n\n")
    } else {
        format!("{}\n\n{}", parts.join("\n\n"), content)
    }
}

fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.contains("json")
        || mime.contains("xml")
        || mime.contains("javascript")
        || mime.contains("yaml")
        || mime.contains("toml")
        || mime == "application/x-sh"
        || mime == "application/x-shellscript"
}
````

- [ ] **Step 2: Add base64 dependency to Cargo.toml**

Check if `base64` is already a dependency:

```bash
grep -r "base64" apps/agent-gui/src-tauri/Cargo.toml
```

If not present, add to `[dependencies]`:

```toml
base64 = "0.22"
```

- [ ] **Step 3: Build check**

```bash
cargo build -p agent-gui 2>&1 | tail -20
```

Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/Cargo.toml
git commit -m "feat(gui): accept attachments in send_message command with file reading"
```

---

### Task 3: Add attachment UI to ChatPanel.vue

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue`

- [ ] **Step 1: Update the script section**

Replace the `<script setup>` block (lines 1-145) with the updated version:

```typescript
<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { formatProfileDisplay, useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import { useNotifications } from "@/composables/useNotifications";
import { renderMarkdown } from "../utils/markdown";
import type { ProfileInfo, ProjectedRole } from "../types";

const { t } = useI18n();
const session = useSessionStore();
const projectStore = useProjectStore();
const { notify } = useNotifications();
const inputText = ref("");
const scrollbar = ref<HTMLElement | null>(null);
const modelPopoverOpen = ref(false);
const switchingModel = ref(false);

interface Attachment {
  id: string;
  path: string;
  name: string;
  mimeType: string;
}
const attachments = ref<Attachment[]>([]);

function isImageMime(mimeType: string): boolean {
  return mimeType.startsWith("image/");
}

function attachmentThumbnailSrc(att: Attachment): string {
  return convertFileSrc(att.path);
}

function attachmentTypeLabel(mimeType: string): string {
  const parts = mimeType.split("/");
  if (parts.length < 2) return "FILE";
  const subtype = parts[1].toUpperCase();
  if (subtype.length <= 4) return subtype;
  return subtype.slice(0, 4);
}

function truncateFilename(name: string, maxLen = 18): string {
  if (name.length <= maxLen) return name;
  const ext = name.lastIndexOf(".");
  if (ext > 0) {
    const base = name.slice(0, ext);
    const suffix = name.slice(ext);
    const available = maxLen - suffix.length - 1;
    if (available > 4) return base.slice(0, available) + "…" + suffix;
  }
  return name.slice(0, maxLen - 1) + "…";
}

async function pickFiles() {
  try {
    const selected = await open({ multiple: true });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    for (const filePath of paths) {
      if (!filePath) continue;
      const name = (filePath as string).split(/[\\/]/).pop() || (filePath as string);
      const ext = name.split(".").pop()?.toLowerCase() || "";
      const mimeType = mimeFromExtension(ext);
      attachments.value.push({
        id: crypto.randomUUID(),
        path: filePath as string,
        name,
        mimeType,
      });
    }
  } catch (e) {
    console.error("File picker error:", e);
  }
}

function mimeFromExtension(ext: string): string {
  const map: Record<string, string> = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
    bmp: "image/bmp",
    pdf: "application/pdf",
    txt: "text/plain",
    md: "text/markdown",
    rs: "text/x-rust",
    py: "text/x-python",
    ts: "text/typescript",
    js: "text/javascript",
    json: "application/json",
    yaml: "application/x-yaml",
    yml: "application/x-yaml",
    toml: "application/toml",
    html: "text/html",
    css: "text/css",
    csv: "text/csv",
    xml: "application/xml",
    sh: "application/x-sh",
    bash: "application/x-sh",
    zsh: "application/x-sh",
    log: "text/plain",
  };
  return map[ext] || "application/octet-stream";
}

function removeAttachment(id: string) {
  attachments.value = attachments.value.filter((a) => a.id !== id);
}

/** Map role to CSS class suffix. */
const roleClass: Record<ProjectedRole, string> = {
  user: "user",
  assistant: "assistant",
  planner: "planner",
  worker: "worker",
  reviewer: "reviewer",
  system: "system"
};

const currentSession = computed(() => session.currentSessionInfo);

const sessionGitMeta = computed(() => {
  const sessionInfo = currentSession.value;
  if (!sessionInfo?.project_id && !sessionInfo?.worktree_path) return [];

  const gitMetaParts = [];
  if (sessionInfo.branch) gitMetaParts.push(sessionInfo.branch);
  else if (sessionInfo.worktree_path) gitMetaParts.push(sessionInfo.worktree_path);
  if (!gitMetaParts.length && sessionInfo.project_id) gitMetaParts.push(sessionInfo.project_id);
  return gitMetaParts;
});

const currentProjectId = computed(() => currentSession.value?.project_id ?? null);
const isEmptyProjectChat = computed(
  () =>
    Boolean(currentProjectId.value) &&
    session.projection.messages.length === 0 &&
    !session.projection.token_stream
);
const projectInstructionSummaryText = computed(() => {
  const projectId = currentProjectId.value;
  if (!projectId || !isEmptyProjectChat.value) return null;

  const instructionSummary = projectStore.instructionSummariesByProject.get(projectId);
  const sourceFileNames =
    instructionSummary?.sourcePaths
      .map((sourcePath) => sourcePath.split(/[\\/]/).filter(Boolean).at(-1))
      .filter((fileName): fileName is string => Boolean(fileName)) ?? [];
  if (!sourceFileNames.length) return null;

  return `Loaded ${sourceFileNames.join(", ")}`;
});

const modelOptions = computed<ProfileInfo[]>(() => session.profileInfos);
const sendDisabled = computed(
  () => session.isStreaming || (!inputText.value.trim() && attachments.value.length === 0)
);

function getModelOptionDisplay(profile: ProfileInfo): string {
  return formatProfileDisplay(profile);
}

async function selectModelProfile(alias: string) {
  if (switchingModel.value) return;
  if (alias === session.currentProfile) {
    modelPopoverOpen.value = false;
    return;
  }
  if (!session.currentSessionId) return;

  switchingModel.value = true;
  try {
    await invoke("switch_model", {
      sessionId: session.currentSessionId,
      profileAlias: alias
    });
    session.currentProfile = alias;
    modelPopoverOpen.value = false;
  } catch (e) {
    console.error("Failed to switch model:", e);
    notify("error", String(e));
  } finally {
    switchingModel.value = false;
  }
}

async function sendMessage() {
  const content = inputText.value.trim();
  if ((!content && attachments.value.length === 0) || session.isStreaming) return;

  const payload: { content: string; attachments: { path: string; name: string; mime_type: string }[] } = {
    content,
    attachments: attachments.value.map((a) => ({
      path: a.path,
      name: a.name,
      mime_type: a.mimeType,
    })),
  };

  inputText.value = "";
  attachments.value = [];
  try {
    await invoke("send_message", payload);
  } catch (e) {
    console.error("Failed to send message:", e);
    session.reportSendError(String(e));
    notify("error", t("chat.sendFailed", { error: String(e) }));
  }
}

async function cancelSession() {
  try {
    await invoke("cancel_session");
  } catch (e) {
    console.error("Failed to cancel session:", e);
    notify("error", t("chat.cancelFailed", { error: String(e) }));
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

onMounted(() => {
  void session.loadProfileInfo();
});

watch(
  () => currentProjectId.value,
  async (projectId) => {
    if (!projectId || projectStore.instructionSummariesByProject.has(projectId)) return;
    await projectStore.getProjectInstructionSummary(projectId);
  },
  { immediate: true }
);

watch(
  () => [session.projection.messages.length, session.projection.token_stream],
  async () => {
    await nextTick();
    if (scrollbar.value) {
      scrollbar.value.scrollTo({ top: scrollbar.value.scrollHeight, behavior: "smooth" });
    }
  }
);
</script>
```

- [ ] **Step 2: Update the template — add attachment row**

Replace the `input-area` div (lines 220-302) with:

```html
<div class="input-area">
  <div class="composer-meta">
    <KxPopover
      v-model:open="modelPopoverOpen"
      content-data-test="chat-model-popover"
      side="top"
      align="start"
    >
      <template #trigger>
        <button
          class="chat-model-trigger"
          type="button"
          data-test="chat-model-trigger"
          :aria-label="t('chat.selectModelAria', { model: session.activeProfileDisplay })"
        >
          {{ session.activeProfileDisplay }}
        </button>
      </template>
      <template #content>
        <div class="chat-model-popover-panel">
          <header class="chat-model-popover-header">{{ t("chat.model") }}</header>
          <ul class="chat-model-list">
            <li v-for="profile in modelOptions" :key="profile.alias">
              <button
                type="button"
                :class="[
                      'chat-model-option',
                      { selected: profile.alias === session.currentProfile }
                    ]"
                :data-test="`chat-model-option-${profile.alias}`"
                :aria-current="profile.alias === session.currentProfile ? 'true' : undefined"
                :disabled="switchingModel"
                @click="selectModelProfile(profile.alias)"
              >
                <span class="chat-model-option-label"> {{ getModelOptionDisplay(profile) }} </span>
                <span class="chat-model-option-meta">
                  {{ profile.alias }}
                  <span v-if="profile.alias === session.currentProfile">
                    · {{ t("chat.currentModel") }}</span
                  >
                </span>
              </button>
            </li>
          </ul>
        </div>
      </template>
    </KxPopover>
    <span v-if="sessionGitMeta.length" class="git-meta" data-test="session-git-meta">
      {{ sessionGitMeta.join(" · ") }}
    </span>
  </div>
  <div v-if="attachments.length > 0" class="attachment-row" data-test="attachment-row">
    <button
      class="attach-btn attach-btn-inline"
      type="button"
      data-test="attach-file-btn"
      :aria-label="t('chat.attachFileAria')"
      :disabled="session.isStreaming"
      @click="pickFiles"
    >
      +
    </button>
    <div
      v-for="att in attachments"
      :key="att.id"
      class="attachment-chip"
      data-test="attachment-chip"
      :data-filename="att.name"
    >
      <img
        v-if="isImageMime(att.mimeType)"
        :src="attachmentThumbnailSrc(att)"
        class="attachment-thumbnail"
        :alt="att.name"
      />
      <span v-else class="attachment-type-badge">{{ attachmentTypeLabel(att.mimeType) }}</span>
      <span class="attachment-name" :title="att.name">{{ truncateFilename(att.name) }}</span>
      <button
        class="attachment-remove"
        type="button"
        :aria-label="t('chat.removeFileAria', { name: att.name })"
        data-test="attachment-remove"
        @click="removeAttachment(att.id)"
      >
        &times;
      </button>
    </div>
  </div>
  <div class="input-row">
    <button
      v-if="attachments.length === 0"
      class="attach-btn"
      type="button"
      data-test="attach-file-btn"
      :aria-label="t('chat.attachFileAria')"
      :disabled="session.isStreaming"
      @click="pickFiles"
    >
      +
    </button>
    <textarea
      v-model="inputText"
      class="message-input"
      data-test="message-input"
      :disabled="session.isStreaming"
      rows="1"
      :placeholder="t('chat.placeholder')"
      @keydown="handleKeydown"
    />
    <ContextMeter variant="ring" />
    <button
      v-if="session.isStreaming"
      class="btn btn-error"
      data-test="cancel-button"
      @click="cancelSession"
    >
      {{ t("common.cancel") }}
    </button>
    <button
      v-else
      class="btn btn-primary"
      data-test="send-button"
      :disabled="sendDisabled"
      @click="sendMessage"
    >
      {{ t("common.send") }}
    </button>
  </div>
</div>
```

- [ ] **Step 3: Add styles for attachment components**

Append the following styles to the existing `<style scoped>` block (after line 587):

```css
.attachment-row {
  display: flex;
  gap: 6px;
  align-items: center;
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
  overflow-x: auto;
  overflow-y: hidden;
}
.attachment-chip {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  background: var(--app-muted-surface-color, var(--app-card-color));
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  padding: 3px 6px 3px 3px;
  max-width: 200px;
}
.attachment-thumbnail {
  width: 36px;
  height: 36px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
}
.attachment-type-badge {
  width: 36px;
  height: 36px;
  border-radius: 4px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  font-weight: 700;
  background: color-mix(in srgb, var(--app-primary-color) 16%, transparent);
  color: var(--app-primary-color);
  text-transform: uppercase;
  letter-spacing: 0.02em;
}
.attachment-name {
  font-size: 12px;
  color: var(--app-text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100px;
}
.attachment-remove {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--app-muted-text-color, var(--app-text-color));
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}
.attachment-remove:hover {
  background: color-mix(in srgb, var(--app-error-color, #d03050) 16%, transparent);
  color: var(--app-error-color, #d03050);
}
.attach-btn {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border: 1px dashed var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: transparent;
  color: var(--app-muted-text-color, var(--app-text-color));
  cursor: pointer;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  line-height: 1;
}
.attach-btn:hover:not(:disabled) {
  border-color: var(--app-primary-color);
  color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
.attach-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.attach-btn-inline {
  width: 28px;
  height: 28px;
  font-size: 16px;
}
```

- [ ] **Step 4: Add i18n keys**

Check if `chat.attachFileAria` and `chat.removeFileAria` exist in the locale files:

```bash
grep -r "attachFileAria\|removeFileAria" apps/agent-gui/src/locales/ 2>/dev/null || echo "not found"
```

If not found, add to the English locale file (`apps/agent-gui/src/locales/en.json`) in the `chat` section:

```json
"attachFileAria": "Attach file",
"removeFileAria": "Remove {name}"
```

- [ ] **Step 5: Type check and lint**

```bash
pnpm --filter agent-gui run type-check 2>&1
pnpm run lint 2>&1 | tail -10
```

Expected: passes both.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue apps/agent-gui/src/locales/en.json
git commit -m "feat(gui): add file attachment UI with picker, thumbnails, and remove"
```

---

### Task 4: Regenerate TypeScript bindings

**Files:**

- Auto-regenerate: `apps/agent-gui/src/generated/commands.ts`

- [ ] **Step 1: Run gen-types**

```bash
just gen-types 2>&1
```

Expected: regenerates `commands.ts` without errors.

- [ ] **Step 2: Verify the generated binding**

Check that `sendMessage` in `commands.ts` now accepts an `attachments` parameter:

```bash
grep -A3 "sendMessage:" apps/agent-gui/src/generated/commands.ts
```

Expected: shows new signature with `attachments` parameter.

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/generated/commands.ts
git commit -m "chore(gui): regenerate TypeScript bindings for attachments"
```

---

### Task 5: Full build verification

- [ ] **Step 1: Build the entire workspace**

```bash
cargo build --workspace 2>&1 | tail -10
```

Expected: compiles cleanly.

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace --all-targets 2>&1 | tail -20
pnpm --filter agent-gui run test 2>&1 | tail -10
```

Expected: same results as baseline (pre-existing MCP failures only, 401 GUI tests passing).

- [ ] **Step 3: Run lint and format checks**

```bash
pnpm run format:check 2>&1
pnpm run lint 2>&1 | tail -10
```

Expected: passes both.

- [ ] **Step 4: Report ready**

Report: "All checks pass. Ready for manual GUI smoke test: click + button → select files → verify thumbnails/chips → send message → verify files reach the model."

---

### Task 6: Fix permission panel i18n

**Files:**

- Modify: `apps/agent-gui/src/components/PermissionCenter.vue:1-28`
- Modify: `apps/agent-gui/src/components/PermissionPrompt.vue:1-152`
- Modify: `apps/agent-gui/src/locales/en.json`

- [ ] **Step 1: Add i18n keys to en.json**

Add the following to the `permission` section in `apps/agent-gui/src/locales/en.json`:

```json
"permission": {
  "allow": "Allow",
  "deny": "Deny",
  "accept": "Accept",
  "reject": "Reject",
  "titlePermissionRequired": "Permission Required",
  "titleMemoryProposed": "Memory Proposed",
  "panelTitle": "Permissions",
  "emptyState": "No pending requests",
  "scopePrefix": "Scope",
  "storeLabel": "Store",
  "toolLabel": "Tool",
  "mcpServerPrefix": "MCP Server",
  "mcpTrustedBadge": "Trusted",
  "mcpTrustCheckbox": "Trust this server for future requests"
}
```

- [ ] **Step 2: Update PermissionCenter.vue to use i18n**

Replace lines 1-28 with:

```vue
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";

const { t } = useI18n();

const pendingEntries = computed(() =>
  traceState.entries.filter(
    (e) => (e.kind === "permission" || e.kind === "memory") && e.status === "pending"
  )
);
</script>

<template>
  <div class="card permission-center">
    <div class="card-header">
      <h2>{{ t("permission.panelTitle") }}</h2>
    </div>
    <div class="card-content">
      <div v-if="pendingEntries.length === 0" class="empty-state">
        {{ t("permission.emptyState") }}
      </div>
      <ul v-else class="permission-list">
        <li v-for="entry in pendingEntries" :key="entry.id" class="permission-list-item">
          <PermissionPrompt :entry="entry" />
        </li>
      </ul>
    </div>
  </div>
</template>
```

- [ ] **Step 3: Update PermissionPrompt.vue hardcoded strings**

Replace the 5 hardcoded text locations in the template:

**Line 106** — `Scope: {{ entry.scope }}` becomes:

```html
<div v-if="entry.scope" class="permission-meta">
  {{ t("permission.scopePrefix") }}: {{ entry.scope }}
</div>
```

**Line 110** — `{{ isMemory ? "Store" : "Tool" }}: {{ entry.toolId }}` becomes:

```html
<div class="permission-meta">
  {{ isMemory ? t("permission.storeLabel") : t("permission.toolLabel") }}: {{ entry.toolId }}
</div>
```

**Line 116** — `MCP Server: ` becomes:

```html
{{ t("permission.mcpServerPrefix") }}: <strong>{{ mcpServerId }}</strong>
```

**Line 118** — `✅ Trusted` becomes:

```html
✅ {{ t("permission.mcpTrustedBadge") }}
```

**Line 134** — `Trust this server for future requests` becomes:

```html
{{ t("permission.mcpTrustCheckbox") }}
```

- [ ] **Step 4: Verify tests pass**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -10
```

Expected: 401+ tests passing.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/PermissionCenter.vue apps/agent-gui/src/components/PermissionPrompt.vue apps/agent-gui/src/locales/en.json
git commit -m "fix(gui): add i18n support to permission panel"
```
