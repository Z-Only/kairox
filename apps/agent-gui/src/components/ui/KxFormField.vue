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
  gap: 2px;
}

.kx-form-field__label {
  color: var(--app-text-color-2);
  font-size: 0.85em;
}

.kx-form-field__description {
  color: var(--app-text-color-3);
  font-size: 0.8em;
  line-height: 1.4;
}
</style>
