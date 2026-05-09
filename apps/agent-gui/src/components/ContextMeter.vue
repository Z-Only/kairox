<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useToast } from "@/composables/useToast";
import type { ContextSource } from "@/types";

const { t } = useI18n();
const session = useSessionStore();
const toast = useToast();
const popoverOpen = ref(false);

const ratio = computed(() => {
  const u = session.lastContextUsage;
  if (!u || u.budget_tokens === 0) return 0;
  return Math.min(1, u.total_tokens / u.budget_tokens);
});

const ratioPct = computed(() => Math.round(ratio.value * 100));

const badgeKind = computed<"healthy" | "warn" | "err">(() => {
  if (ratio.value >= 0.85) return "err";
  if (ratio.value >= 0.7) return "warn";
  return "healthy";
});

// `ContextSource` is `#[serde(rename_all = "snake_case")]` on the Rust side
// (see crates/agent-core/src/context_types.rs:5), so by_source tuples carry
// snake_case strings — these maps must use the same casing.
const sourceColorVar: Record<ContextSource, string> = {
  system: "var(--src-system)",
  tool_definitions: "var(--src-tools)",
  memory: "var(--src-memory)",
  history: "var(--src-history)",
  tool_result: "var(--src-tool-result)",
  selected_file: "var(--src-selected-file)",
  compaction_summary: "var(--src-compaction-summary)",
  request: "var(--src-request)"
};

const sourceLabel: Record<ContextSource, string> = {
  system: "context.sourceSystem",
  tool_definitions: "context.sourceTools",
  memory: "context.sourceMemory",
  history: "context.sourceHistory",
  tool_result: "context.sourceToolResult",
  selected_file: "context.sourceSelectedFile",
  compaction_summary: "context.sourceCompactionSummary",
  request: "context.sourceRequest"
};

function formatTokens(n: number): string {
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function togglePopover() {
  if (!session.lastContextUsage) return;
  popoverOpen.value = !popoverOpen.value;
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
  <div class="context-meter" data-test="context-meter">
    <div v-if="!session.lastContextUsage" class="empty" data-test="context-meter-empty">
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
            width: `${(tokens / session.lastContextUsage.budget_tokens) * 100}%`,
            background: sourceColorVar[source]
          }"
        />
      </button>

      <span class="numbers" data-test="context-meter-numbers">
        {{ formatTokens(session.lastContextUsage.total_tokens) }} /
        {{ formatTokens(session.lastContextUsage.budget_tokens) }}
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
      v-if="popoverOpen && session.lastContextUsage"
      class="popover"
      data-test="context-meter-popover"
    >
      <header class="popover-header">{{ t("context.popoverHeader") }}</header>
      <table class="popover-table">
        <tbody>
          <tr
            v-for="[source, tokens] in session.lastContextUsage.by_source"
            :key="source"
            :data-test="`context-meter-row-${source}`"
          >
            <td>
              <span class="swatch" :style="{ background: sourceColorVar[source] }" />
              {{ t(sourceLabel[source]) }}
            </td>
            <td>{{ formatTokens(tokens) }}</td>
            <td>
              {{
                t("context.percentOfBudget", {
                  pct: Math.round((tokens / session.lastContextUsage.budget_tokens) * 100)
                })
              }}
            </td>
          </tr>
          <tr data-test="context-meter-reserved">
            <td>{{ t("context.reservedForResponse") }}</td>
            <td>{{ formatTokens(session.lastContextUsage.output_reservation) }}</td>
            <td></td>
          </tr>
        </tbody>
      </table>
      <div class="popover-actions">
        <button
          type="button"
          class="btn btn-ghost"
          data-test="context-meter-switch-model"
          disabled
          :title="t('context.switchModel')"
        >
          {{ t("context.switchModel") }}
        </button>
        <button
          type="button"
          class="btn btn-primary"
          data-test="context-meter-compact"
          :disabled="session.compacting"
          @click="onCompactClick"
        >
          {{ session.compacting ? t("context.compactInProgress") : t("context.compactNow") }}
        </button>
      </div>
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
  z-index: 20;
  margin-top: 4px;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 8px 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
}
.popover-header {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 6px;
}
.popover-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.popover-table td {
  padding: 3px 0;
}
.popover-table td + td {
  text-align: right;
}
.swatch {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 2px;
  margin-right: 6px;
  vertical-align: middle;
}
.popover-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
.btn {
  padding: 4px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-primary {
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-primary-color);
}
.btn-ghost {
  background: transparent;
}
</style>
