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
    <ContextMeter />
    <header class="chat-header">
      <h2>{{ t("chat.header") }}</h2>
      <span class="tag" data-test="chat-profile-badge">
        {{ session.currentProfile }}
      </span>
    </header>

    <div ref="scrollbar" class="message-list" data-test="message-list">
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
        <span
          v-if="session.projection.cancelled"
          class="tag cancelled-marker"
          data-test="cancelled-marker"
        >
          {{ t("chat.cancelled") }}
        </span>
      </div>
    </div>

    <div class="input-area">
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
.tag {
  display: inline-block;
  padding: 0 8px;
  font-size: 12px;
  line-height: 22px;
  border-radius: 3px;
  background: var(--app-tag-color, color-mix(in srgb, var(--app-primary-color) 10%, transparent));
  color: var(--app-text-color);
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
.input-area {
  padding: 8px 16px;
  border-top: 1px solid var(--app-border-color, #d7d7d7);
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
