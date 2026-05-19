<script setup lang="ts">
withDefaults(
  defineProps<{
    label: string;
    description?: string;
    required?: boolean;
    dataTest?: string;
  }>(),
  {
    description: "",
    required: false,
    dataTest: undefined
  }
);
</script>

<template>
  <label class="kx-form-field" :data-test="dataTest">
    <span class="kx-form-field__label">
      {{ label }}<span v-if="required" class="kx-form-field__required" aria-hidden="true"> *</span>
    </span>
    <slot />
    <span v-if="description" class="kx-form-field__description">
      {{ description }}
    </span>
  </label>
</template>

<style scoped>
.kx-form-field {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: var(--app-space-1);
}

.kx-form-field__label {
  color: var(--app-text-color-2);
  font-size: var(--app-text-sm);
  font-weight: 600;
}

.kx-form-field__description {
  color: var(--app-text-color-3);
  font-size: var(--app-text-xs);
  line-height: 1.4;
}

.kx-form-field :deep(.kx-form-control) {
  box-sizing: border-box;
  width: 100%;
  min-height: 34px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md);
  background: var(--app-card-color);
  color: var(--app-text-color);
  font: inherit;
  font-size: var(--app-text-sm);
}

.kx-form-field :deep(.kx-form-control:focus) {
  border-color: var(--app-primary-color);
  outline: none;
}

.kx-form-field :deep(.kx-form-control:focus-visible) {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.kx-form-field :deep(.kx-form-control:disabled),
.kx-form-field :deep(.kx-form-control[readonly]) {
  opacity: 0.68;
}
</style>
