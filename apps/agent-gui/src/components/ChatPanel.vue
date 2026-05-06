<script setup lang="ts">
import { ref, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useAgentsStore } from "@/stores/agents";
import { useUiStore } from "@/stores/ui";
import { renderMarkdown } from "../utils/markdown";
import type { ProjectedRole } from "../types";

const session = useSessionStore();
const agents = useAgentsStore();
const ui = useUiStore();
const inputText = ref("");
const messageList = ref<HTMLElement | null>(null);

/** Map role to display label. */
const roleDisplay: Record<ProjectedRole, string> = {
  user: "You",
  assistant: "Agent",
  planner: "Planner",
  worker: "Worker",
  reviewer: "Reviewer",
  system: "System"
};

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
  const base = roleDisplay[msg.role] || "Agent";
  if (msg.sourceAgentId && msg.role !== "user" && msg.role !== "system") {
    const label = agents.agentLabel(msg.sourceAgentId);
    if (label) return `${base} (${label})`;
  }
  return base;
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
    ui.pushNotification("error", `Failed to send message: ${e}`);
  }
}

async function cancelSession() {
  try {
    await invoke("cancel_session");
  } catch (e) {
    console.error("Failed to cancel session:", e);
    ui.pushNotification("error", `Cancel failed: ${e}`);
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
    if (messageList.value) {
      messageList.value.scrollTop = messageList.value.scrollHeight;
    }
  }
);
</script>

<template>
  <section class="chat-panel">
    <header class="chat-header">
      <h2>Chat</h2>
      <span class="profile-badge">{{ session.currentProfile }}</span>
    </header>
    <div ref="messageList" class="message-list">
      <div
        v-for="(msg, i) in session.projection.messages"
        :key="i"
        :class="['message', `message-${roleClass[msg.role] || 'assistant'}`]"
      >
        <span
          :class="[
            'message-role',
            `role-badge-${roleClass[msg.role] || 'assistant'}`
          ]"
          >{{ messageLabel(msg) }}</span
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
          v-html="renderMarkdown(msg.content)"
        ></span>
        <!-- eslint-enable vue/no-v-html -->
        <span v-else class="message-content">{{ msg.content }}</span>
      </div>
      <div
        v-if="session.projection.token_stream"
        class="message message-assistant streaming"
      >
        <span class="message-role">Agent</span>
        <span class="message-content"
          >{{ session.projection.token_stream
          }}<span class="cursor">▌</span></span
        >
      </div>
      <div v-if="session.projection.cancelled" class="cancelled-marker">
        [cancelled]
      </div>
    </div>
    <div class="input-area">
      <textarea
        v-model="inputText"
        :disabled="session.isStreaming"
        class="message-input"
        placeholder="Type your message..."
        rows="1"
        @keydown="handleKeydown"
      ></textarea>
      <button
        v-if="session.isStreaming"
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
  border-bottom: 1px solid #d7d7d7;
}
.chat-header h2 {
  margin: 0;
  font-size: 14px;
}
.profile-badge {
  font-size: 11px;
  padding: 2px 8px;
  background: #e8e8e8;
  border-radius: 4px;
  color: #555;
}
.message-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 16px;
}
.message {
  margin-bottom: 12px;
  line-height: 1.5;
}
.message-user .message-role {
  color: #0077cc;
  font-weight: 600;
}
.message-assistant .message-role {
  color: #22a06b;
  font-weight: 600;
}
.message-planner .message-role {
  color: #0077cc;
  font-weight: 600;
}
.message-worker .message-role {
  color: #22a06b;
  font-weight: 600;
}
.message-reviewer .message-role {
  color: #7c3aed;
  font-weight: 600;
}
.message-system .message-role {
  color: #888;
  font-weight: 600;
  font-style: italic;
}
.message-system .message-content {
  color: #888;
  font-style: italic;
}
.role-badge-planner {
  background: none;
}
.role-badge-worker {
  background: none;
}
.role-badge-reviewer {
  background: none;
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
  color: #b45309;
  font-style: italic;
  margin-top: 4px;
}
@keyframes blink {
  50% {
    opacity: 0;
  }
}
.input-area {
  display: flex;
  gap: 8px;
  padding: 8px 16px;
  border-top: 1px solid #d7d7d7;
}
.message-input {
  flex: 1;
  padding: 8px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  font-family: inherit;
  font-size: 13px;
  resize: none;
}
.message-input:disabled {
  background: #f5f5f5;
}
.send-button {
  padding: 8px 16px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.send-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.cancel-button {
  padding: 8px 16px;
  background: #cc3333;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.cancel-button:hover {
  background: #b32828;
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
  background: #f0f0f0;
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
