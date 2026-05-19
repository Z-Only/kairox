<script setup lang="ts">
type SettingsStateTone = "empty" | "loading" | "info" | "success" | "warning" | "error";

const props = withDefaults(
  defineProps<{
    tone?: SettingsStateTone;
    role?: string;
    dataTest?: string;
    compact?: boolean;
  }>(),
  {
    tone: "empty",
    role: undefined,
    dataTest: undefined,
    compact: undefined
  }
);

const resolvedCompact = computed(() => props.compact ?? props.tone !== "empty");
</script>

<template>
  <KxStateBlock
    :class="['settings-state', `settings-state--${tone}`]"
    :tone="tone"
    :role="role"
    :data-test="dataTest"
    :compact="resolvedCompact"
  >
    <span class="settings-state__message">
      <slot />
    </span>
    <div v-if="$slots.actions" class="settings-state__actions">
      <slot name="actions" />
    </div>
  </KxStateBlock>
</template>

<style scoped>
.settings-state {
  width: 100%;
}

.settings-state__message {
  min-width: 0;
}

.settings-state__actions {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--app-space-2);
  flex-wrap: wrap;
}

.settings-state--empty {
  margin-block: var(--app-space-1);
}

.settings-state--loading .settings-state__message {
  color: var(--app-text-color-2);
}

@media (max-width: 640px) {
  .settings-state__actions {
    width: 100%;
  }
}
</style>
