<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { SkillSettingsView } from "@/generated/commands";

const { t } = useI18n();
const skillsStore = useSkillsStore();
const activeSubTab = ref<"installed" | "discover">("installed");
const discoverQuery = ref("");
const githubSource = ref("");
const installTarget = ref<"project" | "user">("project");
const busySkillId = ref<string | null>(null);

onMounted(() => {
  void skillsStore.loadSkillSettings();
});

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

async function searchRemoteSkills(): Promise<void> {
  const trimmedQuery = discoverQuery.value.trim();
  if (!trimmedQuery) {
    return;
  }

  await skillsStore.searchRemoteSkills(trimmedQuery);
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

    <template v-if="activeSubTab === 'installed'">
      <div class="skill-toolbar">
        <button
          class="btn"
          type="button"
          :disabled="skillsStore.settingsLoading"
          data-test="skill-refresh"
          @click="skillsStore.loadSkillSettings()"
        >
          {{ skillsStore.settingsLoading ? t("skills.refreshing") : t("skills.refreshSkills") }}
        </button>
      </div>

      <p v-if="skillsStore.settingsLoading" class="alert alert-info" role="status">
        {{ t("skills.loading") }}
      </p>
      <p v-else-if="skillsStore.skillSettings.length === 0" class="empty-state">
        {{ t("skills.noSkills") }}
      </p>

      <article
        v-for="skill in skillsStore.skillSettings"
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
              {{ skillsStore.settingsLoading ? t("skills.installing") : t("skills.installButton") }}
            </button>
          </form>
        </div>
      </section>
    </template>

    <template v-if="activeSubTab === 'discover'">
      <form
        class="skill-settings__search-form"
        data-test="skill-discover-form"
        @submit.prevent="searchRemoteSkills"
      >
        <input
          id="skill-discover-query"
          v-model="discoverQuery"
          type="search"
          data-test="skill-discover-query"
          :placeholder="t('skills.searchPlaceholder')"
        />
        <button
          class="btn btn-primary"
          type="submit"
          :disabled="skillsStore.remoteLoading || !discoverQuery.trim()"
          data-test="skill-discover-submit"
        >
          {{ skillsStore.remoteLoading ? t("skills.searching") : t("skills.search") }}
        </button>
      </form>

      <div class="skill-settings__remote-list" aria-label="Remote skill results">
        <article
          v-for="result in skillsStore.remoteResults"
          :key="result.package"
          class="card skill-settings__remote"
          :data-test="`skill-remote-${slugify(result.name)}`"
        >
          <div>
            <h4>{{ result.name }}</h4>
            <p>{{ result.description }}</p>
            <span class="tag">{{
              t("skills.installs", { count: result.install_count ?? 0 })
            }}</span>
          </div>
          <button
            class="btn btn-sm"
            type="button"
            :disabled="skillsStore.settingsLoading"
            :data-test="`skill-install-${slugify(result.name)}`"
            @click="skillsStore.installRemoteSkill(result.package, installTarget)"
          >
            {{ skillsStore.settingsLoading ? t("skills.installing") : t("skills.install") }}
          </button>
        </article>
      </div>
    </template>
  </section>
</template>

<style scoped>
.skill-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
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
</style>
