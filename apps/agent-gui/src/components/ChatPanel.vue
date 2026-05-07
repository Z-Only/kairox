<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useAgentsStore } from "@/stores/agents";
import { useNotifications } from "@/composables/useNotifications";
import { renderMarkdown } from "../utils/markdown";
import type { ProjectedRole } from "../types";

const { t } = useI18n();
const session = useSessionStore();
const agents = useAgentsStore();
const { notify } = useNotifications();
const inputText = ref("");
const scrollbar = ref<HTMLElement | null>(null);

/**
 * Map role to display label. Uses the locale's translations for the two
 * user-facing roles (`user` → "You", `assistant` → "Agent"); the more
 * specific Planner/Worker/Reviewer/System labels are intentionally kept in
 * English because they refer to the agent-system role taxonomy and appear
 * verbatim in trace events / system logs (see Task 7b carry-over #6 — only
 * the user-facing greeting strings are translated here).
 */
const roleDisplay = computed<Record<ProjectedRole, string>>(() => ({
  user: t("chat.roleYou"),
  assistant: t("chat.roleAgent"),
  planner: "Planner",
  worker: "Worker",
  reviewer: "Reviewer",
  system: "System"
}));

/** Map role to CSS class suffix. */
const roleClass: Record<ProjectedRole, string> = {
  user: "user",
  assistant: "assistant",
  planner: "planner",
  worker: "worker",
  reviewer: "reviewer",
  system: "system"
};

/** Get the display label for a message, including agent attribution if available. */
function messageLabel(msg: (typeof session.projection.messages)[0]): string {
  const base = roleDisplay.value[msg.role] || t("chat.roleAgent");
  if (msg.sourceAgentId && msg.role !== "user" && msg.role !== "system") {
    const label = agents.agentLabel(msg.sourceAgentId);
    if (label) return `${base} (${label})`;
  }
  return base;
}

const sendDisabled = computed(() => session.isStreaming || !inputText.value.trim());

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
      <NTag size="small" :bordered="false" data-test="chat-profile-badge">
        {{ session.currentProfile }}
      </NTag>
    </header>

    <NScrollbar ref="scrollbar" class="message-list" data-test="message-list">
      <div class="message-list-inner">
        <div
          v-for="(msg, i) in session.projection.messages"
          :key="i"
          :class="['message', `message-${roleClass[msg.role] || 'assistant'}`]"
        >
          <span :class="['message-role', `role-badge-${roleClass[msg.role] || 'assistant'}`]">{{
            messageLabel(msg)
          }}</span>
          <!-- eslint-disable vue/no-v-html -->
          <span
            v-if="
              msg.role === 'assistant' ||
              msg.role === 'planner' ||
              msg.role === 'worker' ||
              msg.role === 'reviewer'
            "
            class="message-content markdown-body"
            v-html="renderMarkdown(msg.content)"
          ></span>
          <!-- eslint-enable vue/no-v-html -->
          <span v-else class="message-content">{{ msg.content }}</span>
        </div>
        <div v-if="session.projection.token_stream" class="message message-assistant streaming">
          <span class="message-role">{{ t("chat.roleAgent") }}</span>
          <span class="message-content"
            >{{ session.projection.token_stream }}<span class="cursor">▌</span></span
          >
        </div>
        <NTag
          v-if="session.projection.cancelled"
          type="warning"
          size="small"
          :bordered="false"
          class="cancelled-marker"
          data-test="cancelled-marker"
        >
          {{ t("chat.cancelled") }}
        </NTag>
      </div>
    </NScrollbar>

    <div class="input-area">
      <div class="input-row">
        <NInput
          v-model:value="inputText"
          type="textarea"
          class="message-input"
          data-test="message-input"
          :disabled="session.isStreaming"
          :autosize="{ minRows: 1, maxRows: 6 }"
          :placeholder="t('chat.placeholder')"
          :style="{ width: '100%' }"
          @keydown="handleKeydown"
        />
        <NButton
          v-if="session.isStreaming"
          type="error"
          data-test="cancel-button"
          @click="cancelSession"
        >
          {{ t("common.cancel") }}
        </NButton>
        <NButton
          v-else
          type="primary"
          data-test="send-button"
          :disabled="sendDisabled"
          @click="sendMessage"
        >
          {{ t("common.send") }}
        </NButton>
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
}
.message-list-inner {
  padding: 12px 16px;
}
.message {
  margin-bottom: 12px;
  line-height: 1.5;
}
.message-user .message-role {
  color: var(--app-primary-color, #0077cc);
  font-weight: 600;
}
.message-assistant .message-role {
  color: var(--app-success-color, #22a06b);
  font-weight: 600;
}
.message-planner .message-role {
  color: var(--app-primary-color, #0077cc);
  font-weight: 600;
}
.message-worker .message-role {
  color: var(--app-success-color, #22a06b);
  font-weight: 600;
}
.message-reviewer .message-role {
  color: var(--app-info-color, #7c3aed);
  font-weight: 600;
}
.message-system .message-role {
  color: var(--app-text-color);
  opacity: 0.6;
  font-weight: 600;
  font-style: italic;
}
.message-system .message-content {
  color: var(--app-text-color);
  opacity: 0.6;
  font-style: italic;
}
.message-role {
  margin-right: 6px;
}
.message-content {
  white-space: pre-wrap;
  overflow-wrap: break-word;
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
.input-area {
  padding: 8px 16px;
  border-top: 1px solid var(--app-border-color, #d7d7d7);
}
.input-row {
  display: flex;
  gap: 8px;
  align-items: flex-end;
}
.message-input {
  flex: 1;
  min-width: 0;
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
