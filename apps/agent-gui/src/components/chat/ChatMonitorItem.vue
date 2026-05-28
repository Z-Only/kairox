<script setup lang="ts">
const props = defineProps<{
  monitorId: string;
  description: string;
  status: "running" | "completed" | "failed";
  lastLine?: string;
  command?: string;
  stopReason?: string;
}>();

const { t } = useI18n();

const expanded = ref(false);

function toggle() {
  expanded.value = !expanded.value;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    toggle();
  }
}

const statusIcon = computed(() => {
  switch (props.status) {
    case "running":
      return "⏳";
    case "completed":
      return "✅";
    case "failed":
      return "❌";
  }
});

const hasDetails = computed(() => props.command || props.lastLine || props.stopReason);
</script>

<template>
  <div
    class="chat-monitor-item"
    :class="`chat-monitor-item--${status}`"
    :data-test="`chat-monitor-item-${monitorId}`"
    :data-status="status"
    role="status"
    :aria-label="t('chatStream.monitor.label', { description })"
  >
    <div
      class="chat-monitor-header"
      :role="hasDetails ? 'button' : undefined"
      :tabindex="hasDetails ? 0 : undefined"
      :aria-expanded="hasDetails ? expanded : undefined"
      :aria-controls="hasDetails ? `chat-monitor-details-${monitorId}` : undefined"
      data-test="chat-monitor-header"
      @click="hasDetails && toggle()"
      @keydown="hasDetails && onKeydown($event)"
    >
      <span class="chat-monitor-icon" aria-hidden="true">{{ statusIcon }}</span>
      <span class="chat-monitor-description" data-test="chat-monitor-description">{{
        description
      }}</span>
      <span class="chat-monitor-status" data-test="chat-monitor-status">{{
        t(`chatStream.monitor.status.${status}`)
      }}</span>
      <span
        v-if="hasDetails"
        class="chat-monitor-chevron"
        :class="{ 'chat-monitor-chevron--open': expanded }"
        aria-hidden="true"
        >▸</span
      >
    </div>

    <div
      v-if="expanded && hasDetails"
      :id="`chat-monitor-details-${monitorId}`"
      class="chat-monitor-details"
      data-test="chat-monitor-details"
    >
      <div v-if="command" class="chat-monitor-detail" data-test="chat-monitor-command">
        <span class="chat-monitor-detail-label">{{ t("chatStream.monitor.command") }}</span>
        <code class="chat-monitor-detail-value">{{ command }}</code>
      </div>
      <div v-if="lastLine" class="chat-monitor-detail" data-test="chat-monitor-last-line">
        <span class="chat-monitor-detail-label">{{ t("chatStream.monitor.lastLine") }}</span>
        <code class="chat-monitor-detail-value">{{ lastLine }}</code>
      </div>
      <div v-if="stopReason" class="chat-monitor-detail" data-test="chat-monitor-stop-reason">
        <span class="chat-monitor-detail-label">{{ t("chatStream.monitor.stopReason") }}</span>
        <span class="chat-monitor-detail-value">{{ stopReason }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.chat-monitor-item {
  margin: 6px 0;
  border-left: 2px solid color-mix(in srgb, var(--app-text-color, #1f2937) 25%, transparent);
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 4%, transparent);
  border-radius: 4px;
  font-size: 12px;
}
.chat-monitor-item--running {
  border-left-color: var(--app-primary-color, #1677ff);
}
.chat-monitor-item--completed {
  border-left-color: var(--app-success-color, #52c41a);
}
.chat-monitor-item--failed {
  border-left-color: var(--app-error-color, #d03050);
}
.chat-monitor-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: default;
  user-select: none;
}
.chat-monitor-header[role="button"] {
  cursor: pointer;
}
.chat-monitor-header[role="button"]:hover {
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 6%, transparent);
}
.chat-monitor-icon {
  flex-shrink: 0;
  font-size: 13px;
  line-height: 1;
}
.chat-monitor-description {
  flex: 1;
  font-weight: 600;
  opacity: 0.9;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chat-monitor-status {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: 999px;
  font-size: 11px;
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 10%, transparent);
  color: color-mix(in srgb, var(--app-text-color, #1f2937) 80%, transparent);
}
.chat-monitor-chevron {
  flex-shrink: 0;
  font-size: 11px;
  opacity: 0.5;
  transition: transform 0.15s ease;
}
.chat-monitor-chevron--open {
  transform: rotate(90deg);
}
.chat-monitor-details {
  padding: 4px 12px 8px 32px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.chat-monitor-detail {
  display: flex;
  gap: 8px;
  align-items: baseline;
}
.chat-monitor-detail-label {
  flex-shrink: 0;
  font-weight: 500;
  opacity: 0.65;
  font-size: 11px;
  min-width: 60px;
}
.chat-monitor-detail-value {
  font-size: 11px;
  opacity: 0.85;
  overflow-wrap: anywhere;
}
code.chat-monitor-detail-value {
  font-family: var(--app-font-mono, monospace);
  padding: 1px 4px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 6%, transparent);
}
</style>
