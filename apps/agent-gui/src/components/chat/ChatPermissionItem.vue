<script setup lang="ts">
/**
 * ChatPermissionItem — thin adapter from the chat-stream prop shape
 * to PermissionPrompt's TraceEntryData input.
 *
 * Wraps the existing PermissionPrompt so that tool / memory permission
 * prompts can be rendered inline inside the unified chat-stream feed
 * (Claude Code / Codex style) without duplicating allow/deny logic.
 * The local invoke + MCP trust flow live entirely in PermissionPrompt;
 * we only translate prop shapes here.
 *
 * The ChatPermissionStreamItem interface is intentionally re-declared
 * inline rather than imported from `@/types/chatStream`, keeping this
 * leaf component decoupled from the chat-stream value-shape (which is
 * still evolving across the v0.30.0 campaign).
 */
import PermissionPrompt from "@/components/PermissionPrompt.vue";
import type { TraceEntryData } from "@/types/trace";

const props = defineProps<{
  id: string;
  variant: "tool" | "memory";
  toolId?: string;
  title?: string;
  input?: string;
  reason?: string;
  scope?: string;
  content?: string;
  rawEvent?: string;
}>();

const adaptedEntry = computed<TraceEntryData>(() => ({
  id: props.id,
  kind: props.variant === "memory" ? "memory" : "permission",
  status: "pending",
  toolId: props.toolId,
  title: props.title ?? "",
  startedAt: 0,
  input: props.input,
  reason: props.reason,
  scope: props.scope,
  content: props.content,
  rawEvent: props.rawEvent,
  expanded: false
}));
</script>

<template>
  <div class="chat-permission-item" data-test="chat-permission-item" :data-variant="variant">
    <PermissionPrompt :entry="adaptedEntry" />
  </div>
</template>

<style scoped>
.chat-permission-item {
  display: block;
  width: 100%;
  max-width: 100%;
}
</style>
