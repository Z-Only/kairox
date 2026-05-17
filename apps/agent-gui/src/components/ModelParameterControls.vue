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
      <label>
        <span>{{ t("models.contextWindow") }}</span>
        <input
          :id="`${idPrefix}-ctx`"
          :value="contextWindow"
          type="number"
          :data-test="`${idPrefix}-ctx`"
          @input="emit('update:contextWindow', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <label>
        <span>{{ t("models.outputLimit") }}</span>
        <input
          :id="`${idPrefix}-out`"
          :value="outputLimit"
          type="number"
          :data-test="`${idPrefix}-out`"
          @input="emit('update:outputLimit', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <label>
        <span>{{ t("models.temperature") }}</span>
        <input
          :id="`${idPrefix}-temp`"
          :value="temperature"
          type="number"
          step="0.1"
          min="0"
          max="2"
          :data-test="`${idPrefix}-temp`"
          @input="emit('update:temperature', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <label>
        <span>{{ t("models.topP") }}</span>
        <input
          :id="`${idPrefix}-top-p`"
          :value="topP"
          type="number"
          step="0.1"
          min="0"
          max="1"
          :data-test="`${idPrefix}-top-p`"
          @input="emit('update:topP', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <label>
        <span>{{ t("models.topK") }}</span>
        <input
          :id="`${idPrefix}-top-k`"
          :value="topK"
          type="number"
          min="0"
          :data-test="`${idPrefix}-top-k`"
          @input="emit('update:topK', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <label>
        <span>{{ t("models.maxTokens") }}</span>
        <input
          :id="`${idPrefix}-max-tokens`"
          :value="maxTokens"
          type="number"
          :data-test="`${idPrefix}-max-tokens`"
          @input="emit('update:maxTokens', ($event.target as HTMLInputElement).value)"
        />
      </label>
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

.model-form__section label,
label {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

label > span {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--app-text-color-2);
}

input {
  padding: 6px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  font-size: 0.85rem;
}

input:focus {
  border-color: var(--app-primary-color);
  outline: none;
}

input:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
