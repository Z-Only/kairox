<script setup lang="ts">
import SettingsStatusTag from "./SettingsStatusTag.vue";

type AuditTone =
  | "neutral"
  | "success"
  | "warning"
  | "error"
  | "info"
  | "muted"
  | "override"
  | "disabled-by"
  | "source-builtin"
  | "source-user"
  | "source-project"
  | "source-local";

interface AuditItem {
  key: string;
  label: string;
  value: string;
  tone: AuditTone;
}

const props = withDefaults(
  defineProps<{
    source?: string | null;
    sourceTone?: AuditTone;
    enabled?: boolean | null;
    effective?: boolean | null;
    shadowedBy?: string | null;
    overrides?: string | null;
    disabledBy?: string | null;
    valid?: boolean | null;
    dataTest?: string;
  }>(),
  {
    source: null,
    sourceTone: "neutral",
    enabled: null,
    effective: null,
    shadowedBy: null,
    overrides: null,
    disabledBy: null,
    valid: null,
    dataTest: undefined
  }
);

const { t } = useI18n();

const items = computed<AuditItem[]>(() => {
  const next: AuditItem[] = [];

  if (props.source) {
    next.push({
      key: "source",
      label: t("settings.auditSource"),
      value: props.source,
      tone: props.sourceTone
    });
  }

  if (props.enabled !== null) {
    next.push({
      key: "state",
      label: t("settings.auditState"),
      value: props.enabled ? t("settings.auditEnabled") : t("settings.auditDisabled"),
      tone: props.enabled ? "success" : "warning"
    });
  }

  if (props.effective !== null) {
    const shadowedBy = props.shadowedBy ?? null;
    next.push({
      key: "effective",
      label: t("settings.auditEffective"),
      value: props.effective
        ? t("settings.auditActive")
        : shadowedBy
          ? t("settings.auditShadowedBy", { source: shadowedBy })
          : t("settings.auditInactive"),
      tone: props.effective ? "success" : "warning"
    });
  }

  if (props.overrides) {
    next.push({
      key: "overrides",
      label: t("settings.auditOverrides"),
      value: props.overrides,
      tone: "override"
    });
  }

  if (props.disabledBy) {
    next.push({
      key: "disabled-by",
      label: t("settings.auditDisabledBy"),
      value: props.disabledBy,
      tone: "disabled-by"
    });
  }

  if (props.valid !== null) {
    next.push({
      key: "validity",
      label: t("settings.auditValidity"),
      value: props.valid ? t("settings.auditValid") : t("settings.auditInvalid"),
      tone: props.valid ? "success" : "error"
    });
  }

  return next;
});

function itemTestId(key: string): string | undefined {
  return props.dataTest ? `${props.dataTest}-${key}` : undefined;
}
</script>

<template>
  <dl
    v-if="items.length"
    class="settings-effective-audit"
    :aria-label="t('settings.auditAria')"
    :data-test="props.dataTest"
  >
    <div v-for="item in items" :key="item.key" class="settings-effective-audit__item">
      <dt>{{ item.label }}</dt>
      <dd>
        <SettingsStatusTag :tone="item.tone" :data-test="itemTestId(item.key)">
          {{ item.value }}
        </SettingsStatusTag>
      </dd>
    </div>
  </dl>
</template>

<style scoped>
.settings-effective-audit {
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px 10px;
  margin: 0;
}

.settings-effective-audit__item {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.settings-effective-audit dt {
  color: var(--app-text-color-2);
  font-size: 12px;
  font-weight: 600;
}

.settings-effective-audit dd {
  min-width: 0;
  margin: 0;
}
</style>
