<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { SkillSettingsView, SkillCatalogQuery } from "@/generated/commands";
import { commands } from "@/generated/commands";
import SkillDiscoverList from "@/components/skills/SkillDiscoverList.vue";
import SkillSourcesSettings from "@/components/skills/SkillSourcesSettings.vue";

const { t } = useI18n();
const skillsStore = useSkillsStore();
const activeSubTab = ref<"installed" | "discover">("installed");
const sourceSettingsOpen = ref(false);
const discoverKeyword = ref("");
const githubSource = ref("");
const installTarget = ref<"project" | "user">("user");
const busySkillId = ref<string | null>(null);

const discoverSourceChips = computed(() => {
  const remoteSources = skillsStore.catalogSources
    .filter((s) => s.id !== "builtin")
    .map((s) => ({
      id: s.id,
      display_name: s.display_name
    }));
  return [{ id: "builtin", display_name: t("skills.builtinSource") }, ...remoteSources];
});

async function searchSkillsCatalog(): Promise<void> {
  const query: SkillCatalogQuery = {
    keyword: discoverKeyword.value.trim() || null,
    sources: null,
    limit: 50
  };
  await skillsStore.searchCatalog(query);
}

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

// Sync the GitHub install target with the ConfigSourceBar selection.
watch(
  () => configSource?.value,
  (src) => {
    if (src) installTarget.value = src;
  },
  { immediate: true }
);

const filteredSkills = computed(() => {
  const all = skillsStore.skillSettings;
  if (!configSource?.value) return all;
  if (configSource.value === "project") {
    return all.filter((s) => s.scope === "project");
  }
  return all.filter((s) => s.scope !== "project");
});

watch(
  [() => configSource?.value, () => configProjectId?.value],
  () => {
    void skillsStore.loadSkillSettings();
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

function canUpdateSkill(skill: SkillSettingsView): boolean {
  return (
    skill.editable &&
    skill.install_source !== "builtin" &&
    skill.update_state === "update_available"
  );
}

function skillSettingsTestId(skill: SkillSettingsView): string {
  return slugify(skill.settings_id);
}

async function runSkillAction(skillId: string, action: () => Promise<unknown>): Promise<void> {
  busySkillId.value = skillId;
  try {
    await action();
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

  const installedSkill = await skillsStore.installGithubSkill(trimmedSource, installTarget.value);
  if (installedSkill) {
    githubSource.value = "";
  }
}
</script>

<template>
  <section class="skill-settings" aria-label="Skills settings" data-test="skill-settings-pane">
    <p v-if="skillsStore.error" class="alert alert-error" role="alert" data-test="skill-page-error">
      {{ skillsStore.error }}
    </p>

    <div class="skill-sub-tabs" role="tablist" aria-label="Skill sections">
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
    </div>

    <div v-if="activeSubTab === 'installed'" class="skill-settings__installed">
      <div class="skill-toolbar">
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
      </div>

      <div class="skill-settings__body">
        <p v-if="skillsStore.settingsLoading" class="alert alert-info" role="status">
          {{ t("skills.loading") }}
        </p>
        <p v-else-if="filteredSkills.length === 0" class="empty-state">
          {{ t("skills.noSkills") }}
        </p>

        <article
          v-for="skill in filteredSkills"
          v-else
          :key="skill.settings_id"
          class="skill-settings__row"
          :data-test="`skill-row-${skillSettingsTestId(skill)}`"
        >
          <div class="skill-settings__main">
            <div class="skill-settings__title-row">
              <h4>{{ skill.name }}</h4>
              <span class="tag">{{ skill.scope }}</span>
              <span :class="['tag', skill.enabled ? 'tag-success' : 'tag-warning']">
                {{ skill.enabled ? t("skills.enabled") : t("skills.disabled") }}
              </span>
              <span :class="['tag', skill.effective ? 'tag-success' : 'tag-warning']">
                {{
                  skill.effective
                    ? t("skills.effective")
                    : t("skills.shadowedBy", { name: skill.shadowed_by })
                }}
              </span>
              <span :class="['tag', skill.valid ? 'tag-success' : 'tag-error']">
                {{ skill.valid ? t("skills.valid") : t("skills.invalid") }}
              </span>
            </div>
            <p>{{ skill.description }}</p>
            <dl class="skill-settings__meta">
              <div>
                <dt>{{ t("skills.activation") }}</dt>
                <dd>{{ skill.activation_mode }}</dd>
              </div>
              <div>
                <dt>{{ t("skills.source") }}</dt>
                <dd>{{ skill.install_source }}</dd>
              </div>
              <div>
                <dt>{{ t("skills.update") }}</dt>
                <dd>{{ formatUpdateState(skill.update_state) }}</dd>
              </div>
              <div>
                <dt>{{ t("skills.path") }}</dt>
                <dd>{{ skill.path }}</dd>
              </div>
            </dl>
            <p
              v-if="skill.validation_error"
              class="alert alert-error"
              role="alert"
              :data-test="`skill-invalid-${skillSettingsTestId(skill)}`"
            >
              {{ skill.validation_error }}
            </p>
          </div>

          <div class="skill-settings__actions" aria-label="Skill actions">
            <button
              class="btn btn-sm"
              type="button"
              :disabled="busySkillId === skill.settings_id"
              :data-test="`skill-enabled-${skillSettingsTestId(skill)}`"
              @click="
                runSkillAction(skill.settings_id, () =>
                  skillsStore.setSkillEnabled(skill.settings_id, !skill.enabled)
                )
              "
            >
              {{ skill.enabled ? t("skills.disable") : t("skills.enable") }}
            </button>
            <button
              class="btn btn-sm"
              type="button"
              disabled
              :title="'Skill editing is not available in this settings pane yet.'"
              :data-test="`skill-edit-${skillSettingsTestId(skill)}`"
            >
              {{ t("skills.edit") }}
            </button>
            <button
              class="btn btn-sm"
              type="button"
              :disabled="!canUpdateSkill(skill) || busySkillId === skill.settings_id"
              :data-test="`skill-update-${skillSettingsTestId(skill)}`"
              @click="
                runSkillAction(skill.settings_id, () => skillsStore.updateSkill(skill.settings_id))
              "
            >
              {{ t("skills.updateSkill") }}
            </button>
            <button
              class="btn btn-danger btn-sm"
              type="button"
              :disabled="!skill.deletable || busySkillId === skill.settings_id"
              :data-test="`skill-delete-${skillSettingsTestId(skill)}`"
              @click="
                runSkillAction(skill.settings_id, () => skillsStore.deleteSkill(skill.settings_id))
              "
            >
              {{ t("skills.delete") }}
            </button>
          </div>
        </article>

        <section class="card skill-settings__section" aria-labelledby="github-skills-title">
          <div class="card-header">
            <h3 id="github-skills-title">{{ t("skills.installFromGithub") }}</h3>
          </div>
          <div class="card-body skill-settings__body">
            <form
              class="skill-settings__inline-form"
              data-test="skill-github-form"
              @submit.prevent="installFromGithub"
            >
              <label for="skill-install-target">{{ t("skills.target") }}</label>
              <select
                id="skill-install-target"
                v-model="installTarget"
                data-test="skill-install-target"
              >
                <option value="project">{{ t("skills.targetProject") }}</option>
                <option value="user">{{ t("skills.targetUser") }}</option>
              </select>

              <label for="skill-github-source">{{ t("skills.githubUrl") }}</label>
              <input
                id="skill-github-source"
                v-model="githubSource"
                type="url"
                data-test="skill-github-source"
                placeholder="https://github.com/org/skill.git"
              />
              <button
                class="btn btn-primary"
                type="submit"
                :disabled="skillsStore.settingsLoading || !githubSource.trim()"
                data-test="skill-github-submit"
              >
                {{
                  skillsStore.settingsLoading ? t("skills.installing") : t("skills.installButton")
                }}
              </button>
            </form>
          </div>
        </section>
      </div>
    </div>

    <template v-if="activeSubTab === 'discover'">
      <div class="source-filter">
        <button
          v-for="chip in discoverSourceChips"
          :key="chip.id"
          :class="['btn', 'chip', { active: skillsStore.isCatalogSourceEnabled(chip.id) }]"
          data-test="skill-source-chip"
          @click="skillsStore.toggleCatalogSource(chip.id)"
        >
          {{ chip.display_name }}
        </button>
        <button
          class="btn settings-icon"
          data-test="skill-source-settings-btn"
          :aria-label="t('marketplace.sourceSettingsAria')"
          @click="sourceSettingsOpen = !sourceSettingsOpen"
        >
          <span aria-hidden="true">⚙</span>
        </button>
      </div>

      <div
        v-if="sourceSettingsOpen"
        class="card settings-drawer"
        data-test="skill-source-settings-drawer"
      >
        <SkillSourcesSettings />
      </div>

      <div class="discover-search-row">
        <input
          v-model="discoverKeyword"
          class="discover-search-input"
          type="search"
          :placeholder="t('skills.searchPlaceholder')"
          data-test="skill-catalog-search"
          @keyup.enter="searchSkillsCatalog()"
        />
        <button
          class="btn btn-primary btn-sm"
          type="button"
          data-test="skill-catalog-search-btn"
          @click="searchSkillsCatalog()"
        >
          {{ t("common.search") }}
        </button>
      </div>

      <SkillDiscoverList />
    </template>
  </section>
</template>

<style scoped>
.skill-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow: hidden;
}

.skill-sub-tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--app-border-color, #e0e0e0);
}

.sub-tab-btn {
  padding: 6px 14px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 13px;
  color: var(--app-text-color-2, #6b7280);
  border-bottom: 2px solid transparent;
  transition:
    color 0.2s,
    border-color 0.2s;
}

.sub-tab-btn[aria-selected="true"] {
  color: var(--app-primary-color, #18a058);
  border-bottom-color: var(--app-primary-color, #18a058);
}

.sub-tab-btn:hover {
  color: var(--app-primary-color, #18a058);
}

.sub-tab-btn:focus-visible {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
}

.skill-toolbar {
  display: flex;
  gap: 8px;
  align-items: center;
  flex: none;
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

.skill-settings__title-row,
.skill-settings__row,
.skill-settings__remote,
.skill-settings__inline-form,
.skill-settings__actions {
  display: flex;
  gap: 12px;
}

.skill-settings__row,
.skill-settings__remote {
  align-items: flex-start;
  justify-content: space-between;
}

.skill-settings__row h4,
.skill-settings__remote h4 {
  margin: 0;
}

.skill-settings__row p,
.skill-settings__remote p {
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

.skill-settings__row:last-child,
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

.skill-settings__inline-form label {
  display: grid;
  gap: 4px;
  font-weight: 600;
}

.skill-settings__search-form {
  display: flex;
  gap: 8px;
  align-items: center;
}

.skill-settings__search-form input {
  flex: 1;
  min-height: 36px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  color: var(--app-text-color, #111827);
}

.skill-settings__remote-list {
  margin-top: 16px;
}

.skill-settings input,
.skill-settings select {
  min-height: 36px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  color: var(--app-text-color, #111827);
}

.skill-settings input:focus,
.skill-settings select:focus,
.skill-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
}

.source-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.source-filter .chip {
  padding: 4px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 14px;
  background: var(--app-card-color);
  cursor: pointer;
  color: var(--app-text-color);
  font-size: 13px;
}

.source-filter .chip.active {
  background: var(--app-primary-color, #18a058);
  color: #fff;
  border-color: var(--app-primary-color, #18a058);
}

.source-filter .settings-icon {
  padding: 4px 8px;
  font-size: 16px;
  margin-left: auto;
}

.discover-search-row {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.discover-search-input {
  flex: 1;
  max-width: 320px;
  min-height: 32px;
  padding: 4px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  font-size: 13px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}

.settings-drawer {
  margin-top: 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 12px;
}
</style>
