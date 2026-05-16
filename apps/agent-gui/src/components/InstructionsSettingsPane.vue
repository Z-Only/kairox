<script setup lang="ts">
import type { ConfigScope, InstructionsView } from "@/generated/commands";
import { commands } from "@/generated/commands";

const { t } = useI18n();

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

const view = ref<InstructionsView>({ system: "", user: null, project: null });
const userText = ref("");
const projectText = ref("");
const saving = ref(false);
const errorMsg = ref("");
const loaded = ref(false);

const scope = computed<ConfigScope>(() => (configSource?.value === "project" ? "Project" : "User"));

const projectRoot = computed(() =>
  configSource?.value === "project" ? (configProjectId?.value ?? null) : null
);

const effectiveInstructions = computed(() => {
  const parts: string[] = [view.value.system];
  if (view.value.user) parts.push(view.value.user);
  if (view.value.project) parts.push(view.value.project);
  return parts.filter(Boolean).join("\n\n");
});

async function load(): Promise<void> {
  errorMsg.value = "";
  try {
    const fetchedView = await commands.getInstructions(scope.value, projectRoot.value);
    view.value = fetchedView;
    userText.value = fetchedView.user ?? "";
    projectText.value = fetchedView.project ?? "";
    loaded.value = true;
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function save(): Promise<void> {
  errorMsg.value = "";
  saving.value = true;
  try {
    const text =
      configSource?.value === "project" ? projectText.value.trim() : userText.value.trim();
    await commands.upsertInstructions({ scope: scope.value, text }, projectRoot.value);
    await load();
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    saving.value = false;
  }
}

watch(
  [() => configSource?.value, () => configProjectId?.value],
  () => {
    load();
  },
  { immediate: true }
);
</script>

<template>
  <section
    class="instructions-pane"
    role="tabpanel"
    aria-label="Instructions settings"
    data-test="instructions-settings-pane"
  >
    <p v-if="errorMsg" class="alert alert-error" role="alert" data-test="instructions-error">
      {{ errorMsg }}
    </p>

    <div v-if="loaded" class="instructions-levels">
      <!-- System level -->
      <div class="instructions-level" data-test="instructions-level-system">
        <header class="instructions-level__header">
          <h3>{{ t("instructions.system") }}</h3>
          <span class="instructions-level__badge" data-test="badge-system">{{
            t("instructions.readOnly")
          }}</span>
        </header>
        <textarea
          class="instructions-level__textarea"
          :value="view.system"
          readonly
          rows="6"
          data-test="system-instructions"
        />
      </div>

      <!-- User level -->
      <div class="instructions-level" data-test="instructions-level-user">
        <header class="instructions-level__header">
          <h3>{{ t("instructions.user") }}</h3>
          <span
            v-if="scope === 'Project'"
            class="instructions-level__badge"
            data-test="badge-user-readonly"
          >
            {{ t("instructions.readOnly") }}
          </span>
          <span v-else class="instructions-level__badge" data-test="badge-user-editable">
            {{ t("instructions.editable") }}
          </span>
        </header>
        <textarea
          class="instructions-level__textarea"
          :value="userText"
          :readonly="scope === 'Project'"
          :placeholder="t('instructions.userPlaceholder')"
          rows="6"
          data-test="user-instructions"
          @input="userText = ($event.target as HTMLTextAreaElement).value"
        />
      </div>

      <!-- Project level -->
      <div class="instructions-level" data-test="instructions-level-project">
        <header class="instructions-level__header">
          <h3>{{ t("instructions.project") }}</h3>
          <span
            v-if="scope === 'User'"
            class="instructions-level__badge instructions-level__badge--muted"
            data-test="badge-project-disabled"
          >
            {{ t("instructions.projectScopeRequired") }}
          </span>
          <span v-else class="instructions-level__badge" data-test="badge-project-editable">
            {{ t("instructions.editable") }}
          </span>
        </header>
        <textarea
          class="instructions-level__textarea"
          :value="projectText"
          :readonly="scope === 'User'"
          :disabled="scope === 'User'"
          :placeholder="t('instructions.projectPlaceholder')"
          rows="6"
          data-test="project-instructions"
          @input="projectText = ($event.target as HTMLTextAreaElement).value"
        />
      </div>

      <button
        class="instructions__save-btn"
        data-test="instructions-save"
        :disabled="saving"
        @click="save"
      >
        {{ t("common.save") }}
      </button>

      <!-- Effective preview -->
      <div class="instructions-level" data-test="instructions-preview">
        <header class="instructions-level__header">
          <h3>{{ t("instructions.effectivePreview") }}</h3>
        </header>
        <textarea
          class="instructions-level__textarea instructions-level__textarea--preview"
          :value="effectiveInstructions"
          readonly
          rows="12"
          data-test="effective-instructions"
        />
      </div>
    </div>

    <p v-else class="instructions__loading" data-test="instructions-loading">
      {{ t("common.loading") }}
    </p>
  </section>
</template>

<style scoped>
.instructions-pane {
  padding: 12px 0;
}

.instructions-levels {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.instructions-level__header {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 6px;
}

.instructions-level__header h3 {
  margin: 0;
  font-size: 0.94rem;
  font-weight: 600;
}

.instructions-level__badge {
  font-size: 0.72rem;
  padding: 1px 6px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  color: var(--app-primary-color);
}

.instructions-level__badge--muted {
  background: color-mix(in srgb, var(--app-text-color-2) 15%, transparent);
  color: var(--app-text-color-2);
}

.instructions-level__textarea {
  width: 100%;
  padding: 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  font: inherit;
  font-size: 0.84rem;
  resize: vertical;
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
}

.instructions-level__textarea:read-only {
  background: color-mix(in srgb, var(--app-card-color) 60%, transparent);
  color: var(--app-text-color-2);
  cursor: default;
}

.instructions-level__textarea:disabled {
  background: color-mix(in srgb, var(--app-card-color) 40%, transparent);
  color: var(--app-text-color-2);
  cursor: not-allowed;
}

.instructions-level__textarea--preview {
  border-color: var(--app-primary-color);
}

.instructions__save-btn {
  align-self: flex-start;
  padding: 6px 20px;
  border: none;
  border-radius: 6px;
  background: var(--app-primary-color);
  color: #fff;
  font: inherit;
  font-size: 0.84rem;
  cursor: pointer;
}

.instructions__save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.instructions__save-btn:hover:not(:disabled) {
  opacity: 0.85;
}

.instructions__loading {
  color: var(--app-text-color-2);
  font-size: 0.84rem;
}

.alert-error {
  padding: 8px 12px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--app-error-color, #e53e3e) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--app-error-color, #e53e3e) 30%, transparent);
  color: var(--app-error-color, #e53e3e);
  font-size: 0.84rem;
  margin-bottom: 12px;
}
</style>
