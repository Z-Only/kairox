<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { ConfigScope, EffectiveSkillView, SkillInstallTarget } from "@/generated/commands";
import { commands } from "@/generated/commands";
import SkillDiscoverList from "@/components/skills/SkillDiscoverList.vue";
import SettingsCardItem from "@/components/ui/SettingsCardItem.vue";
import SettingsCardList from "@/components/ui/SettingsCardList.vue";

const { t } = useI18n();
const skillsStore = useSkillsStore();
const activeSubTab = ref<"installed" | "discover">("installed");
const githubSource = ref("");
const installTarget = ref<ConfigScope>("User");
const busySkillId = ref<string | null>(null);
const skillCatalogInstallTarget = computed<SkillInstallTarget>(
  () => installTarget.value.toLowerCase() as SkillInstallTarget
);

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

// Sync installs with the SettingsLayout ConfigSourceBar selection.
watch(
  () => configSource?.value,
  (src) => {
    if (src) installTarget.value = (src === "user" ? "User" : "Project") as ConfigScope;
  },
  { immediate: true }
);

function formatUpdateState(updateState: string): string {
  return updateState.replaceAll("_", " ");
}

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function canUpdateSkill(skill: EffectiveSkillView): boolean {
  return (
    skill.value.editable &&
    skill.value.install_source !== "builtin" &&
    skill.value.update_state === "update_available"
  );
}

function skillSettingsTestId(skill: EffectiveSkillView): string {
  return slugify(skill.value.settings_id);
}

watch(
  [() => configSource?.value, () => configProjectId?.value],
  async () => {
    await Promise.all([skillsStore.loadSkillSettings(), skillsStore.fetchEffectiveSkills()]);
  },
  { immediate: true }
);

async function runSkillAction(skillId: string, action: () => Promise<unknown>): Promise<void> {
  busySkillId.value = skillId;
  try {
    await action();
    await Promise.all([skillsStore.loadSkillSettings(), skillsStore.fetchEffectiveSkills()]);
  } finally {
    busySkillId.value = null;
  }
}

async function openSkillsDir(): Promise<void> {
  try {
    await commands.openSkillsDir();
  } catch {
    // best-effort
  }
}

async function installFromGithub(): Promise<void> {
  const trimmedSource = githubSource.value.trim();
  if (!trimmedSource) {
    return;
  }

  const target: SkillInstallTarget = installTarget.value.toLowerCase() as SkillInstallTarget;
  const installedSkill = await skillsStore.installGithubSkill(trimmedSource, target);
  if (installedSkill) {
    githubSource.value = "";
  }
}
</script>

<template>
  <section class="skill-settings" aria-label="Skills settings" data-test="skill-settings-pane">
    <SettingsState v-if="skillsStore.error" tone="error" data-test="skill-page-error">
      {{ skillsStore.error }}
    </SettingsState>

    <SettingsSubtabs aria-label="Skill sections">
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'installed'"
        data-test="skill-subtab-installed"
        @click="activeSubTab = 'installed'"
      >
        {{ t("skills.tabInstalled") }}
      </button>
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'discover'"
        data-test="skill-subtab-discover"
        @click="activeSubTab = 'discover'"
      >
        {{ t("skills.tabDiscover") }}
      </button>
    </SettingsSubtabs>

    <div v-if="activeSubTab === 'installed'" class="skill-settings__installed">
      <SettingsToolbar :aria-label="t('skills.tabInstalled')">
        <button
          class="btn btn-sm"
          type="button"
          data-test="skill-open-config-dir"
          :title="t('settings.openConfigDir')"
          @click="openSkillsDir()"
        >
          {{ t("settings.openConfigDir") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="skillsStore.settingsLoading"
          data-test="skill-refresh"
          @click="skillsStore.loadSkillSettings()"
        >
          {{ skillsStore.settingsLoading ? t("skills.refreshing") : t("skills.refreshSkills") }}
        </button>
      </SettingsToolbar>

      <div class="skill-settings__body">
        <SettingsState
          v-if="skillsStore.settingsLoading"
          tone="loading"
          data-test="skill-loading-state"
        >
          {{ t("skills.loading") }}
        </SettingsState>
        <SettingsState
          v-else-if="skillsStore.effectiveSkills.length === 0"
          tone="empty"
          data-test="skill-empty-state"
        >
          {{ t("skills.noSkills") }}
        </SettingsState>

        <SettingsCardList
          v-else
          :aria-label="t('skills.tabInstalled')"
          data-test="skill-installed-list"
        >
          <SettingsCardItem
            v-for="skill in skillsStore.effectiveSkills"
            :key="skill.value.settings_id"
            :data-test="`skill-row-${skillSettingsTestId(skill)}`"
          >
            <div class="skill-settings__main">
              <div class="skill-settings__title-row">
                <h4>{{ skill.value.name }}</h4>
                <span class="tag tag--source" :class="`tag--source-${skill.source.toLowerCase()}`">
                  {{ skill.source }}
                </span>
                <span v-if="skill.overrides" class="tag tag--override">
                  {{ t("skills.overrides", { source: skill.overrides }) }}
                </span>
                <span v-if="skill.disabledBy" class="tag tag--disabled-by">
                  {{ t("skills.disabledBy", { source: skill.disabledBy }) }}
                </span>
                <span class="tag">{{ skill.value.scope }}</span>
                <span :class="['tag', skill.enabled ? 'tag-success' : 'tag-warning']">
                  {{ skill.enabled ? t("skills.enabled") : t("skills.disabled") }}
                </span>
                <span :class="['tag', skill.value.effective ? 'tag-success' : 'tag-warning']">
                  {{
                    skill.value.effective
                      ? t("skills.effective")
                      : t("skills.shadowedBy", { name: skill.value.shadowed_by })
                  }}
                </span>
                <span :class="['tag', skill.value.valid ? 'tag-success' : 'tag-error']">
                  {{ skill.value.valid ? t("skills.valid") : t("skills.invalid") }}
                </span>
              </div>
              <p>{{ skill.value.description }}</p>
              <dl class="skill-settings__meta">
                <div>
                  <dt>{{ t("skills.activation") }}</dt>
                  <dd>{{ skill.value.activation_mode }}</dd>
                </div>
                <div>
                  <dt>{{ t("skills.source") }}</dt>
                  <dd>{{ skill.value.install_source }}</dd>
                </div>
                <div>
                  <dt>{{ t("skills.update") }}</dt>
                  <dd>{{ formatUpdateState(skill.value.update_state) }}</dd>
                </div>
                <div>
                  <dt>{{ t("skills.path") }}</dt>
                  <dd>{{ skill.value.path }}</dd>
                </div>
              </dl>
              <KxInlineAlert
                v-if="skill.value.validation_error"
                tone="error"
                compact
                :data-test="`skill-invalid-${skillSettingsTestId(skill)}`"
              >
                {{ skill.value.validation_error }}
              </KxInlineAlert>
            </div>

            <div class="skill-settings__actions" aria-label="Skill actions">
              <button
                class="btn btn-sm"
                type="button"
                :disabled="!skill.writable || busySkillId === skill.value.settings_id"
                :data-test="`skill-enabled-${skillSettingsTestId(skill)}`"
                @click="
                  runSkillAction(skill.value.settings_id, () =>
                    skillsStore.setSkillEnabled(skill.value.settings_id, !skill.enabled)
                  )
                "
              >
                {{ skill.enabled ? t("skills.disable") : t("skills.enable") }}
              </button>
              <button
                class="btn btn-sm"
                type="button"
                :disabled="!canUpdateSkill(skill) || busySkillId === skill.value.settings_id"
                :data-test="`skill-update-${skillSettingsTestId(skill)}`"
                @click="
                  runSkillAction(skill.value.settings_id, () =>
                    skillsStore.updateSkill(skill.value.settings_id)
                  )
                "
              >
                {{ t("skills.updateSkill") }}
              </button>
              <button
                class="btn btn-danger btn-sm"
                type="button"
                :disabled="!skill.writable || busySkillId === skill.value.settings_id"
                :data-test="`skill-delete-${skillSettingsTestId(skill)}`"
                @click="
                  runSkillAction(skill.value.settings_id, () =>
                    skillsStore.deleteSkill(skill.value.settings_id)
                  )
                "
              >
                {{ t("skills.delete") }}
              </button>
            </div>
          </SettingsCardItem>
        </SettingsCardList>
      </div>
    </div>

    <div v-if="activeSubTab === 'discover'" class="skill-settings__discover">
      <details class="advanced-install" data-test="skill-advanced-install">
        <summary>{{ t("skills.advancedInstall") }}</summary>
        <form
          class="skill-settings__inline-form advanced-install__form"
          data-test="skill-github-form"
          @submit.prevent="installFromGithub"
        >
          <KxFormField class="advanced-install__field" :label="t('skills.githubUrl')">
            <input
              id="skill-github-source"
              v-model="githubSource"
              class="kx-form-control"
              type="text"
              data-test="skill-github-source"
              placeholder="https://github.com/org/repo/tree/main/path/to/skill"
            />
          </KxFormField>
          <button
            class="btn btn-primary"
            type="submit"
            :disabled="skillsStore.settingsLoading || !githubSource.trim()"
            data-test="skill-github-submit"
          >
            {{ skillsStore.settingsLoading ? t("skills.installing") : t("skills.installButton") }}
          </button>
        </form>
      </details>

      <div class="skill-settings__discover-body">
        <SkillDiscoverList :install-target="skillCatalogInstallTarget" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.skill-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
  overflow: hidden;
}

.skill-settings__installed {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.skill-settings__installed > .skill-settings__body {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.skill-settings__discover {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: hidden;
}

.skill-settings__discover-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.skill-settings__title-row,
.skill-settings__remote,
.skill-settings__inline-form,
.skill-settings__actions {
  display: flex;
  gap: 12px;
}

.skill-settings__remote {
  align-items: flex-start;
  justify-content: space-between;
}

.skill-settings__remote h4 {
  margin: 0;
}

.skill-settings__title-row h4 {
  margin: 0;
}

.skill-settings__remote p {
  margin: 4px 0 0;
  color: var(--app-text-color-2, #6b7280);
}

.skill-settings__main > p {
  margin: 4px 0 0;
  color: var(--app-text-color-2, #6b7280);
}

.skill-settings__section .card-header h3 {
  font-size: 14px;
}

.skill-settings__body,
.skill-settings__main,
.skill-settings__remote-list {
  display: grid;
  gap: 12px;
}

.skill-settings__remote:last-child {
  padding-bottom: 0;
  border-bottom-style: none;
}

.skill-settings__title-row,
.skill-settings__actions {
  flex-wrap: wrap;
  align-items: center;
}

.skill-settings__actions {
  justify-content: flex-end;
}

.skill-settings__meta {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px;
  margin: 0;
}

.skill-settings__meta dt {
  color: var(--app-text-color-2, #6b7280);
  font-size: 12px;
  font-weight: 600;
}

.skill-settings__meta dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.skill-settings__inline-form {
  flex-wrap: wrap;
  align-items: end;
}

.skill-settings__remote-list {
  margin-top: 16px;
}

.skill-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
}

.advanced-install {
  max-width: 760px;
  padding: 8px 0;
}

.advanced-install summary {
  cursor: pointer;
  color: var(--app-text-color);
  font-weight: 600;
}

.advanced-install__form {
  margin-top: 12px;
}

.advanced-install__field {
  flex: 1 1 320px;
}

/* Source tags for effective (unified) view */
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
