<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
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
const sendDisabled = computed(() => session.isStreaming || !inputText.value.trim());

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
  if (!content || session.isStreaming) return;

  inputText.value = "";
  try {
    await invoke("send_message", { content });
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
      <div class="input-row">
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
.message-content {
  max-width: min(760px, 82%);
  border-radius: 16px;
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
</style>
