<script setup lang="ts">
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
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
const previewAttachment = ref<Attachment | null>(null);
const previewPos = ref({ x: 0, y: 0 });

function isImageMime(mimeType: string): boolean {
  return mimeType.startsWith("image/");
}

function getThumbnailUrl(att: Attachment): string {
  return convertFileSrc(att.path);
}

function onThumbnailError(e: Event) {
  const img = e.target as HTMLImageElement;
  img.style.display = "none";
  const badge = img.nextElementSibling as HTMLElement | null;
  if (badge) badge.style.display = "";
}

const PREVIEW_MAX_HEIGHT = 328; // 320px img + 8px padding

function showPreview(att: Attachment, e: MouseEvent) {
  previewAttachment.value = att;
  clampPreviewPos(e.clientX, e.clientY);
}

function hidePreview() {
  previewAttachment.value = null;
}

function updatePreviewPos(e: MouseEvent) {
  clampPreviewPos(e.clientX, e.clientY);
}

function clampPreviewPos(clientX: number, clientY: number): void {
  let top = clientY - PREVIEW_MAX_HEIGHT / 2;
  if (top < 8) top = 8;
  if (top + PREVIEW_MAX_HEIGHT > window.innerHeight - 8) {
    top = window.innerHeight - PREVIEW_MAX_HEIGHT - 8;
  }
  previewPos.value = { x: clientX + 12, y: top };
}

const FILE_ICON_MAP: Record<string, string> = {
  pdf: "pdf",
  txt: "text",
  md: "text",
  csv: "text",
  log: "text",
  rs: "code",
  py: "code",
  ts: "code",
  js: "code",
  json: "data",
  yaml: "data",
  yml: "data",
  toml: "data",
  xml: "data",
  html: "web",
  css: "web",
  sh: "script",
  bash: "script",
  zsh: "script"
};

function fileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

function fileIconClass(name: string): string {
  const ext = fileExtension(name);
  const category = FILE_ICON_MAP[ext] || "generic";
  return `fi-${category}`;
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
      const att: Attachment = {
        id: crypto.randomUUID(),
        path: filePath as string,
        name,
        mimeType
      };
      attachments.value = [...attachments.value, att];
      // Thumbnails are loaded on-demand via convertFileSrc in the template.
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
    log: "text/plain"
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

const modelOptions = computed<ProfileInfo[]>(() =>
  [...session.profileInfos].sort((a, b) => a.alias.localeCompare(b.alias))
);
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
    const errMsg = String(e);
    if (errMsg.includes("unknown model")) {
      notify("error", t("errors.modelNotFound", { alias }));
    } else {
      notify("error", t("context.switchModelFailed", { error: errMsg }));
    }
  } finally {
    switchingModel.value = false;
  }
}

async function sendMessage() {
  const content = inputText.value.trim();
  if ((!content && attachments.value.length === 0) || session.isStreaming) return;

  const payload: {
    content: string;
    attachments: { path: string; name: string; mime_type: string }[];
  } = {
    content,
    attachments: attachments.value.map((a) => ({
      path: a.path,
      name: a.name,
      mime_type: a.mimeType
    }))
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

<template>
  <section class="chat-panel" data-test="chat-panel">
    <header class="chat-header">
      <h2>{{ t("chat.header") }}</h2>
    </header>

    <div ref="scrollbar" class="message-list" data-test="message-list">
      <div
        class="message-list-inner"
        :data-test="
          session.projection.messages.length === 0 && !session.projection.token_stream
            ? 'chat-empty-state'
            : undefined
        "
      >
        <div
          v-if="session.projection.messages.length === 0 && !session.projection.token_stream"
          class="empty-state"
          data-test="chat-empty-state"
        >
          <svg
            width="48"
            height="48"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            aria-hidden="true"
          >
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
          </svg>
          <p>{{ t("chat.emptyState") }}</p>
        </div>
        <div
          v-for="(msg, i) in session.projection.messages"
          :key="i"
          :class="['message', `message-${roleClass[msg.role] || 'assistant'}`]"
          data-test="chat-message"
          :data-role="roleClass[msg.role] || 'assistant'"
          :data-error="msg.content.startsWith('[error]') ? 'true' : undefined"
        >
          <!-- eslint-disable vue/no-v-html -->
          <span
            v-if="
              msg.role === 'assistant' ||
              msg.role === 'planner' ||
              msg.role === 'worker' ||
              msg.role === 'reviewer'
            "
            class="message-content markdown-body"
            :data-test="msg.content.startsWith('[error]') ? 'error-banner' : undefined"
            v-html="renderMarkdown(msg.content)"
          ></span>
          <!-- eslint-enable vue/no-v-html -->
          <span v-else class="message-content">{{ msg.content }}</span>
        </div>
        <div
          v-if="projectInstructionSummaryText"
          class="project-instruction-summary"
          data-test="project-instruction-summary"
        >
          {{ projectInstructionSummaryText }}
        </div>
        <div
          v-if="session.projection.token_stream"
          class="message message-assistant streaming"
          data-test="stream-indicator"
        >
          <span class="message-content"
            >{{ session.projection.token_stream }}<span class="cursor">▌</span></span
          >
        </div>
        <span
          v-if="session.projection.cancelled"
          class="tag cancelled-marker"
          data-test="cancelled-marker"
        >
          {{ t("chat.cancelled") }}
        </span>
      </div>
    </div>

    <div
      v-if="session.lastSendError"
      class="send-error-banner"
      data-test="error-banner"
      role="alert"
    >
      {{ session.lastSendError }}
    </div>

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
                    <span class="chat-model-option-label">
                      {{ getModelOptionDisplay(profile) }}
                    </span>
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
            :src="getThumbnailUrl(att)"
            class="attachment-thumbnail"
            :alt="att.name"
            @error="onThumbnailError"
            @mouseenter="showPreview(att, $event)"
            @mousemove="updatePreviewPos"
            @mouseleave="hidePreview"
          />
          <span
            v-else
            :class="['attachment-type-icon', fileIconClass(att.name)]"
            :aria-label="att.mimeType"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path
                v-if="fileIconClass(att.name) === 'fi-pdf'"
                d="M6 2h8l6 6v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zm7 1.5V9h5.5L13 3.5zM7 12h10v1.5H7V12zm0 3h8v1.5H7V15zm0 3h10v1.5H7V18z"
              />
              <path
                v-else-if="fileIconClass(att.name) === 'fi-code'"
                d="M14.6 16.6l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4zm-5.2 0L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4z"
              />
              <path
                v-else-if="fileIconClass(att.name) === 'fi-data'"
                d="M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm0 2v2h2V5H5zm4 0v2h2V5H9zm4 0v2h2V5h-2zm4 0v2h2V5h-2zM3 9h18v2H3V9zm0 4h18v2H3v-2zm0 4h18v2H3v-2z"
              />
              <path
                v-else-if="fileIconClass(att.name) === 'fi-web'"
                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"
              />
              <path
                v-else-if="fileIconClass(att.name) === 'fi-text'"
                d="M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm2 4v2h10V7H7zm0 4v2h10v-2H7zm0 4v2h7v-2H7z"
              />
              <path
                v-else-if="fileIconClass(att.name) === 'fi-script'"
                d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zM6 20V4h7v5h5v11H6zm2-6h8v2H8v-2zm0-4h4v2H8v-2z"
              />
              <path
                v-else
                d="M6 2h8l6 6v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zm7 1.5V9h5.5L13 3.5z"
              />
            </svg>
          </span>
          <span v-if="!isImageMime(att.mimeType)" class="attachment-name" :title="att.name">{{
            truncateFilename(att.name)
          }}</span>
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
    <Teleport to="body">
      <div
        v-if="previewAttachment"
        class="thumbnail-preview-overlay"
        :style="{ left: previewPos.x + 'px', top: previewPos.y + 'px' }"
        @mouseleave="hidePreview"
      >
        <img
          :src="getThumbnailUrl(previewAttachment)"
          class="thumbnail-preview-image"
          :alt="previewAttachment.name"
        />
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
}
.chat-header h2 {
  margin: 0;
  font-size: 14px;
}
.message-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.message-list-inner {
  padding: 12px 16px;
}
.message {
  display: flex;
  margin-bottom: 12px;
  line-height: 1.5;
}
.project-instruction-summary {
  margin-bottom: 12px;
  color: var(--app-muted-text-color, var(--app-text-color));
  font-size: 12px;
  line-height: 1.5;
}
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px 16px;
  color: var(--app-muted-text-color, var(--app-text-color));
  opacity: 0.72;
}
.empty-state p {
  margin: 0;
  font-size: 13px;
}
.message-content {
  max-width: min(760px, 82%);
  border-radius: var(--app-radius-xl);
  padding: 10px 12px;
  white-space: pre-wrap;
  overflow-wrap: break-word;
}
.message-user {
  justify-content: flex-end;
}
.message-user .message-content {
  color: var(--app-primary-contrast, #ffffff);
  background: var(--app-primary-color, #0077cc);
}
.message-assistant,
.message-planner,
.message-worker,
.message-reviewer,
.message-system {
  justify-content: flex-start;
}
.message-assistant .message-content,
.message-planner .message-content,
.message-worker .message-content,
.message-reviewer .message-content,
.message-system .message-content {
  color: var(--app-muted-text-color, var(--app-text-color));
  background: var(--app-muted-surface-color, var(--app-card-color));
}
.message-system .message-content {
  opacity: 0.72;
  font-style: italic;
}
.streaming .cursor {
  animation: blink 1s step-end infinite;
}
.cancelled-marker {
  margin-top: 4px;
}
@keyframes blink {
  50% {
    opacity: 0;
  }
}
.tag {
  display: inline-block;
  padding: 0 8px;
  font-size: 12px;
  line-height: 22px;
  border-radius: 3px;
  background: var(--app-tag-color, color-mix(in srgb, var(--app-primary-color) 10%, transparent));
  color: var(--app-text-color);
}
.chat-model-trigger {
  max-width: min(100%, 280px);
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--app-primary-color) 22%, var(--app-border-color));
  border-radius: 999px;
  padding: 3px 10px;
  cursor: pointer;
  background: color-mix(in srgb, var(--app-primary-color) 10%, var(--app-card-color));
  color: var(--app-text-color);
  font: inherit;
  font-size: 12px;
  line-height: 18px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chat-model-trigger:hover {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 16%, var(--app-card-color));
}
.chat-model-trigger:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-model-trigger {
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }
}
.chat-model-popover-panel {
  min-width: 240px;
}
.chat-model-popover-header {
  margin-bottom: 8px;
  color: var(--app-text-color-2, var(--app-muted-text-color));
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.chat-model-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 0;
  margin: 0;
  list-style: none;
}
.chat-model-option {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font: inherit;
  text-align: left;
}
.chat-model-option:hover:not(:disabled) {
  border-color: var(--app-border-color);
  background: var(--app-hover-color, color-mix(in srgb, var(--app-primary-color) 8%, transparent));
}
.chat-model-option.selected {
  border-color: color-mix(in srgb, var(--app-primary-color) 45%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
}
.chat-model-option:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.chat-model-option-label {
  max-width: 100%;
  overflow: hidden;
  font-size: 13px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chat-model-option-meta {
  color: var(--app-text-color-3, var(--app-muted-text-color));
  font-size: 11px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-model-popover-panel {
    animation: popover-in 0.15s ease;
  }
}
@keyframes popover-in {
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
.cancelled-marker.tag {
  background: color-mix(in srgb, var(--app-warning-color, #faad14) 15%, transparent);
  color: var(--app-warning-color, #faad14);
}
.btn {
  padding: 6px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-primary {
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-primary-color);
}
.btn-error {
  background: var(--app-error-color, #d03050);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-error-color, #d03050);
}
.message-input {
  flex: 1;
  min-width: 0;
  resize: vertical;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 4px;
  padding: 6px 10px;
  font-size: 13px;
  font-family: inherit;
  background: var(--app-card-color);
  color: var(--app-text-color);
  outline: none;
  width: 100%;
  box-sizing: border-box;
}
.message-input:focus {
  border-color: var(--app-primary-color);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--app-primary-color) 25%, transparent);
}
.message-input:disabled {
  opacity: 0.5;
}
.send-error-banner {
  margin: 8px 16px 0;
  padding: 8px 10px;
  border: 1px solid var(--app-error-color, #d03050);
  border-radius: 4px;
  background: color-mix(in srgb, var(--app-error-color, #d03050) 10%, transparent);
  color: var(--app-error-color, #d03050);
  font-size: 13px;
}
.input-area {
  padding: 8px 16px;
  border-top: 1px solid var(--app-border-color, #d7d7d7);
}
.composer-meta {
  display: flex;
  min-width: 0;
  overflow: hidden;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  margin-bottom: 6px;
  color: var(--app-muted-text-color, var(--app-text-color));
  font-size: 12px;
}
.git-meta {
  min-width: 0;
  max-width: min(100%, 420px);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.72;
}
.input-row {
  display: flex;
  gap: 8px;
  align-items: flex-end;
}
.markdown-body :deep(pre.hljs) {
  margin: 8px 0;
  border-radius: 6px;
  padding: 12px;
  overflow-x: auto;
  font-size: 13px;
  line-height: 1.5;
}
.markdown-body :deep(code) {
  font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
}
.markdown-body :deep(:not(pre) > code) {
  background: var(--app-card-color);
  padding: 2px 4px;
  border-radius: 3px;
  font-size: 12px;
}
.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  padding-left: 20px;
}
.markdown-body :deep(p) {
  margin: 6px 0;
}
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
.attachment-type-icon {
  width: 36px;
  height: 36px;
  border-radius: 4px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
  color: var(--app-primary-color);
}
.attachment-type-icon svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
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
.thumbnail-preview-overlay {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  padding: 4px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}
.thumbnail-preview-image {
  display: block;
  max-width: 320px;
  max-height: 320px;
  border-radius: 6px;
  object-fit: contain;
}
</style>
