<script setup lang="ts">
import { ref, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { sessionState, reportSendError } from "../stores/session";

const inputText = ref("");
const messageList = ref<HTMLElement | null>(null);

async function sendMessage() {
  const content = inputText.value.trim();
  if (!content || sessionState.isStreaming) return;

  inputText.value = "";
  try {
    await invoke("send_message", { content });
  } catch (e) {
    console.error("Failed to send message:", e);
    reportSendError(String(e));
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

watch(
  () => [
    sessionState.projection.messages.length,
    sessionState.projection.token_stream
  ],
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
      <span class="profile-badge">{{ sessionState.currentProfile }}</span>
    </header>
    <div ref="messageList" class="message-list">
      <div
        v-for="(msg, i) in sessionState.projection.messages"
        :key="i"
        :class="[
          'message',
          msg.role === 'user' ? 'message-user' : 'message-assistant'
        ]"
      >
        <span class="message-role">{{
          msg.role === "user" ? "You" : "Agent"
        }}</span>
        <span class="message-content">{{ msg.content }}</span>
      </div>
      <div
        v-if="sessionState.projection.token_stream"
        class="message message-assistant streaming"
      >
        <span class="message-role">Agent</span>
        <span class="message-content"
          >{{ sessionState.projection.token_stream
          }}<span class="cursor">▌</span></span
        >
      </div>
      <div v-if="sessionState.projection.cancelled" class="cancelled-marker">
        [cancelled]
      </div>
    </div>
    <div class="input-area">
      <textarea
        v-model="inputText"
        :disabled="sessionState.isStreaming"
        class="message-input"
        placeholder="Type your message..."
        rows="1"
        @keydown="handleKeydown"
      ></textarea>
      <button
        class="send-button"
        :disabled="!inputText.trim() || sessionState.isStreaming"
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
  background: #a0c4e8;
  cursor: not-allowed;
}
</style>
