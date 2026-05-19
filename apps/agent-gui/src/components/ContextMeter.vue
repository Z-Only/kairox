<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useToast } from "@/composables/useToast";
import KxPopover from "@/components/ui/KxPopover.vue";
import KxProgressRing from "@/components/ui/KxProgressRing.vue";
import ContextMeterDetails from "@/components/ContextMeterDetails.vue";
import { useContextFormatting } from "@/composables/contextFormatting";

const props = withDefaults(
  defineProps<{
    variant?: "bar" | "ring";
  }>(),
  { variant: "bar" }
);

const { t } = useI18n();
const session = useSessionStore();
const toast = useToast();
const popoverOpen = ref(false);

const ratio = computed(() => {
  const u = session.lastContextUsage;
  const budget = displayBudgetTokens.value;
  if (!u || budget === 0) return 0;
  return Math.min(1, u.total_tokens / budget);
});

const ratioPct = computed(() => Math.round(ratio.value * 100));

const badgeKind = computed<"healthy" | "warn" | "err">(() => {
  if (ratio.value >= 0.85) return "err";
  if (ratio.value >= 0.7) return "warn";
  return "healthy";
});

const progressRingState = computed<"normal" | "warning" | "danger">(() => {
  if (badgeKind.value === "err") return "danger";
  if (badgeKind.value === "warn") return "warning";
  return "normal";
});

const currentModelContextWindow = computed(() => session.modelLimits?.context_window ?? null);

// Display budget tokens — calculates from modelLimits when lastContextUsage is stale
const displayBudgetTokens = computed(() => {
  const usage = session.lastContextUsage;
  const limits = session.modelLimits;
  if (!usage && !limits) return 0;
  if (limits) {
    const safety = Math.max(2000, Math.floor(limits.output_limit / 10));
    return limits.context_window - (limits.output_limit + safety);
  }
  return usage!.budget_tokens;
});

// Display context window — prefers modelLimits when available
const displayContextWindow = computed(() => {
  if (session.modelLimits) return session.modelLimits.context_window;
  return session.lastContextUsage?.context_window ?? 0;
});

// Whether context usage data matches the current model limits
const contextUsageMatchesModel = computed(() => {
  if (!session.modelLimits || !session.lastContextUsage) return true;
  return session.lastContextUsage.context_window === session.modelLimits.context_window;
});

const contextWindowSummary = computed(() => {
  const usageContextWindow = session.lastContextUsage?.context_window;
  const currentModelWindow = currentModelContextWindow.value;

  if (usageContextWindow && currentModelWindow) {
    return `${formatTokens(usageContextWindow)} · ${session.currentProfile}: ${formatTokens(currentModelWindow)}`;
  }
  if (usageContextWindow) return formatTokens(usageContextWindow);
  if (currentModelWindow) return `${session.currentProfile}: ${formatTokens(currentModelWindow)}`;
  return t("context.unavailable");
});

const compactionStateLabel = computed(() => {
  if (session.compacting) return t("context.compactInProgress");
  if (session.lastCompactionError) return t("context.failedFallback");
  return t("context.idle");
});

const hasMessages = computed(() => session.projection.messages.length > 0);

const compressionRatioTooLow = computed(() => ratio.value < 0.3);

const needsAutoCompression = computed(() => ratio.value >= 0.85);

const { formatTokens, formatSourceColor, formatSourcePercent } = useContextFormatting();

function togglePopover() {
  popoverOpen.value = !popoverOpen.value;
}

function onHoverOpen() {
  popoverOpen.value = true;
}

function onHoverClose() {
  popoverOpen.value = false;
}

async function onCompactClick() {
  if (session.compacting) return;
  popoverOpen.value = false;
  try {
    await invoke("compact_session");
  } catch (e) {
    toast.error(t("context.compactionFailed", { error: String(e) }));
  }
}
</script>

<template>
  <div
    v-if="!(variant === 'ring' && !hasMessages)"
    class="context-meter"
    :class="{ 'context-meter-ring-mode': props.variant === 'ring' }"
    data-test="context-meter"
  >
    <KxPopover
      v-if="props.variant === 'ring' && hasMessages"
      v-model:open="popoverOpen"
      content-data-test="context-meter-popover"
      side="top"
      align="end"
      :side-offset="0"
    >
      <template #trigger>
        <button
          type="button"
          class="ring-trigger"
          data-test="context-meter-ring"
          :class="[`ring-trigger--${progressRingState}`]"
          :aria-label="
            session.lastContextUsage ? `${ratioPct}% context used` : t('context.noUsageYet')
          "
          :title="t('context.popoverHeader')"
        >
          <KxProgressRing
            data-test="context-progress-ring"
            :ratio="ratio"
            :label="
              session.lastContextUsage ? `${ratioPct}% context used` : t('context.noUsageYet')
            "
            :state="progressRingState"
          >
            <span v-if="session.lastContextUsage">{{ ratioPct }}%</span>
            <span
              v-else
              data-test="context-meter-ring-empty"
              role="img"
              :aria-label="t('context.noUsageYet')"
              :title="t('context.noUsageYet')"
            />
          </KxProgressRing>
        </button>
      </template>

      <template #content>
        <div class="ring-popover-body" @mouseenter="onHoverOpen" @mouseleave="onHoverClose">
          <header class="popover-header">{{ t("context.popoverHeader") }}</header>
          <div v-if="!session.lastContextUsage" class="fallback-detail">
            {{ t("context.noUsageYet") }}
          </div>
          <template v-else>
            <dl class="detail-grid">
              <div>
                <dt>{{ t("context.usedTokens") }}</dt>
                <dd>{{ formatTokens(session.lastContextUsage.total_tokens) }}</dd>
              </div>
              <div>
                <dt>{{ t("context.maxTokens") }}</dt>
                <dd>
                  <template v-if="contextUsageMatchesModel">{{
                    formatTokens(session.lastContextUsage.budget_tokens)
                  }}</template>
                  <template v-else>
                    <span class="estimated-value" :title="t('context.estimatedBudget')">{{
                      formatTokens(displayBudgetTokens)
                    }}</span>
                  </template>
                </dd>
              </div>
              <div>
                <dt>{{ t("context.percentage") }}</dt>
                <dd>{{ ratioPct }}%</dd>
              </div>
              <div>
                <dt>{{ t("context.contextWindow") }}</dt>
                <dd>{{ formatTokens(displayContextWindow) }}</dd>
              </div>
              <div>
                <dt>{{ t("context.compactionState") }}</dt>
                <dd>{{ compactionStateLabel }}</dd>
              </div>
            </dl>
            <ContextMeterDetails
              :by-source="session.lastContextUsage.by_source"
              :output-reservation="session.lastContextUsage.output_reservation"
              :display-budget-tokens="displayBudgetTokens"
              :compacting="session.compacting"
              :compression-ratio-too-low="compressionRatioTooLow"
              :needs-auto-compression="needsAutoCompression"
              @compact="onCompactClick"
            />
          </template>
        </div>
      </template>
    </KxPopover>

    <div v-else-if="!session.lastContextUsage" class="empty" data-test="context-meter-empty">
      <span class="empty-bar" />
      <span class="empty-label">{{ t("context.noUsageYet") }}</span>
    </div>

    <div v-else class="meter-row">
      <button
        type="button"
        class="bar"
        data-test="context-meter-bar"
        :title="t('context.popoverHeader')"
        @click="togglePopover"
      >
        <span
          v-for="[source, tokens] in session.lastContextUsage.by_source"
          :key="source"
          class="segment"
          :style="{
            width: `${formatSourcePercent(tokens, displayBudgetTokens)}%`,
            background: formatSourceColor(source)
          }"
        />
      </button>

      <span class="numbers" data-test="context-meter-numbers">
        {{ formatTokens(session.lastContextUsage.total_tokens) }} /
        {{ formatTokens(displayBudgetTokens) }}
        ({{ ratioPct }}%)
      </span>

      <span v-if="session.compacting" class="badge badge-busy" data-test="context-meter-badge-busy">
        <span class="dot" />
        {{ t("context.compactInProgress") }}
      </span>
      <span
        v-else-if="badgeKind === 'err'"
        class="badge badge-err"
        data-test="context-meter-badge-err"
      >
        ⚠ {{ t("status.contextNearFull") }}
      </span>
      <span
        v-else-if="badgeKind === 'warn'"
        class="badge badge-warn"
        data-test="context-meter-badge-warn"
      >
        ⚠
      </span>

      <span
        v-if="session.lastCompactionError"
        class="badge badge-warn"
        data-test="context-meter-badge-failed"
        :title="session.lastCompactionError"
      >
        ⚠ {{ t("context.failedFallback") }}
      </span>
    </div>

    <div
      v-if="props.variant === 'bar' && popoverOpen && session.lastContextUsage"
      class="popover"
      data-test="context-meter-popover"
    >
      <header class="popover-header">{{ t("context.popoverHeader") }}</header>
      <ContextMeterDetails
        :by-source="session.lastContextUsage.by_source"
        :output-reservation="session.lastContextUsage.output_reservation"
        :display-budget-tokens="displayBudgetTokens"
        :compacting="session.compacting"
        :compression-ratio-too-low="compressionRatioTooLow"
        :needs-auto-compression="needsAutoCompression"
        @compact="onCompactClick"
      />
    </div>
  </div>
</template>

<style scoped>
.context-meter {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 6px 16px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
  background: var(--app-card-color);
}
.context-meter-ring-mode {
  flex: 0 0 auto;
  padding: 0;
  border-bottom: 0;
  background: transparent;
}
.empty {
  display: flex;
  align-items: center;
  gap: 8px;
}
.empty-bar {
  display: inline-block;
  height: 6px;
  width: 80px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--app-text-color) 10%, transparent);
}
.empty-label {
  font-size: 12px;
  opacity: 0.6;
}
.meter-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.bar {
  flex: 1;
  display: flex;
  height: 6px;
  border-radius: 3px;
  overflow: hidden;
  background: color-mix(in srgb, var(--app-text-color) 8%, transparent);
  border: none;
  padding: 0;
  cursor: pointer;
}
.bar:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
.ring-trigger {
  display: inline-grid;
  width: 40px;
  height: 40px;
  place-items: center;
  border: 0;
  border-radius: 999px;
  padding: 0;
  color: var(--app-text-color, #1f2937);
  background: transparent;
  cursor: pointer;
}
.ring-trigger:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
.segment {
  height: 100%;
  display: block;
}
.numbers {
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  opacity: 0.85;
  white-space: nowrap;
}
.badge {
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 3px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.badge-warn {
  background: color-mix(in srgb, var(--app-warning-color, #faad14) 15%, transparent);
  color: var(--app-warning-color, #faad14);
}
.badge-err {
  background: color-mix(in srgb, var(--app-error-color, #d03050) 15%, transparent);
  color: var(--app-error-color, #d03050);
}
.badge-busy {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  color: var(--app-primary-color);
}
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    opacity: 0.3;
  }
}
.popover {
  position: absolute;
  top: 100%;
  left: 16px;
  right: 16px;
  z-index: var(--app-z-popover);
  margin-top: 4px;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 8px 12px;
  box-shadow: var(--app-overlay-shadow);
}
.context-meter-ring-mode .popover {
  left: auto;
  right: 0;
  width: min(320px, calc(100vw - 32px));
}
.popover-header {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 6px;
}
.ring-popover-body {
  width: min(320px, calc(100vw - 32px));
}
.fallback-detail {
  font-size: 12px;
  opacity: 0.75;
}
.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px 12px;
  margin: 0 0 8px;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.detail-grid div {
  min-width: 0;
}
.detail-grid dt {
  margin: 0;
  color: color-mix(in srgb, var(--app-text-color) 65%, transparent);
}
.detail-grid dd {
  margin: 2px 0 0;
  font-weight: 600;
}
.estimated-value {
  border-bottom: 1px dashed var(--app-text-color);
  cursor: help;
}
</style>
