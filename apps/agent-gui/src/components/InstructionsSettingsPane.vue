<script setup lang="ts">
import type { ConfigScope, InstructionsView } from "@/generated/commands";
import { commands } from "@/generated/commands";
import { useProjectStore } from "@/stores/project";

const { t } = useI18n();
const projectStore = useProjectStore();

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

const view = ref<InstructionsView>({ system: "", user: null, project: null });
const userText = ref("");
const projectText = ref("");
const saving = ref(false);
const errorMsg = ref("");
const loaded = ref(false);
let loadRequestId = 0;

const scope = computed<ConfigScope>(() => (configSource?.value === "project" ? "Project" : "User"));

const projectRoot = computed(() => {
  if (configSource?.value !== "project") return null;
  const projectId = configProjectId?.value;
  if (!projectId) return null;
  return (
    projectStore.activeProjects.find((project) => project.projectId === projectId)?.rootPath ?? null
  );
});

const effectiveInstructions = computed(() => {
  const parts: string[] = [view.value.system];
  if (view.value.user) parts.push(view.value.user);
  if (view.value.project) parts.push(view.value.project);
  return parts.filter(Boolean).join("\n\n");
});

async function load(resetLoaded = true): Promise<void> {
  const requestId = ++loadRequestId;
  const requestedScope = scope.value;
  const requestedProjectRoot = projectRoot.value;
  const isCurrentRequest = () =>
    requestId === loadRequestId &&
    requestedScope === scope.value &&
    requestedProjectRoot === projectRoot.value;

  errorMsg.value = "";
  if (resetLoaded) {
    loaded.value = false;
  }
  if (requestedScope === "Project" && !requestedProjectRoot) {
    return;
  }
  try {
    const result = await commands.getInstructions(requestedScope, requestedProjectRoot);
    if (!isCurrentRequest()) {
      return;
    }
    if (isCommandResult(result)) {
      if (result.status === "error") throw new Error(String(result.error));
      view.value = (result as { data: InstructionsView }).data;
    } else {
      view.value = result;
    }
    userText.value = view.value.user ?? "";
    projectText.value = view.value.project ?? "";
    loaded.value = true;
  } catch (e) {
    if (!isCurrentRequest()) {
      return;
    }
    errorMsg.value = String(e);
  }
}

function isCommandResult(
  value: unknown
): value is { status: string; data?: unknown; error?: unknown } {
  return (
    typeof value === "object" &&
    value !== null &&
    "status" in value &&
    ((value as { status: string }).status === "ok" ||
      (value as { status: string }).status === "error")
  );
}

async function save(): Promise<void> {
  errorMsg.value = "";
  saving.value = true;
  const savedScope = scope.value;
  const savedProjectRoot = projectRoot.value;
  try {
    const text = savedScope === "Project" ? projectText.value.trim() : userText.value.trim();
    const result = await commands.upsertInstructions({ scope: savedScope, text }, savedProjectRoot);
    if (isCommandResult(result) && result.status === "error") {
      throw new Error(String(result.error));
    }
    if (savedScope === scope.value && savedProjectRoot === projectRoot.value) {
      await load(false);
    }
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    saving.value = false;
  }
}

watch(
  [() => scope.value, () => projectRoot.value],
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
    :aria-label="t('instructions.title')"
    data-test="instructions-settings-pane"
  >
    <SettingsState v-if="errorMsg" tone="error" data-test="instructions-error">
      {{ errorMsg }}
    </SettingsState>

    <div v-if="loaded" class="instructions-levels">
      <!-- System level (hidden when editing project config) -->
      <div
        v-if="scope !== 'Project'"
        class="instructions-level"
        data-test="instructions-level-system"
      >
        <header class="instructions-level__header">
          <h3>{{ t("instructions.system") }}</h3>
          <span class="instructions-level__badge" data-test="badge-system">{{
            t("instructions.readOnly")
          }}</span>
        </header>
        <KxTextarea
          :model-value="view.system"
          readonly
          rows="6"
          variant="mono"
          data-test="system-instructions"
        />
      </div>

      <!-- User level (hidden when editing project config) -->
      <div
        v-if="scope !== 'Project'"
        class="instructions-level"
        data-test="instructions-level-user"
      >
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
        <KxTextarea
          v-model="userText"
          :readonly="scope === 'Project'"
          :placeholder="t('instructions.userPlaceholder')"
          rows="6"
          variant="mono"
          data-test="user-instructions"
        />
      </div>

      <!-- Project level -->
      <div
        v-if="scope === 'Project'"
        class="instructions-level"
        data-test="instructions-level-project"
      >
        <header class="instructions-level__header">
          <h3>{{ t("instructions.project") }}</h3>
          <span class="instructions-level__badge" data-test="badge-project-editable">
            {{ t("instructions.editable") }}
          </span>
        </header>
        <KxTextarea
          v-model="projectText"
          :placeholder="t('instructions.projectPlaceholder')"
          rows="6"
          variant="mono"
          data-test="project-instructions"
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
        <KxTextarea
          :model-value="effectiveInstructions"
          readonly
          rows="12"
          variant="preview"
          data-test="effective-instructions"
        />
      </div>
    </div>

    <SettingsState v-else tone="loading" data-test="instructions-loading">
      {{ t("common.loading") }}
    </SettingsState>
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
</style>
