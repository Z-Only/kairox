<script setup lang="ts">
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";
import { useSkillsStore } from "@/stores/skills";
import MarketplacePane from "@/components/MarketplacePane.vue";

// Hoisted to module scope + `as const` so the option arrays are not rebuilt
// per-render and their literal types are preserved through the template.
const themes = [
  { value: "auto", labelKey: "settings.themeAuto" },
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" }
] as const satisfies ReadonlyArray<{ value: ThemeMode; labelKey: string }>;

const locales = [
  { value: "en", labelKey: "settings.localeEn" },
  { value: "zh-CN", labelKey: "settings.localeZh" }
] as const satisfies ReadonlyArray<{
  value: SupportedLocale;
  labelKey: string;
}>;

const { t } = useI18n();
const ui = useUiStore();
const skillsStore = useSkillsStore();
const { locale, colorMode } = storeToRefs(ui);
const { skills, loading: skillsLoading, error: skillsError } = storeToRefs(skillsStore);
const activeTab = ref<"general" | "skills" | "marketplace">("general");
const isThemeSelectFocused = ref(false);
const skillsLoaded = ref(false);

async function showSkillsTab(): Promise<void> {
  activeTab.value = "skills";
  if (skillsLoaded.value) {
    return;
  }

  await skillsStore.loadSkills();
  skillsLoaded.value = skillsStore.error === null;
}

function formatSkillMeta(label: string, value: string | null): string {
  return `${label}: ${value || "Not specified"}`;
}
</script>

<template>
  <main class="settings" data-test="view-settings">
    <h1>{{ t("settings.title") }}</h1>

    <div class="tabs" role="tablist">
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'general'"
        @click="activeTab = 'general'"
      >
        {{ t("settings.general") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'skills'"
        data-test="settings-tab-skills"
        @click="showSkillsTab"
      >
        Skills
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'marketplace'"
        data-test="settings-tab-marketplace"
        @click="activeTab = 'marketplace'"
      >
        {{ t("nav.marketplace") }}
      </button>
    </div>

    <div v-show="activeTab === 'general'" role="tabpanel">
      <div class="settings__row">
        <label for="settings-locale">{{ t("settings.locale") }}</label>
        <select
          id="settings-locale"
          :value="locale"
          data-test="settings-locale"
          @change="ui.setLocale(($event.target as HTMLSelectElement).value as SupportedLocale)"
        >
          <option v-for="opt in locales" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>
      </div>

      <div class="settings__row" data-test="theme-toggle">
        <label for="settings-theme">{{ t("settings.theme") }}</label>
        <select
          id="settings-theme"
          :value="colorMode"
          :class="{ 'settings__select--focused': isThemeSelectFocused }"
          data-test="settings-theme"
          @focus="isThemeSelectFocused = true"
          @blur="isThemeSelectFocused = false"
          @change="ui.setTheme(($event.target as HTMLSelectElement).value as ThemeMode)"
        >
          <option v-for="opt in themes" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>
      </div>
    </div>

    <section
      v-show="activeTab === 'skills'"
      class="skills-panel"
      role="tabpanel"
      data-test="settings-skills-panel"
    >
      <div class="skills-panel__header">
        <div>
          <h2>Agent skills</h2>
          <p>Discover and activate native skills for the current session.</p>
        </div>
        <button
          class="skills-panel__button"
          type="button"
          :disabled="skillsLoading"
          data-test="settings-skills-refresh"
          @click="skillsStore.loadSkills"
        >
          Refresh
        </button>
      </div>

      <p v-if="skillsLoading" class="skills-panel__status">Loading skills…</p>
      <p v-else-if="skillsError" class="skills-panel__error" role="alert">
        {{ skillsError }}
      </p>
      <p v-else-if="!skillsStore.hasSkills" class="skills-panel__status">
        No skills discovered yet.
      </p>

      <div v-else class="skills-list">
        <article
          v-for="skill in skills"
          :key="skill.id"
          class="skill-card"
          :data-test="`skill-card-${skill.id}`"
        >
          <div class="skill-card__body">
            <div class="skill-card__title-row">
              <h3>{{ skill.name }}</h3>
              <span
                class="skill-card__status"
                :class="{ 'skill-card__status--invalid': !skill.valid }"
              >
                {{ skill.valid ? "Valid" : "Invalid" }}
              </span>
            </div>
            <p class="skill-card__description">{{ skill.description }}</p>
            <dl class="skill-card__meta">
              <div>
                <dt>Source</dt>
                <dd>{{ skill.source }}</dd>
              </div>
              <div>
                <dt>Activation</dt>
                <dd>{{ formatSkillMeta("Mode", skill.activation_mode) }}</dd>
              </div>
              <div v-if="skill.version">
                <dt>Version</dt>
                <dd>{{ skill.version }}</dd>
              </div>
            </dl>
            <p v-if="skill.validation_error" class="skill-card__validation" role="alert">
              {{ skill.validation_error }}
            </p>
          </div>

          <button
            class="skills-panel__button"
            type="button"
            :disabled="!skill.valid || skillsStore.activatingSkillId === skill.id"
            :data-test="`skill-toggle-${skill.id}`"
            @click="
              skillsStore.isSkillActive(skill.id)
                ? skillsStore.deactivateSkill(skill.id)
                : skillsStore.activateSkill(skill.id)
            "
          >
            {{ skillsStore.isSkillActive(skill.id) ? "Deactivate" : "Activate" }}
          </button>
        </article>
      </div>
    </section>

    <div v-show="activeTab === 'marketplace'" role="tabpanel">
      <MarketplacePane />
    </div>
  </main>
</template>

<style scoped>
.settings {
  padding: 16px;
  max-width: 640px;
  flex: 1;
  overflow: auto;
}
.settings__row {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-block: 12px;
}
.settings__row label {
  min-width: 100px;
}

select:focus,
.settings__select--focused {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
  box-shadow: inset 0 0 0 2px var(--app-primary-color, #3b82f6);
  background-color: color-mix(in srgb, var(--app-primary-color, #3b82f6) 12%, transparent);
}

.tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--border-color, #e0e0e0);
  margin-bottom: 12px;
}

.tab-btn {
  padding: 8px 16px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: inherit;
  color: var(--app-text-color-2, #6b7280);
  border-bottom: 2px solid transparent;
  transition:
    color 0.2s,
    border-color 0.2s;
}

.tab-btn[aria-selected="true"] {
  color: var(--primary-color, #18a058);
  border-bottom-color: var(--primary-color, #18a058);
}

.tab-btn:hover {
  color: var(--primary-color, #18a058);
}

.skills-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.skills-panel__header {
  display: flex;
  gap: 16px;
  align-items: flex-start;
  justify-content: space-between;
}

.skills-panel__header h2 {
  margin: 0 0 4px;
  font-size: 1.125rem;
}

.skills-panel__header p,
.skills-panel__status {
  margin: 0;
  color: var(--app-text-color-2, #6b7280);
}

.skills-panel__button {
  min-height: 44px;
  padding: 8px 14px;
  border: 1px solid var(--border-color, #e0e0e0);
  border-radius: 8px;
  background: var(--app-surface-color, #ffffff);
  color: var(--app-text-color, #111827);
  cursor: pointer;
  transition:
    border-color 0.2s,
    color 0.2s,
    background-color 0.2s;
}

.skills-panel__button:hover:not(:disabled) {
  border-color: var(--primary-color, #18a058);
  color: var(--primary-color, #18a058);
}

.skills-panel__button:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.skills-panel__error,
.skill-card__validation {
  margin: 0;
  color: var(--app-danger-color, #dc2626);
}

.skills-list {
  display: grid;
  gap: 12px;
}

.skill-card {
  display: flex;
  gap: 16px;
  align-items: flex-start;
  justify-content: space-between;
  padding: 16px;
  border: 1px solid var(--border-color, #e0e0e0);
  border-radius: 12px;
  background: var(--app-surface-color, #ffffff);
}

.skill-card__body {
  min-width: 0;
}

.skill-card__title-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 6px;
}

.skill-card__title-row h3 {
  margin: 0;
  font-size: 1rem;
}

.skill-card__status {
  padding: 2px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--primary-color, #18a058) 14%, transparent);
  color: var(--primary-color, #18a058);
  font-size: 0.75rem;
  font-weight: 600;
}

.skill-card__status--invalid {
  background: color-mix(in srgb, var(--app-danger-color, #dc2626) 12%, transparent);
  color: var(--app-danger-color, #dc2626);
}

.skill-card__description {
  margin: 0 0 12px;
  color: var(--app-text-color-2, #6b7280);
}

.skill-card__meta {
  display: grid;
  gap: 8px;
  margin: 0;
}

.skill-card__meta div {
  display: grid;
  gap: 2px;
}

.skill-card__meta dt {
  color: var(--app-text-color-2, #6b7280);
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
}

.skill-card__meta dd {
  margin: 0;
  overflow-wrap: anywhere;
}
</style>
