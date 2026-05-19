<script setup lang="ts">
type RadioValue = string | number;

const props = defineProps<{
  modelValue: RadioValue;
  value: RadioValue;
  label: string;
  description?: string;
  name?: string;
  dataTest?: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: RadioValue];
}>();

const isSelected = computed(() => props.modelValue === props.value);

function onChange(event: Event): void {
  if ((event.target as HTMLInputElement).checked) {
    emit("update:modelValue", props.value);
  }
}
</script>

<template>
  <label
    :class="['kx-radio-card', { 'kx-radio-card--selected': isSelected }]"
    :data-test="dataTest"
  >
    <input
      class="kx-radio-card__input"
      type="radio"
      :name="name"
      :value="value"
      :checked="isSelected"
      :aria-label="label"
      @change="onChange"
    />
    <span class="kx-radio-card__content">
      <span class="kx-radio-card__title">{{ label }}</span>
      <span v-if="description" class="kx-radio-card__description">{{ description }}</span>
    </span>
  </label>
</template>

<style scoped>
.kx-radio-card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md, 6px);
  background: var(--app-card-color);
  cursor: pointer;
  transition:
    border-color 0.15s,
    background-color 0.15s;
}

.kx-radio-card:hover {
  border-color: var(--app-primary-color);
}

.kx-radio-card--selected {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, var(--app-card-color));
}

.kx-radio-card__input {
  flex: none;
  width: 16px;
  height: 16px;
  margin-top: 2px;
  accent-color: var(--app-primary-color);
}

.kx-radio-card__content {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.kx-radio-card__title {
  color: var(--app-text-color);
  font-size: 13px;
  font-weight: 600;
}

.kx-radio-card__description {
  color: var(--app-text-color-2);
  font-size: 12px;
  line-height: 1.35;
}
</style>
