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
    <span
      v-if="
        props.role === 'assistant' ||
        props.role === 'planner' ||
        props.role === 'worker' ||
        props.role === 'reviewer'
      "
      class="message-content markdown-body"
      :data-test="props.content.startsWith('[error]') ? 'error-banner' : undefined"
      v-html="renderMarkdown(props.content)"
    ></span>
    <!-- eslint-enable vue/no-v-html -->
    <span v-else class="message-content">{{ props.content }}</span>
  </div>
</template>
