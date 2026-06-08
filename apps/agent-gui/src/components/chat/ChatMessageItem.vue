<script setup lang="ts">
import { renderMarkdown } from "../../utils/markdown";
import type { ProjectedRole } from "../../types";

interface Props {
  role: ProjectedRole;
  content: string;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  copy: [content: string];
  edit: [content: string];
}>();

const copiedRecently = ref(false);

async function handleCopy() {
  try {
    await navigator.clipboard.writeText(props.content);
  } catch {
    /* parent handles toast via emit */
  }
  emit("copy", props.content);
  copiedRecently.value = true;
  setTimeout(() => {
    copiedRecently.value = false;
  }, 1500);
}

function handleEdit() {
  emit("edit", props.content);
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
</script>

<template>
  <div
    :class="['message', `message-${roleClass[props.role] || 'assistant'}`]"
    data-test="chat-message"
    :data-role="roleClass[props.role] || 'assistant'"
    :data-error="props.content.startsWith('[error]') ? 'true' : undefined"
  >
    <!-- eslint-disable vue/no-v-html -->
    <div
      v-if="
        props.role === 'assistant' ||
        props.role === 'planner' ||
        props.role === 'worker' ||
        props.role === 'reviewer'
      "
      class="message-content markdown-body"
      :data-test="props.content.startsWith('[error]') ? 'error-banner' : undefined"
      v-html="renderMarkdown(props.content)"
    ></div>
    <!-- eslint-enable vue/no-v-html -->
    <template v-else>
      <div class="user-message-wrapper">
        <div class="user-message-actions">
          <button
            class="action-btn"
            :title="copiedRecently ? 'Copied!' : 'Copy'"
            @click="handleCopy"
          >
            <svg
              v-if="copiedRecently"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
            <svg
              v-else
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
          </button>
          <button class="action-btn" title="Edit" @click="handleEdit">
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M17 3a2.83 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
              <path d="m15 5 4 4" />
            </svg>
          </button>
        </div>
        <span class="message-content">{{ props.content }}</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
/* ── User message action buttons ── */
.user-message-wrapper {
  display: flex;
  align-items: center;
  gap: 4px;
  max-width: min(760px, 100%);
}

.user-message-actions {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.15s ease;
  pointer-events: none;
}

.message-user:hover .user-message-actions {
  opacity: 1;
  pointer-events: auto;
}

.action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--app-text-secondary, #888);
  cursor: pointer;
  transition:
    background 0.15s ease,
    color 0.15s ease;
}

.action-btn:hover {
  background: var(--app-hover-color, rgba(128, 128, 128, 0.12));
  color: var(--app-text-primary, #333);
}

.action-btn:active {
  transform: scale(0.92);
}

/* ── Markdown styles ── */
.markdown-body :deep(pre.hljs) {
  margin: 8px 0;
  border-radius: var(--app-radius-md);
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
  margin: 6px 0;
  padding-left: 0;
  list-style-position: inside;
}

.markdown-body :deep(li) {
  padding-left: 0.15em;
}

.markdown-body :deep(p) {
  margin: 0;
}

.markdown-body :deep(p + p) {
  margin-top: 6px;
}
</style>
