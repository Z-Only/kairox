<script setup lang="ts">
import ModelParameterControls from "@/components/ModelParameterControls.vue";
import ModalDialog from "@/components/ui/ModalDialog.vue";

const props = defineProps<{
  open: boolean;
  mode: "add" | "edit";
  loading: boolean;
  canTest?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  test: [];
}>();

const { t } = useI18n();

const alias = defineModel<string>("alias", { required: true });
const provider = defineModel<string>("provider", { required: true });
const modelId = defineModel<string>("modelId", { required: true });
const contextWindow = defineModel<string>("contextWindow", { required: true });
const outputLimit = defineModel<string>("outputLimit", { required: true });
const temperature = defineModel<string>("temperature", { required: true });
const topP = defineModel<string>("topP", { required: true });
const topK = defineModel<string>("topK", { required: true });
const maxTokens = defineModel<string>("maxTokens", { required: true });
const baseUrl = defineModel<string>("baseUrl", { required: true });
const apiKeyEnv = defineModel<string>("apiKeyEnv", { required: true });
const advancedOpen = defineModel<boolean>("advancedOpen", { required: true });

const idPrefix = computed(() => `model-${props.mode}`);
const isAddMode = computed(() => props.mode === "add");
const title = computed(() => t(isAddMode.value ? "models.addProfile" : "models.editProfile"));
const description = computed(() =>
  t(isAddMode.value ? "models.addProfileDesc" : "models.editProfileDesc")
);
const formTestId = computed(() => (isAddMode.value ? "model-add-form" : "model-edit-form"));
const aliasTestId = computed(() => (isAddMode.value ? "model-form-alias" : "model-edit-alias"));
const providerTestId = computed(() =>
  isAddMode.value ? "model-form-provider" : "model-edit-provider"
);
const modelIdTestId = computed(() =>
  isAddMode.value ? "model-form-model-id" : "model-edit-model-id"
);
const baseUrlTestId = computed(() =>
  isAddMode.value ? "model-form-base-url" : "model-edit-base-url"
);
const apiKeyEnvTestId = computed(() =>
  isAddMode.value ? "model-form-api-key-env" : "model-edit-api-key-env"
);
const saveButtonTestId = computed(() =>
  isAddMode.value ? "model-save-button" : "model-edit-save-button"
);
const testButtonTestId = computed(() =>
  isAddMode.value ? "model-test-form-btn" : "model-edit-test-btn"
);
const saveDisabled = computed(() => {
  if (props.loading || !provider.value.trim() || !modelId.value.trim()) return true;
  return isAddMode.value && !alias.value.trim();
});
const testDisabled = computed(() => (isAddMode.value ? !baseUrl.value.trim() : !props.canTest));
</script>

<template>
  <ModalDialog
    :open="open"
    :title="title"
    :description="description"
    :data-test="isAddMode ? 'model-add-dialog' : 'model-edit-dialog'"
    @close="emit('close')"
  >
    <form class="model-form" :data-test="formTestId" @submit.prevent="emit('save')">
      <fieldset class="model-form__section">
        <legend>{{ t("models.basicOptions") }}</legend>
        <div class="model-form__grid model-form__grid--2col">
          <KxFormField :label="t('models.alias')" :required="isAddMode">
            <input
              :id="`${idPrefix}-alias`"
              v-model="alias"
              class="kx-form-control"
              :data-test="aliasTestId"
              :required="isAddMode"
              :readonly="!isAddMode"
            />
          </KxFormField>
          <KxFormField :label="t('models.provider')" required>
            <input
              :id="`${idPrefix}-provider`"
              v-model="provider"
              class="kx-form-control"
              :data-test="providerTestId"
              required
            />
          </KxFormField>
        </div>
        <KxFormField :label="t('models.modelId')" required>
          <input
            :id="`${idPrefix}-model-id`"
            v-model="modelId"
            class="kx-form-control"
            :data-test="modelIdTestId"
            required
          />
        </KxFormField>
      </fieldset>

      <fieldset class="model-form__section">
        <legend>{{ t("models.connectionOptions") }}</legend>
        <KxFormField :label="t('models.baseUrl')">
          <input
            :id="`${idPrefix}-base-url`"
            v-model="baseUrl"
            class="kx-form-control"
            :data-test="baseUrlTestId"
          />
        </KxFormField>
        <KxFormField :label="t('models.apiKeyEnv')">
          <input
            :id="`${idPrefix}-api-key-env`"
            v-model="apiKeyEnv"
            class="kx-form-control"
            :data-test="apiKeyEnvTestId"
          />
        </KxFormField>
      </fieldset>

      <ModelParameterControls
        :id-prefix="idPrefix"
        :open="advancedOpen"
        :context-window="contextWindow"
        :output-limit="outputLimit"
        :temperature="temperature"
        :top-p="topP"
        :top-k="topK"
        :max-tokens="maxTokens"
        @update:context-window="contextWindow = $event"
        @update:output-limit="outputLimit = $event"
        @update:temperature="temperature = $event"
        @update:top-p="topP = $event"
        @update:top-k="topK = $event"
        @update:max-tokens="maxTokens = $event"
        @toggle="advancedOpen = !advancedOpen"
      />
    </form>

    <template #footer>
      <button class="btn" type="button" @click="emit('close')">
        {{ t("common.cancel") }}
      </button>
      <button
        class="btn btn-sm"
        type="button"
        :disabled="testDisabled"
        :data-test="testButtonTestId"
        @click="emit('test')"
      >
        {{ t("models.testConnectivity") }}
      </button>
      <button
        class="btn btn-primary"
        type="submit"
        :disabled="saveDisabled"
        :data-test="saveButtonTestId"
        @click.prevent="emit('save')"
      >
        {{ loading ? t("models.saving") : t("models.saveProfile") }}
      </button>
    </template>
  </ModalDialog>
</template>

<style scoped>
.model-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.model-form__section {
  border: none;
  padding: 0;
  margin: 0;
}

.model-form__section legend {
  font-weight: 600;
  font-size: 0.9rem;
  margin-bottom: 8px;
  color: var(--app-text-color-2);
  width: 100%;
}

.model-form__grid {
  display: grid;
  gap: 8px;
}

.model-form__grid--2col {
  grid-template-columns: 1fr 1fr;
}

button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
