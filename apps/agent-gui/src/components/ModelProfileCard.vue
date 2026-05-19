<script setup lang="ts">
import type { ProfileSettingsView } from "@/generated/commands";
import SettingsCardItem from "@/components/ui/SettingsCardItem.vue";

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
</script>

<template>
  <SettingsCardItem class="model-settings__profile" :data-test="`model-row-${profile.alias}`">
    <div class="model-settings__profile-main">
      <h3>{{ profile.alias }}</h3>
      <p>{{ profile.provider }} / {{ profile.model_id }}</p>
      <div class="server__tags" aria-label="Profile metadata">
        <span class="tag tag--source" :class="`tag--source-${sourceClass(profile.source)}`">
          {{ sourceLabel(profile.source) }}
        </span>
        <span :class="['tag', profile.enabled ? 'tag-success' : 'tag-warning']">
          {{ profile.enabled ? t("models.enabled") : t("models.disabled") }}
        </span>
        <span v-if="profile.context_window" class="tag">
          {{ t("models.contextWindow") }}: {{ profile.context_window.toLocaleString() }}
        </span>
        <span v-if="profile.output_limit" class="tag">
          {{ t("models.outputLimit") }}: {{ profile.output_limit.toLocaleString() }}
        </span>
        <span v-if="profile.temperature != null" class="tag">
          {{ t("models.temperature") }}: {{ profile.temperature }}
        </span>
      </div>
    </div>

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

.model-settings__profile-main {
  min-width: 0;
  display: grid;
  gap: 8px;
}

.model-settings__profile h3 {
  margin: 0 0 4px;
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

.tag--source {
  font-weight: 600;
}

.tag--source-builtin {
  background: var(--color-muted);
  color: var(--color-text-muted);
}

.tag--source-user {
  background: var(--color-secondary-light);
  color: var(--color-secondary);
}

.tag--source-project {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.tag--source-local {
  background: var(--color-accent-light, var(--color-primary-light));
  color: var(--color-accent, var(--color-primary));
}

.tag--override {
  background: var(--color-warning-light);
  color: var(--color-warning);
}

.tag--disabled-by {
  background: var(--color-danger-light);
  color: var(--color-danger);
}
</style>
