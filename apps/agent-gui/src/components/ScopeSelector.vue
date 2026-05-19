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

function updateScope(value: string | number): void {
  emit("update:modelValue", value as ConfigScope);
}
</script>

<template>
  <fieldset class="scope-selector" aria-label="Install target scope" data-test="scope-selector">
    <KxRadioCard
      v-for="option in options"
      :key="option.value"
      :model-value="modelValue"
      :value="option.value"
      :label="option.label"
      :description="option.description"
      name="scope-selector"
      :data-test="`scope-${option.value.toLowerCase()}`"
      @update:model-value="updateScope"
    />
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
</style>
