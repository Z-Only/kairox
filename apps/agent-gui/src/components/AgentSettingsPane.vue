<script setup lang="ts">
import { useAgentSettingsStore } from "@/stores/agentSettings";
import ModalDialog from "@/components/ui/ModalDialog.vue";
import SettingsItemMeta from "@/components/ui/SettingsItemMeta.vue";
import SettingsItemSummary from "@/components/ui/SettingsItemSummary.vue";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";
import type {
  AgentSettingsInput,
  AgentSettingsScope,
  AgentSettingsView
} from "@/generated/commands";

const store = useAgentSettingsStore();
const { t } = useI18n();
const configSource = inject<Ref<"user" | "project">>("configSource");

const selectedAgentId = ref<string | null>(null);
const editorDialogOpen = ref(false);
const form = reactive<AgentSettingsInput>({
  scope: "User",
  name: "",
  description: "",
  tools: [],
  modelProfile: null,
  permissionMode: null,
  skills: [],
  nicknameCandidates: [],
  enabled: true,
  instructions: ""
});
const toolsText = ref("");
const skillsText = ref("");
const nicknamesText = ref("");

const selectedScope = computed<AgentSettingsScope>(() =>
  configSource?.value === "project" ? "Project" : "User"
);

const canSave = computed(() => form.name.trim().length > 0 && form.description.trim().length > 0);

function splitCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function scopeLabel(scope: AgentSettingsScope | "Builtin" | "Local"): string {
  if (scope === "Builtin") return t("agents.scopeBuiltin");
  if (scope === "Project") return t("agents.scopeProject");
  if (scope === "Local") return t("agents.scopeLocal");
  return t("agents.scopeUser");
}

function startCreate(): void {
  selectedAgentId.value = null;
  editorDialogOpen.value = true;
  Object.assign(form, {
    scope: selectedScope.value,
    name: "",
    description: "",
    tools: [],
    modelProfile: null,
    permissionMode: null,
    skills: [],
    nicknameCandidates: [],
    enabled: true,
    instructions: ""
  });
  toolsText.value = "";
  skillsText.value = "";
  nicknamesText.value = "";
}

function editAgent(agent: AgentSettingsView): void {
  selectedAgentId.value = agent.settingsId;
  editorDialogOpen.value = true;
  Object.assign(form, {
    scope: agent.scope === "Builtin" ? selectedScope.value : agent.scope,
    name: agent.name,
    description: agent.description,
    tools: [...agent.tools],
    modelProfile: agent.modelProfile,
    permissionMode: agent.permissionMode,
    skills: [...agent.skills],
    nicknameCandidates: [...agent.nicknameCandidates],
    enabled: agent.enabled,
    instructions: agent.instructions
  });
  toolsText.value = agent.tools.join(", ");
  skillsText.value = agent.skills.join(", ");
  nicknamesText.value = agent.nicknameCandidates.join(", ");
}

function closeEditor(): void {
  editorDialogOpen.value = false;
  selectedAgentId.value = null;
}

async function saveAgent(): Promise<void> {
  if (!canSave.value) return;
  await store.saveAgent({
    ...form,
    name: form.name.trim(),
    description: form.description.trim(),
    tools: splitCsv(toolsText.value),
    skills: splitCsv(skillsText.value),
    nicknameCandidates: splitCsv(nicknamesText.value),
    modelProfile: form.modelProfile?.trim() || null,
    permissionMode: form.permissionMode?.trim() || null,
    instructions: form.instructions.trimEnd()
  });
  closeEditor();
}

async function copyToUser(agent: AgentSettingsView): Promise<void> {
  await store.copyAgent(agent.settingsId, "User");
}

async function deleteAgent(agent: AgentSettingsView): Promise<void> {
  await store.deleteAgent(agent.settingsId);
}

onMounted(() => {
  void store.loadAgents();
});

watch(
  () => selectedScope.value,
  (scope) => {
    if (!selectedAgentId.value) form.scope = scope;
  }
);
</script>

<template>
  <section class="agent-settings" :aria-label="t('agents.title')" data-test="agent-settings-pane">
    <SettingsState v-if="store.error" tone="error" data-test="agent-error">
      {{ store.error }}
    </SettingsState>

    <SettingsToolbar :aria-label="t('agents.title')">
      <KxToolbarAction variant="primary" data-test="agent-new" @click="startCreate">
        {{ t("agents.newAgent") }}
      </KxToolbarAction>
      <KxToolbarAction data-test="agent-open-dir" @click="store.openAgentsDir()">
        {{ t("agents.openFolder") }}
      </KxToolbarAction>
      <KxToolbarAction
        :disabled="store.loading"
        data-test="agent-refresh"
        @click="store.loadAgents()"
      >
        {{ store.loading ? t("agents.refreshing") : t("common.refresh") }}
      </KxToolbarAction>
    </SettingsToolbar>

    <SettingsState v-if="store.loading" tone="loading" data-test="agent-loading-state">
      {{ t("agents.loading") }}
    </SettingsState>
    <SettingsState v-else-if="store.agents.length === 0" tone="empty" data-test="agent-empty-state">
      {{ t("agents.empty") }}
    </SettingsState>

    <SettingsCardList v-else :aria-label="t('agents.title')" data-test="agent-list">
      <SettingsCardItem
        v-for="agent in store.agents"
        :key="agent.settingsId"
        class="agent-row"
        :data-test="`agent-row-${slugify(agent.name)}`"
        :data-agent-settings-id="agent.settingsId"
        :data-agent-scope="agent.scope"
        :actions-label="t('agents.title')"
      >
        <SettingsItemSummary :title="agent.name" :description="agent.description">
          <template #tags>
            <SettingsStatusTag>{{ scopeLabel(agent.scope) }}</SettingsStatusTag>
            <SettingsStatusTag :tone="agent.enabled ? 'success' : 'warning'">
              {{ agent.enabled ? t("agents.enabled") : t("agents.disabled") }}
            </SettingsStatusTag>
            <SettingsStatusTag :tone="agent.effective ? 'success' : 'warning'">
              {{
                agent.effective
                  ? t("agents.effective")
                  : t("agents.shadowedBy", { source: agent.shadowedBy })
              }}
            </SettingsStatusTag>
            <SettingsStatusTag :tone="agent.valid ? 'success' : 'error'">
              {{ agent.valid ? t("agents.valid") : t("agents.invalid") }}
            </SettingsStatusTag>
          </template>

          <SettingsItemMeta wrap-values>
            <div>
              <dt>{{ t("agents.model") }}</dt>
              <dd>{{ agent.modelProfile || t("agents.defaultValue") }}</dd>
            </div>
            <div>
              <dt>{{ t("agents.permission") }}</dt>
              <dd>{{ agent.permissionMode || t("agents.defaultValue") }}</dd>
            </div>
            <div>
              <dt>{{ t("agents.tools") }}</dt>
              <dd>
                {{ agent.tools.length ? agent.tools.join(", ") : t("agents.defaultValue") }}
              </dd>
            </div>
            <div>
              <dt>{{ t("agents.path") }}</dt>
              <dd>{{ agent.path }}</dd>
            </div>
          </SettingsItemMeta>
          <KxInlineAlert v-if="agent.validationError" tone="error" compact>
            {{ agent.validationError }}
          </KxInlineAlert>
        </SettingsItemSummary>

        <template #actions>
          <KxInlineAction
            :data-test="`agent-edit-${slugify(agent.name)}`"
            @click="editAgent(agent)"
          >
            {{ agent.editable ? t("common.edit") : t("agents.view") }}
          </KxInlineAction>
          <KxInlineAction
            v-if="!agent.editable"
            :data-test="`agent-copy-${slugify(agent.name)}`"
            @click="copyToUser(agent)"
          >
            {{ t("agents.copyToUser") }}
          </KxInlineAction>
          <KxInlineAction
            v-if="agent.deletable"
            variant="danger"
            :data-test="`agent-delete-${slugify(agent.name)}`"
            @click="deleteAgent(agent)"
          >
            {{ t("common.delete") }}
          </KxInlineAction>
        </template>
      </SettingsCardItem>
    </SettingsCardList>

    <ModalDialog
      :open="editorDialogOpen"
      :title="selectedAgentId ? t('agents.editAgent') : t('agents.newAgent')"
      :description="scopeLabel(form.scope)"
      data-test="agent-editor-dialog"
      @close="closeEditor"
    >
      <form class="agent-editor" data-test="agent-editor" @submit.prevent="saveAgent">
        <KxFormField :label="t('agents.name')">
          <KxInput
            v-model="form.name"
            data-test="agent-form-name"
            :placeholder="t('agents.namePlaceholder')"
          />
        </KxFormField>
        <KxFormField :label="t('agents.description')">
          <KxInput v-model="form.description" data-test="agent-form-description" />
        </KxFormField>
        <KxFormField :label="t('agents.modelProfile')">
          <KxInput
            v-model="form.modelProfile"
            data-test="agent-form-model"
            :placeholder="t('agents.defaultValue')"
          />
        </KxFormField>
        <KxFormField :label="t('agents.permissionMode')">
          <KxInput
            v-model="form.permissionMode"
            data-test="agent-form-permission"
            :placeholder="t('agents.defaultValue')"
          />
        </KxFormField>
        <KxFormField :label="t('agents.tools')">
          <KxInput
            v-model="toolsText"
            data-test="agent-form-tools"
            :placeholder="t('agents.toolsPlaceholder')"
          />
        </KxFormField>
        <KxFormField :label="t('agents.skills')">
          <KxInput
            v-model="skillsText"
            data-test="agent-form-skills"
            :placeholder="t('agents.skillsPlaceholder')"
          />
        </KxFormField>
        <KxFormField :label="t('agents.nicknames')">
          <KxInput
            v-model="nicknamesText"
            data-test="agent-form-nicknames"
            :placeholder="t('agents.nicknamesPlaceholder')"
          />
        </KxFormField>
        <label class="agent-editor__checkbox">
          <input v-model="form.enabled" type="checkbox" data-test="agent-form-enabled" />
          {{ t("agents.enabled") }}
        </label>
        <KxFormField :label="t('settings.instructions')">
          <KxTextarea
            v-model="form.instructions"
            data-test="agent-form-instructions"
            rows="8"
            variant="mono"
          />
        </KxFormField>
      </form>

      <template #footer>
        <KxButton data-test="agent-cancel" @click="closeEditor">
          {{ t("common.cancel") }}
        </KxButton>
        <KxButton
          variant="primary"
          :disabled="!canSave || store.saving"
          data-test="agent-save"
          @click="saveAgent"
        >
          {{ store.saving ? t("agents.saving") : t("agents.saveAgent") }}
        </KxButton>
      </template>
    </ModalDialog>
  </section>
</template>

<style scoped>
.agent-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
}

.agent-editor {
  display: grid;
  align-content: start;
  gap: 10px;
  min-width: 0;
}

.agent-editor__checkbox {
  display: flex !important;
  grid-template-columns: none;
  align-items: center;
}

.agent-editor__checkbox input {
  width: auto;
}
</style>
