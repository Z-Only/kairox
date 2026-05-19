<script setup lang="ts">
const { t } = useI18n();

withDefaults(
  defineProps<{
    contextWindow: string;
    outputLimit: string;
    temperature: string;
    topP: string;
    topK: string;
    maxTokens: string;
    idPrefix: string;
    open: boolean;
  }>(),
  {
    contextWindow: "",
    outputLimit: "",
    temperature: "",
    topP: "",
    topK: "",
    maxTokens: ""
  }
);

const emit = defineEmits<{
  (e: "update:contextWindow", v: string): void;
  (e: "update:outputLimit", v: string): void;
  (e: "update:temperature", v: string): void;
  (e: "update:topP", v: string): void;
  (e: "update:topK", v: string): void;
  (e: "update:maxTokens", v: string): void;
  (e: "toggle"): void;
}>();
</script>

<template>
  <fieldset class="model-form__section">
    <legend>
      <button type="button" class="model-form__toggle" @click="emit('toggle')">
        {{ open ? "▾" : "▸" }} {{ t("models.advancedOptions") }}
      </button>
    </legend>
    <div v-if="open" class="model-form__grid model-form__grid--3col">
      <KxFormField :label="t('models.contextWindow')">
        <KxInput
          :id="`${idPrefix}-ctx`"
          :model-value="contextWindow"
          type="number"
          :data-test="`${idPrefix}-ctx`"
          @update:model-value="emit('update:contextWindow', String($event))"
        />
      </KxFormField>
      <KxFormField :label="t('models.outputLimit')">
        <KxInput
          :id="`${idPrefix}-out`"
          :model-value="outputLimit"
          type="number"
          :data-test="`${idPrefix}-out`"
          @update:model-value="emit('update:outputLimit', String($event))"
        />
      </KxFormField>
      <KxFormField :label="t('models.temperature')">
        <KxInput
          :id="`${idPrefix}-temp`"
          :model-value="temperature"
          type="number"
          step="0.1"
          min="0"
          max="2"
          :data-test="`${idPrefix}-temp`"
          @update:model-value="emit('update:temperature', String($event))"
        />
      </KxFormField>
      <KxFormField :label="t('models.topP')">
        <KxInput
          :id="`${idPrefix}-top-p`"
          :model-value="topP"
          type="number"
          step="0.1"
          min="0"
          max="1"
          :data-test="`${idPrefix}-top-p`"
          @update:model-value="emit('update:topP', String($event))"
        />
      </KxFormField>
      <KxFormField :label="t('models.topK')">
        <KxInput
          :id="`${idPrefix}-top-k`"
          :model-value="topK"
          type="number"
          min="0"
          :data-test="`${idPrefix}-top-k`"
          @update:model-value="emit('update:topK', String($event))"
        />
      </KxFormField>
      <KxFormField :label="t('models.maxTokens')">
        <KxInput
          :id="`${idPrefix}-max-tokens`"
          :model-value="maxTokens"
          type="number"
          :data-test="`${idPrefix}-max-tokens`"
          @update:model-value="emit('update:maxTokens', String($event))"
        />
      </KxFormField>
    </div>
  </fieldset>
</template>

<style scoped>
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

.model-form__toggle {
  all: unset;
  cursor: pointer;
  font-weight: 600;
  font-size: 0.9rem;
  color: var(--app-text-color-2);
}

.model-form__toggle:hover {
  color: var(--color-text);
}

.model-form__toggle:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  border-radius: 2px;
}

.model-form__grid {
  display: grid;
  gap: 8px;
}

.model-form__grid--3col {
  grid-template-columns: 1fr 1fr 1fr;
}
</style>
