<script setup lang="ts">
import type { ConfigScope } from "@/generated/commands";

const props = withDefaults(
  defineProps<{
    modelValue: ConfigScope;
    showLocal?: boolean;
  }>(),
  {
    showLocal: false
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: ConfigScope];
}>();

interface ScopeOption {
  value: ConfigScope;
  label: string;
  description: string;
}

const options = computed<ScopeOption[]>(() => {
  const base: ScopeOption[] = [
    {
      value: "User",
      label: "User (Global)",
      description: "Available across all your projects"
    },
    {
      value: "Project",
      label: "Project",
      description: "Shared with the team via .kairox/"
    }
  ];

  if (props.showLocal) {
    base.push({
      value: "Local",
      label: "Local Override",
      description: "Personal temporary config, not committed to git"
    });
  }

  return base;
});

function onChange(event: Event): void {
  const target = event.target as HTMLInputElement;
  if (target.checked) {
    emit("update:modelValue", target.value as ConfigScope);
  }
}
</script>

<template>
  <fieldset class="scope-selector" aria-label="Install target scope" data-test="scope-selector">
    <div
      v-for="option in options"
      :key="option.value"
      class="scope-selector__option"
      :class="{ 'scope-selector__option--selected': modelValue === option.value }"
      :data-test="`scope-${option.value.toLowerCase()}`"
    >
      <label class="scope-selector__label">
        <input
          type="radio"
          class="scope-selector__input"
          :value="option.value"
          :checked="modelValue === option.value"
          :aria-label="option.label"
          @change="onChange"
        />
        <span class="scope-selector__content">
          <span class="scope-selector__title">{{ option.label }}</span>
          <span class="scope-selector__desc">{{ option.description }}</span>
        </span>
      </label>
    </div>
  </fieldset>
</template>

<style scoped>
.scope-selector {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 0;
  border: none;
  margin: 0;
}

.scope-selector__option {
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  transition:
    border-color 0.15s,
    background-color 0.15s;
}

.scope-selector__option:hover {
  border-color: var(--app-primary-color, #18a058);
}

.scope-selector__option--selected {
  border-color: var(--app-primary-color, #18a058);
  background: color-mix(in srgb, var(--app-primary-color, #18a058) 8%, var(--app-card-color, #fff));
}

.scope-selector__label {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  cursor: pointer;
  margin: 0;
  font-weight: 400;
}

.scope-selector__input {
  flex: none;
  margin-top: 2px;
  accent-color: var(--app-primary-color, #18a058);
  width: 16px;
  height: 16px;
}

.scope-selector__content {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.scope-selector__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--app-text-color, #111827);
}

.scope-selector__desc {
  font-size: 12px;
  color: var(--app-text-color-2, #6b7280);
}
</style>
