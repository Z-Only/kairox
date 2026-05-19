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
</style>
