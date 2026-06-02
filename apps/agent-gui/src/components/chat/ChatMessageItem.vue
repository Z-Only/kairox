<script setup lang="ts">
import { renderMarkdown } from "../../utils/markdown";
import type { ProjectedRole } from "../../types";

interface Props {
  role: ProjectedRole;
  content: string;
}

const props = defineProps<Props>();

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
    <span v-else class="message-content">{{ props.content }}</span>
  </div>
</template>

<style scoped>
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
  padding-left: 1.75em;
}

.markdown-body :deep(li) {
  padding-left: 0.15em;
}

.markdown-body :deep(p) {
  margin: 6px 0;
}
</style>
