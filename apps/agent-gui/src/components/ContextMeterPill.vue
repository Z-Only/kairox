<script setup lang="ts">
// `ContextMeterPill` is the demoted, secondary surface for the context
// usage signal. The richer `ContextMeter` (bar/ring + popover) is preserved
// for diagnostic use; the primary in-chat compaction signal now lives
// inline in the chat stream via `ChatCompactionItem` (PRs #471-#477).
// This pill renders a compact `12.4k/180.0k` token summary and opens the
// same `ContextMeterDetails` popover on click so users can still trigger
// manual compaction without dominating the workbench chrome.
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useToast } from "@/composables/useToast";
import KxPopover from "@/components/ui/KxPopover.vue";
import ContextMeterDetails from "@/components/ContextMeterDetails.vue";
import { useContextFormatting } from "@/composables/contextFormatting";

const { t } = useI18n();
const session = useSessionStore();
const toast = useToast();
const popoverOpen = ref(false);
const { formatTokens } = useContextFormatting();

// Mirrors `ContextMeter`'s budget calculation so the pill remains accurate
// before the first `ContextAssembled` event has populated `lastContextUsage`.
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

const ratio = computed(() => {
  const u = session.lastContextUsage;
  const budget = displayBudgetTokens.value;
  if (!u || budget === 0) return 0;
  return Math.min(1, u.total_tokens / budget);
});

const ratioPct = computed(() => Math.round(ratio.value * 100));

const tone = computed<"normal" | "warn" | "err">(() => {
  if (ratio.value >= 0.85) return "err";
  if (ratio.value >= 0.7) return "warn";
  return "normal";
});

const compressionRatioTooLow = computed(() => ratio.value < 0.3);
const needsAutoCompression = computed(() => ratio.value >= 0.85);

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
  <div class="context-meter-pill" data-test="context-meter-pill">
    <KxPopover
      v-model:open="popoverOpen"
      content-data-test="context-meter-popover"
      content-class="context-meter-popover context-meter-popover--pill"
      width="min(360px, calc(100vw - 32px))"
      side="top"
      align="end"
      :side-offset="6"
    >
      <template #trigger>
        <button
          type="button"
          class="pill-trigger"
          :class="`pill-trigger--${tone}`"
          data-test="context-meter-pill-trigger"
          :title="t('context.popoverHeader')"
          :aria-label="
            session.lastContextUsage ? `${ratioPct}% context used` : t('context.noUsageYet')
          "
        >
          <span class="pill-numbers" data-test="context-meter-pill-numbers">
            <template v-if="session.lastContextUsage">
              {{ formatTokens(session.lastContextUsage.total_tokens) }} /
              {{ formatTokens(displayBudgetTokens) }}
            </template>
            <template v-else>
              {{ t("context.noUsageYet") }}
            </template>
          </span>
          <span
            v-if="session.lastContextUsage"
            class="pill-pct"
            data-test="context-meter-pill-pct"
            aria-hidden="true"
          >
            {{ ratioPct }}%
          </span>
        </button>
      </template>

      <template #content>
        <header class="kx-popover-panel__header context-meter-popover-header">
          {{ t("context.popoverHeader") }}
        </header>
        <div v-if="!session.lastContextUsage" class="fallback-detail">
          {{ t("context.noUsageYet") }}
        </div>
        <template v-else>
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
      </template>
    </KxPopover>
  </div>
</template>

<style scoped>
.context-meter-pill {
  pointer-events: auto;
  display: inline-flex;
}
.pill-trigger {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 10px;
  font-size: 11px;
  line-height: 1.5;
  font-variant-numeric: tabular-nums;
  border-radius: 999px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  background: color-mix(in srgb, var(--app-text-color) 6%, transparent);
  color: var(--app-text-color);
  cursor: pointer;
  opacity: 0.78;
  white-space: nowrap;
}
@media (prefers-reduced-motion: no-preference) {
  .pill-trigger {
    transition:
      opacity 120ms ease,
      color 120ms ease,
      background 120ms ease,
      border-color 120ms ease;
  }
}
.pill-trigger:hover,
.pill-trigger:focus-visible {
  opacity: 1;
  outline: none;
  border-color: var(--app-primary-color);
  color: var(--app-primary-color);
}
.pill-trigger--warn {
  background: color-mix(in srgb, var(--app-warning-color, #faad14) 14%, transparent);
  color: var(--app-warning-color, #faad14);
  border-color: color-mix(in srgb, var(--app-warning-color, #faad14) 32%, transparent);
  opacity: 1;
}
.pill-trigger--err {
  background: color-mix(in srgb, var(--app-error-color, #d03050) 14%, transparent);
  color: var(--app-error-color, #d03050);
  border-color: color-mix(in srgb, var(--app-error-color, #d03050) 32%, transparent);
  opacity: 1;
}
.pill-numbers {
  white-space: nowrap;
}
.pill-pct {
  opacity: 0.7;
  font-size: 10px;
}
.fallback-detail {
  font-size: 12px;
  opacity: 0.75;
}
</style>
