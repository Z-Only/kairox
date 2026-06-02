<script setup lang="ts">
import type { ProfileSettingsView } from "@/generated/commands";
import SettingsCardItem from "@/components/ui/SettingsCardItem.vue";
import SettingsItemSummary from "@/components/ui/SettingsItemSummary.vue";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";

type SourceTone = "source-builtin" | "source-user" | "source-project" | "source-local";

const props = defineProps<{
  profile: ProfileSettingsView;
  index: number;
  total: number;
  busyAlias?: string | null;
}>();

const emit = defineEmits<{
  move: [alias: string, direction: -1 | 1];
  edit: [profile: ProfileSettingsView];
  toggle: [profile: ProfileSettingsView];
  test: [profile: ProfileSettingsView];
  remove: [alias: string];
}>();

const { t } = useI18n();

function sourceClass(source: string): string {
  switch (source) {
    case "defaults":
      return "builtin";
    case "profiles_toml":
    case "user_config":
      return "user";
    case "project_config":
      return "project";
    default:
      return source.toLowerCase().replace(/[^a-z0-9-]/g, "-");
  }
}

function sourceTone(source: string): SourceTone {
  switch (sourceClass(source)) {
    case "project":
      return "source-project";
    case "local":
      return "source-local";
    case "builtin":
      return "source-builtin";
    default:
      return "source-user";
  }
}

function sourceLabel(source: string): string {
  switch (source) {
    case "defaults":
      return t("models.sourceDefaults");
    case "profiles_toml":
      return t("models.sourceProfilesToml");
    case "user_config":
      return t("models.sourceUserConfig");
    case "project_config":
      return t("models.sourceProjectConfig");
    default:
      return source;
  }
}

function isClaudeCodeIdentity(value: string | null | undefined): boolean {
  return value?.trim().toLowerCase().replaceAll("-", "_") === "claude_code";
}
</script>

<template>
  <SettingsCardItem class="model-settings__profile" :data-test="`model-row-${profile.alias}`">
    <SettingsItemSummary
      :title="profile.alias"
      :description="`${profile.provider} / ${profile.model_id}`"
      :tags-label="t('models.title')"
    >
      <template #tags>
        <SettingsStatusTag :tone="sourceTone(profile.source)">
          {{ sourceLabel(profile.source) }}
        </SettingsStatusTag>
        <SettingsStatusTag :tone="profile.enabled ? 'success' : 'warning'">
          {{ profile.enabled ? t("models.enabled") : t("models.disabled") }}
        </SettingsStatusTag>
        <SettingsStatusTag v-if="profile.context_window">
          {{ t("models.contextWindow") }}: {{ profile.context_window.toLocaleString() }}
        </SettingsStatusTag>
        <SettingsStatusTag v-if="profile.output_limit">
          {{ t("models.outputLimit") }}: {{ profile.output_limit.toLocaleString() }}
        </SettingsStatusTag>
        <SettingsStatusTag v-if="profile.temperature != null">
          {{ t("models.temperature") }}: {{ profile.temperature }}
        </SettingsStatusTag>
        <SettingsStatusTag v-if="isClaudeCodeIdentity(profile.client_identity)">
          {{ t("models.claudeCodeIdentity") }}
        </SettingsStatusTag>
      </template>
    </SettingsItemSummary>

    <template #actions>
      <div class="model-settings__reorder">
        <KxIconButton
          :label="t('models.moveUp')"
          size="sm"
          :disabled="busyAlias === profile.alias || index === 0"
          :data-test="`model-move-up-${profile.alias}`"
          :title="t('models.moveUp')"
          @click="emit('move', profile.alias, -1)"
        >
          ▲
        </KxIconButton>
        <KxIconButton
          :label="t('models.moveDown')"
          size="sm"
          :disabled="busyAlias === profile.alias || index === total - 1"
          :data-test="`model-move-down-${profile.alias}`"
          :title="t('models.moveDown')"
          @click="emit('move', profile.alias, 1)"
        >
          ▼
        </KxIconButton>
      </div>
      <KxInlineAction
        :disabled="busyAlias === profile.alias"
        :data-test="`model-edit-${profile.alias}`"
        @click="emit('edit', profile)"
      >
        {{ t("common.edit") }}
      </KxInlineAction>
      <KxInlineAction
        :disabled="busyAlias === profile.alias"
        :data-test="`model-enable-${profile.alias}`"
        @click="emit('toggle', profile)"
      >
        {{ profile.enabled ? t("models.disable") : t("models.enable") }}
      </KxInlineAction>
      <KxInlineAction
        :disabled="busyAlias === profile.alias"
        :data-test="`model-test-${profile.alias}`"
        :title="t('models.testConnectivity')"
        @click="emit('test', profile)"
      >
        {{ t("models.testConnectivity") }}
      </KxInlineAction>
      <KxInlineAction
        v-if="profile.writable"
        variant="danger"
        :disabled="busyAlias === profile.alias"
        :data-test="`model-delete-${profile.alias}`"
        @click="emit('remove', profile.alias)"
      >
        {{ t("common.delete") }}
      </KxInlineAction>
    </template>
  </SettingsCardItem>
</template>

<style scoped>
.model-settings__profile {
  overflow: visible;
}

.model-settings__reorder {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-right: 4px;
}

button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
