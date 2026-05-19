<script setup lang="ts">
import { useContextFormatting } from "@/composables/contextFormatting";

const props = defineProps<{
  bySource: [string, number][];
  outputReservation: number;
  displayBudgetTokens: number;
  compacting: boolean;
  compressionRatioTooLow: boolean;
  needsAutoCompression: boolean;
}>();

const emit = defineEmits<{
  compact: [];
}>();

const { t } = useI18n();
const { formatTokens, formatSourceColor, formatSourceLabel, formatSourcePercent } =
  useContextFormatting();
</script>

<template>
  <table class="context-meter-detail-table by-source-table">
    <tbody>
      <tr
        v-for="[source, tokens] in bySource"
        :key="source"
        :data-test="`context-meter-row-${source}`"
      >
        <td>
          <span class="swatch" :style="{ background: formatSourceColor(source) }" />
          {{ formatSourceLabel(source) }}
        </td>
        <td>{{ formatTokens(tokens) }}</td>
        <td>
          {{
            t("context.percentOfBudget", {
              pct: formatSourcePercent(tokens, displayBudgetTokens)
            })
          }}
        </td>
      </tr>
      <tr data-test="context-meter-reserved">
        <td>{{ t("context.reservedForResponse") }}</td>
        <td>{{ formatTokens(outputReservation) }}</td>
        <td></td>
      </tr>
    </tbody>
  </table>
  <div class="context-meter-actions">
    <button
      type="button"
      class="btn btn-primary btn-sm"
      data-test="context-meter-compact"
      :disabled="compacting || compressionRatioTooLow"
      :title="
        compressionRatioTooLow
          ? t('context.notEnoughToCompact')
          : needsAutoCompression
            ? t('context.autoCompressionActive')
            : t('context.compactNow')
      "
      @click="emit('compact')"
    >
      <template v-if="compacting">
        {{ t("context.compactInProgress") }}
      </template>
      <template v-else-if="needsAutoCompression">
        {{ t("context.autoCompressing") }}
      </template>
      <template v-else>
        {{ t("context.compactNow") }}
      </template>
    </button>
  </div>
</template>

<style scoped>
.by-source-table {
  border-top: 1px solid var(--app-border-color);
  padding-top: 6px;
}
.context-meter-detail-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.context-meter-detail-table td {
  padding: 3px 0;
}
.context-meter-detail-table td + td {
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
.context-meter-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
</style>
