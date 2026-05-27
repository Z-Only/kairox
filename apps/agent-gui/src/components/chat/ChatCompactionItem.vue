<script setup lang="ts">
import type { CompactionStatus, CompactionReason } from "@/types";

// Rust `CompactionStatus` has no Completed variant; we accept a local extended discriminator + sibling props so a parent dispatcher can drive every visual state (running/completed/failed/skipped).
type ChatCompactionStatus = CompactionStatus | { type: "Completed" };

const props = defineProps<{
  status: ChatCompactionStatus;
  reason?: CompactionReason;
  ratio?: number;
  durationMs?: number;
  fallbackUsed?: boolean;
}>();

const { t } = useI18n();

const effectiveRatio = computed(() => {
  if (props.status.type === "Skipped" && typeof props.status.ratio === "number") {
    return props.status.ratio;
  }
  return typeof props.ratio === "number" ? props.ratio : null;
});

const ratioLabel = computed(() => {
  const r = effectiveRatio.value;
  if (r === null || !Number.isFinite(r)) return null;
  return `${Math.round(r * 100)}%`;
});

const durationLabel = computed(() => {
  if (typeof props.durationMs !== "number" || !Number.isFinite(props.durationMs)) return null;
  return `${(props.durationMs / 1000).toFixed(1)}s`;
});

const reasonLabel = computed(() => {
  if (!props.reason) return null;
  return props.reason.type === "UserRequested"
    ? t("chatStream.compaction.reason.user")
    : t("chatStream.compaction.reason.threshold");
});

const skippedReasonLabel = computed(() => {
  if (props.status.type !== "Skipped") return null;
  return props.status.reason.type === "AlreadyCompacting"
    ? t("chatStream.compaction.skipped.reason.alreadyCompacting")
    : t("chatStream.compaction.skipped.reason.thresholdDisabled");
});

const dataStatus = computed<"running" | "completed" | "failed" | "skipped" | null>(() => {
  if (props.status.type === "Running") return "running";
  if (props.status.type === "Completed") return "completed";
  if (props.status.type === "Failed") return "failed";
  if (props.status.type === "Skipped") return "skipped";
  return null;
});

const errorMessage = computed(() => (props.status.type === "Failed" ? props.status.error : null));

const rootTestId = computed(() =>
  props.status.type === "Skipped" ? "chat-compaction-skipped" : "chat-compaction-item"
);
</script>

<template>
  <div
    v-if="props.status.type !== 'Idle'"
    class="chat-compaction-item"
    :class="dataStatus ? `chat-compaction-item--${dataStatus}` : undefined"
    :data-test="rootTestId"
    :data-status="dataStatus ?? undefined"
    role="status"
    :aria-label="t('chatStream.compaction.summary')"
  >
    <template v-if="props.status.type === 'Running'">
      <span class="chat-compaction-label">{{ t("chatStream.compaction.running") }}</span>
      <span
        v-if="reasonLabel"
        class="chat-compaction-chip chat-compaction-chip--reason"
        data-test="chat-compaction-reason"
        >{{ reasonLabel }}</span
      >
      <div
        class="chat-compaction-bar chat-compaction-bar--indeterminate"
        role="progressbar"
        aria-busy="true"
        :aria-label="t('chatStream.compaction.running')"
        data-test="chat-compaction-bar"
      />
    </template>

    <template v-else-if="props.status.type === 'Completed'">
      <span class="chat-compaction-label">{{ t("chatStream.compaction.completed") }}</span>
      <span
        v-if="reasonLabel"
        class="chat-compaction-chip chat-compaction-chip--reason"
        data-test="chat-compaction-reason"
        >{{ reasonLabel }}</span
      >
      <span
        v-if="ratioLabel"
        class="chat-compaction-stat chat-compaction-stat--ratio"
        data-test="chat-compaction-ratio"
        >{{ t("chatStream.compaction.ratio", { value: ratioLabel }) }}</span
      >
      <span
        v-if="durationLabel"
        class="chat-compaction-stat chat-compaction-stat--duration"
        data-test="chat-compaction-duration"
        >{{ t("chatStream.compaction.duration", { value: durationLabel }) }}</span
      >
    </template>

    <template v-else-if="props.status.type === 'Failed'">
      <span class="chat-compaction-label">{{ t("chatStream.compaction.failed") }}</span>
      <span v-if="errorMessage" class="chat-compaction-error" data-test="chat-compaction-error">{{
        errorMessage
      }}</span>
      <span
        v-if="props.fallbackUsed"
        class="chat-compaction-chip chat-compaction-chip--fallback"
        data-test="chat-compaction-fallback"
        >{{ t("chatStream.compaction.fallbackUsed") }}</span
      >
    </template>

    <template v-else-if="props.status.type === 'Skipped'">
      <span class="chat-compaction-label">{{ t("chatStream.compaction.skipped.label") }}</span>
      <span
        v-if="skippedReasonLabel"
        class="chat-compaction-chip chat-compaction-chip--reason"
        data-test="chat-compaction-skipped-reason"
        >{{ skippedReasonLabel }}</span
      >
      <span
        v-if="ratioLabel"
        class="chat-compaction-stat chat-compaction-stat--ratio"
        data-test="chat-compaction-ratio"
        >{{ t("chatStream.compaction.ratio", { value: ratioLabel }) }}</span
      >
    </template>
  </div>
</template>

<style scoped>
.chat-compaction-item {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  margin: 6px 0;
  border-left: 2px solid color-mix(in srgb, var(--app-text-color, #1f2937) 25%, transparent);
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 4%, transparent);
  border-radius: 4px;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.chat-compaction-item--running {
  border-left-color: var(--app-primary-color, #1677ff);
}
.chat-compaction-item--completed {
  border-left-color: var(--app-success-color, #52c41a);
}
.chat-compaction-item--failed {
  border-left-color: var(--app-error-color, #d03050);
}
.chat-compaction-item--skipped {
  border-left-color: var(--app-warning-color, #faad14);
}
.chat-compaction-label {
  font-weight: 600;
  opacity: 0.9;
}
.chat-compaction-chip {
  display: inline-flex;
  align-items: center;
  padding: 1px 6px;
  border-radius: 999px;
  font-size: 11px;
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 10%, transparent);
  color: color-mix(in srgb, var(--app-text-color, #1f2937) 80%, transparent);
}
.chat-compaction-chip--fallback {
  background: color-mix(in srgb, var(--app-warning-color, #faad14) 18%, transparent);
  color: var(--app-warning-color, #faad14);
}
.chat-compaction-stat {
  opacity: 0.85;
}
.chat-compaction-error {
  color: var(--app-error-color, #d03050);
  overflow-wrap: anywhere;
}
.chat-compaction-bar {
  position: relative;
  flex: 1 1 120px;
  min-width: 80px;
  height: 4px;
  border-radius: 2px;
  overflow: hidden;
  background: color-mix(in srgb, var(--app-text-color, #1f2937) 10%, transparent);
}
.chat-compaction-bar--indeterminate::after {
  content: "";
  position: absolute;
  inset: 0;
  width: 40%;
  border-radius: 2px;
  background: var(--app-primary-color, #1677ff);
  animation: chat-compaction-bar-slide 1.2s ease-in-out infinite;
}
@keyframes chat-compaction-bar-slide {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(250%);
  }
}
</style>
