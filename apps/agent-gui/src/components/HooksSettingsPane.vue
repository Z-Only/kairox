<script setup lang="ts">
import type {
  ConfigScope,
  HookSettingsInput,
  HookSettingsView,
  HookTemplateView,
  HooksSettingsView
} from "@/generated/commands";
import { commands } from "@/generated/commands";
import { useProjectStore } from "@/stores/project";

const { t } = useI18n();
const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");
const projectStore = useProjectStore();

const view = ref<HooksSettingsView | null>(null);
const loading = ref(true);
const saving = ref(false);
const errorMsg = ref("");
const formOpen = ref(false);

const form = ref({
  id: "",
  event: "Stop",
  matcher: "*",
  command: "",
  statusMessage: "",
  timeoutSecs: 600,
  enabled: true
});

const events = [
  "SessionStart",
  "UserPromptSubmit",
  "PreToolUse",
  "PermissionRequest",
  "PostToolUse",
  "Stop"
];

const scope = computed<ConfigScope>(() => (configSource?.value === "project" ? "Project" : "User"));
const scopeLabel = computed(() =>
  scope.value === "Project" ? t("settings.projectConfig") : t("settings.userConfig")
);

const projectRoot = computed(() => {
  if (configSource?.value !== "project") return null;
  const projectId = configProjectId?.value;
  if (!projectId) return null;
  return (
    projectStore.activeProjects.find((project) => project.projectId === projectId)?.rootPath ??
    projectId
  );
});

const currentHooks = computed<HookSettingsView[]>(() => {
  if (!view.value) return [];
  return scope.value === "Project" ? view.value.project : view.value.user;
});

function unwrapCommand<T>(result: T | { status: string; data?: T; error?: unknown }): T {
  if (
    typeof result === "object" &&
    result !== null &&
    "status" in result &&
    ((result as { status: string }).status === "ok" ||
      (result as { status: string }).status === "error")
  ) {
    if ((result as { status: string }).status === "error") {
      throw new Error(String((result as { error?: unknown }).error));
    }
    return (result as { data: T }).data;
  }
  return result as T;
}

async function load(): Promise<void> {
  errorMsg.value = "";
  loading.value = true;
  try {
    const result = await commands.getHooksSettings(projectRoot.value);
    view.value = unwrapCommand<HooksSettingsView>(result as unknown as HooksSettingsView);
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    loading.value = false;
  }
}

function editHook(hook: HookSettingsView): void {
  form.value = {
    id: hook.id,
    event: hook.event,
    matcher: hook.matcher ?? "",
    command: hook.command,
    statusMessage: hook.statusMessage ?? "",
    timeoutSecs: hook.timeoutSecs ?? 600,
    enabled: hook.enabled
  };
  formOpen.value = true;
}

function applyTemplate(template: HookTemplateView): void {
  form.value = {
    id: template.id,
    event: template.event,
    matcher: template.matcher ?? "",
    command: template.command,
    statusMessage: template.statusMessage ?? "",
    timeoutSecs: template.timeoutSecs ?? 600,
    enabled: true
  };
  formOpen.value = true;
}

function resetForm(): void {
  form.value = {
    id: "",
    event: "Stop",
    matcher: "*",
    command: "",
    statusMessage: "",
    timeoutSecs: 600,
    enabled: true
  };
}

function openNewHookForm(): void {
  resetForm();
  formOpen.value = true;
}

function closeForm(): void {
  formOpen.value = false;
  resetForm();
}

function buildInput(): HookSettingsInput {
  return {
    scope: scope.value,
    id: form.value.id.trim(),
    event: form.value.event,
    matcher: form.value.matcher.trim() ? form.value.matcher.trim() : null,
    command: form.value.command.trim(),
    statusMessage: form.value.statusMessage.trim() ? form.value.statusMessage.trim() : null,
    timeoutSecs: Number.isFinite(form.value.timeoutSecs) ? Number(form.value.timeoutSecs) : null,
    enabled: form.value.enabled
  };
}

async function saveHook(): Promise<void> {
  errorMsg.value = "";
  saving.value = true;
  try {
    const result = await commands.upsertHookSettings(buildInput(), projectRoot.value);
    unwrapCommand(result);
    await load();
    closeForm();
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    saving.value = false;
  }
}

async function deleteHook(hook: HookSettingsView): Promise<void> {
  errorMsg.value = "";
  saving.value = true;
  try {
    const result = await commands.deleteHookSettings(
      scope.value,
      hook.event,
      hook.id,
      projectRoot.value
    );
    unwrapCommand(result);
    await load();
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    saving.value = false;
  }
}

watch(
  [
    () => configSource?.value,
    () => configProjectId?.value,
    () => projectStore.activeProjects.length
  ],
  () => {
    void load();
  },
  { immediate: true }
);
</script>

<template>
  <section
    class="hooks-pane"
    role="tabpanel"
    aria-label="Hooks settings"
    data-test="hooks-settings-pane"
  >
    <SettingsState v-if="errorMsg" tone="error" data-test="hooks-error">
      {{ errorMsg }}
    </SettingsState>

    <SettingsState v-if="loading" tone="loading" data-test="hooks-loading">
      {{ t("common.loading") }}
    </SettingsState>

    <template v-else>
      <div class="hooks-pane__templates" data-test="hook-templates">
        <button
          v-for="template in view?.templates ?? []"
          :key="template.id"
          class="btn btn-secondary"
          :data-test="`hook-template-${template.id}`"
          type="button"
          @click="applyTemplate(template)"
        >
          {{ template.name }}
        </button>
      </div>

      <div class="hooks-pane__grid">
        <section class="hooks-pane__list">
          <div class="hooks-pane__list-header">
            <h3>{{ t("hooks.scopeHooks", { scope: scopeLabel }) }}</h3>
            <span class="tag">{{ currentHooks.length }}</span>
          </div>

          <SettingsState v-if="currentHooks.length === 0" tone="empty" data-test="hooks-empty">
            {{ t("hooks.empty") }}
          </SettingsState>

          <SettingsCardList
            v-else
            :aria-label="t('hooks.scopeHooks', { scope: scopeLabel })"
            data-test="hooks-list"
            :scroll="false"
            dense
          >
            <SettingsCardItem
              v-for="hook in currentHooks"
              :key="`${hook.event}:${hook.id}`"
              class="hook-row"
              layout="stack"
              :data-test="`hook-row-${hook.id}`"
            >
              <div class="hook-row__main">
                <strong>{{ hook.id }}</strong>
                <span class="tag">{{ hook.event }}</span>
                <span v-if="!hook.enabled" class="tag tag-muted">{{ t("hooks.disabled") }}</span>
              </div>
              <code>{{ hook.command }}</code>
              <div class="hook-row__actions">
                <button
                  class="btn btn-secondary"
                  :data-test="`hook-edit-${hook.id}`"
                  type="button"
                  @click="editHook(hook)"
                >
                  {{ t("common.edit") }}
                </button>
                <button
                  class="btn btn-danger"
                  :data-test="`hook-delete-${hook.id}`"
                  :disabled="saving"
                  type="button"
                  @click="deleteHook(hook)"
                >
                  {{ t("common.delete") }}
                </button>
              </div>
            </SettingsCardItem>
          </SettingsCardList>
        </section>

        <button
          v-if="!formOpen"
          class="btn btn-primary hooks-pane__add"
          data-test="hook-add"
          type="button"
          @click="openNewHookForm"
        >
          {{ t("hooks.add") }}
        </button>

        <form v-else class="hooks-pane__form" data-test="hook-form" @submit.prevent="saveHook">
          <KxFormField :label="t('hooks.id')">
            <KxInput v-model="form.id" data-test="hook-id" required />
          </KxFormField>

          <KxFormField :label="t('hooks.event')">
            <KxSelect v-model="form.event" data-test="hook-event">
              <option v-for="event in events" :key="event" :value="event">{{ event }}</option>
            </KxSelect>
          </KxFormField>

          <KxFormField :label="t('hooks.matcher')">
            <KxInput
              v-model="form.matcher"
              data-test="hook-matcher"
              :placeholder="t('hooks.matcherPlaceholder')"
            />
          </KxFormField>

          <KxFormField :label="t('hooks.command')">
            <KxInput v-model="form.command" data-test="hook-command" required />
          </KxFormField>

          <KxFormField :label="t('hooks.status')">
            <KxInput v-model="form.statusMessage" data-test="hook-status" />
          </KxFormField>

          <KxFormField :label="t('hooks.timeout')">
            <KxInput
              v-model.number="form.timeoutSecs"
              data-test="hook-timeout"
              min="1"
              type="number"
            />
          </KxFormField>

          <label class="hooks-pane__toggle">
            <input v-model="form.enabled" data-test="hook-enabled" type="checkbox" />
            <span>{{ t("hooks.enabled") }}</span>
          </label>

          <KxFormActions align="end">
            <button class="btn" type="button" @click="closeForm">
              {{ t("common.cancel") }}
            </button>
            <button
              class="btn btn-primary"
              data-test="hook-save"
              :disabled="saving"
              type="button"
              @click="saveHook"
            >
              {{ t("common.save") }}
            </button>
          </KxFormActions>
        </form>
      </div>
    </template>
  </section>
</template>

<style scoped>
.hooks-pane {
  padding: 12px 0;
}

.hooks-pane__templates {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}

.hooks-pane__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 340px);
  gap: 16px;
  align-items: start;
}

.hooks-pane__add {
  justify-self: start;
}

.hooks-pane__list,
.hooks-pane__form {
  min-width: 0;
}

.hooks-pane__list-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.hooks-pane__list-header h3 {
  margin: 0;
  font-size: 0.95rem;
}

.hook-row__main,
.hook-row__actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.hook-row code {
  min-width: 0;
  color: var(--app-text-color-2);
  overflow-wrap: anywhere;
}

.tag-muted {
  color: var(--app-text-color-2);
}

.hooks-pane__form {
  display: grid;
  gap: 10px;
}

.hooks-pane__toggle {
  display: flex !important;
  grid-template-columns: none !important;
  flex-direction: row;
  align-items: center;
}

@media (max-width: 760px) {
  .hooks-pane__grid {
    grid-template-columns: 1fr;
  }
}
</style>
