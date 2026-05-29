<script setup lang="ts">
import KxActionGroup from "./KxActionGroup.vue";

const props = withDefaults(
  defineProps<{
    dataTest?: string;
    role?: string;
    layout?: "split" | "stack";
    density?: "normal" | "compact";
    actionsLabel?: string;
  }>(),
  {
    dataTest: undefined,
    role: "listitem",
    layout: "split",
    density: "normal",
    actionsLabel: undefined
  }
);
</script>

<template>
  <article
    :class="[
      'settings-card-item',
      `settings-card-item--${props.layout}`,
      `settings-card-item--${props.density}`,
      { 'settings-card-item--with-actions': $slots.actions }
    ]"
    :role="props.role"
    :data-test="props.dataTest"
  >
    <template v-if="$slots.actions">
      <div class="settings-card-item__row">
        <div class="settings-card-item__content">
          <slot />
        </div>
        <KxActionGroup
          class="settings-card-item__actions"
          :aria-label="props.actionsLabel"
          align="end"
        >
          <slot name="actions" />
        </KxActionGroup>
      </div>
      <div v-if="$slots.details" class="settings-card-item__details">
        <slot name="details" />
      </div>
    </template>
    <slot v-else />
  </article>
</template>

<style scoped>
.settings-card-item {
  min-width: 0;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-lg);
  padding: 14px;
  background: var(--app-card-color);
  box-shadow: var(--app-shadow-sm);
}

.settings-card-item--compact {
  padding: 11px 12px;
}

.settings-card-item--split:not(.settings-card-item--with-actions) {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.settings-card-item--stack {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.settings-card-item--with-actions {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.settings-card-item__row {
  display: flex;
  min-width: 0;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.settings-card-item--stack .settings-card-item__row {
  flex-direction: column;
}

.settings-card-item__content {
  min-width: 0;
  flex: 1 1 auto;
}

.settings-card-item__actions {
  flex: 0 0 auto;
}

.settings-card-item__details {
  min-width: 0;
}

@media (max-width: 720px) {
  .settings-card-item--split:not(.settings-card-item--with-actions),
  .settings-card-item__row {
    flex-direction: column;
  }

  .settings-card-item__content,
  .settings-card-item__actions {
    width: 100%;
  }
}
</style>
