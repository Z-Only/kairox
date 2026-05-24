<script setup lang="ts">
// Standalone inline chat-stream row for a single tool/command execution.
// Pattern mirrors TraceEntry.vue, but this component owns its expand/collapse
// state locally (via prop + emit) and does NOT mutate any Pinia store. A
// later PR will mount it from the chat-stream dispatcher.
//
// NOTE: We intentionally define the props interface inline rather than
// importing from `@/types/chatStream` — that file is owned by a sibling
// lane and does not yet exist on this branch.

interface ChatToolCallItemProps {
  toolId: string;
  title?: string;
  status: "running" | "completed" | "failed" | "pending";
  durationMs?: number;
  input?: string;
  outputPreview?: string;
  scope?: string;
  /** Controlled mode: when provided, the parent owns the expanded state. */
  expanded?: boolean;
  /** Uncontrolled initial value; ignored when `expanded` is provided. */
  defaultExpanded?: boolean;
}

const props = withDefaults(defineProps<ChatToolCallItemProps>(), {
  title: undefined,
  durationMs: undefined,
  input: undefined,
  outputPreview: undefined,
  scope: undefined,
  expanded: undefined,
  defaultExpanded: false
});

const emit = defineEmits<{
  (event: "update:expanded", value: boolean): void;
}>();

const { t } = useI18n();

const isControlled = computed(() => props.expanded !== undefined);
const internalExpanded = ref<boolean>(props.defaultExpanded);

const isExpanded = computed<boolean>(() =>
  isControlled.value ? Boolean(props.expanded) : internalExpanded.value
);

function toggle() {
  const next = !isExpanded.value;
  // Always notify parent so controlled callers can react.
  emit("update:expanded", next);
  // In uncontrolled mode we own the state locally; in controlled mode
  // the rendered state is driven entirely by the prop and we must not
  // flip locally (the parent will update the prop, or not).
  if (!isControlled.value) {
    internalExpanded.value = next;
  }
}

const statusIcon: Record<ChatToolCallItemProps["status"], string> = {
  running: "⏳",
  completed: "✅",
  failed: "❌",
  pending: "🔑"
};

const statusLabel = computed(() => t(`chatStream.toolCall.status.${props.status}`));

const durationLabel = computed(() => {
  if (props.durationMs == null) return null;
  return `${(props.durationMs / 1000).toFixed(1)}s`;
});

const toggleLabel = computed(() =>
  isExpanded.value ? t("chatStream.toolCall.collapse") : t("chatStream.toolCall.expand")
);
</script>

<template>
  <div
    :class="['chat-tool-call', `chat-tool-call--${props.status}`]"
    data-test="chat-tool-call-item"
  >
    <div class="chat-tool-call__row" @click="toggle">
      <span
        class="chat-tool-call__status"
        role="img"
        :aria-label="statusLabel"
        :title="statusLabel"
      >
        {{ statusIcon[props.status] }}
      </span>
      <span class="chat-tool-call__tool">
        <span class="chat-tool-call__tool-text" :title="props.toolId">
          {{ props.title || props.toolId }}
        </span>
      </span>
      <KxTag v-if="props.scope" class="chat-tool-call__scope" tone="info" size="sm">
        {{ props.scope }}
      </KxTag>
      <span
        v-if="durationLabel"
        class="chat-tool-call__duration"
        :title="t('chatStream.toolCall.duration')"
      >
        {{ durationLabel }}
      </span>
      <KxBadge v-if="props.status === 'pending'" class="chat-tool-call__badge" tone="warning">
        {{ statusLabel }}
      </KxBadge>
      <KxIconButton
        :label="toggleLabel"
        :title="toggleLabel"
        size="sm"
        variant="ghost"
        data-test="chat-tool-call-toggle"
        @click.stop="toggle"
      >
        <span aria-hidden="true">{{ isExpanded ? "▾" : "▸" }}</span>
      </KxIconButton>
    </div>
    <div v-if="isExpanded" class="chat-tool-call__detail">
      <div v-if="props.input" class="chat-tool-call__section">
        <span class="chat-tool-call__label">{{ t("chatStream.toolCall.input") }}:</span>
        <pre class="chat-tool-code">{{ props.input }}</pre>
      </div>
      <div v-if="props.outputPreview" class="chat-tool-call__section">
        <span class="chat-tool-call__label">{{ t("chatStream.toolCall.output") }}:</span>
        <pre class="chat-tool-code">{{ props.outputPreview }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.chat-tool-call {
  font-size: 12px;
  box-sizing: border-box;
  min-width: 0;
  max-width: 100%;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  overflow: hidden;
}
.chat-tool-call--failed {
  border-color: color-mix(in srgb, var(--app-error-color) 45%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-error-color) 6%, var(--app-card-color));
}
.chat-tool-call--pending {
  background: color-mix(in srgb, var(--app-warning-color) 8%, transparent);
}
.chat-tool-call__row {
  display: flex;
  min-width: 0;
  max-width: 100%;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  cursor: pointer;
}
.chat-tool-call__row:hover {
  background: var(--app-hover-color);
}
.chat-tool-call__status {
  font-size: 12px;
  line-height: 1;
}
.chat-tool-call__tool {
  flex: 1;
  min-width: 0;
  font-weight: 500;
  overflow: hidden;
}
.chat-tool-call__tool-text {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}
.chat-tool-call__scope {
  font-size: 10px;
}
.chat-tool-call__duration {
  color: var(--app-text-color-3);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.chat-tool-call__badge {
  font-size: 10px;
}
.chat-tool-call__detail {
  padding: 6px 8px 8px;
  border-top: 1px solid var(--app-border-color);
  background: var(--app-card-color);
}
.chat-tool-call__section + .chat-tool-call__section {
  margin-top: 6px;
}
.chat-tool-call__label {
  font-weight: 600;
  font-size: 11px;
  color: var(--app-text-color-2);
}
.chat-tool-code {
  margin: 2px 0 0;
  padding: 6px 8px;
  background: var(--app-code-bg);
  color: var(--app-text-color);
  border-radius: 4px;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
