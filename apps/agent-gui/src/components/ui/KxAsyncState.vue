<script setup lang="ts">
type AsyncStateTone = "empty" | "loading" | "info" | "success" | "warning" | "error";
type AsyncStateDensity = "section" | "inline" | "popover";

const props = withDefaults(
  defineProps<{
    tone?: AsyncStateTone;
    title?: string;
    description?: string;
    role?: string;
    dataTest?: string;
    compact?: boolean;
    density?: AsyncStateDensity;
  }>(),
  {
    tone: "empty",
    title: undefined,
    description: undefined,
    role: undefined,
    dataTest: undefined,
    compact: false,
    density: "section"
  }
);

const resolvedCompact = computed(() => props.compact || props.density !== "section");
</script>

<template>
  <KxStateBlock
    :class="[
      'kx-async-state',
      `kx-async-state--${props.tone}`,
      `kx-async-state--${props.density}`,
      { 'kx-popover-empty': props.density === 'popover' }
    ]"
    :tone="props.tone"
    :role="props.role"
    :data-test="props.dataTest"
    :compact="resolvedCompact"
  >
    <span v-if="$slots.icon" class="kx-async-state__icon" aria-hidden="true">
      <slot name="icon" />
    </span>
    <span class="kx-async-state__body">
      <strong v-if="props.title" class="kx-async-state__title">{{ props.title }}</strong>
      <span v-if="$slots.default" class="kx-async-state__message">
        <slot />
      </span>
      <span v-if="props.description" class="kx-async-state__description">
        {{ props.description }}
      </span>
    </span>
    <span v-if="$slots.actions" class="kx-async-state__actions">
      <slot name="actions" />
    </span>
  </KxStateBlock>
</template>

<style scoped>
.kx-async-state {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  max-width: 100%;
}

.kx-async-state.kx-async-state--inline,
.kx-async-state.kx-async-state--popover {
  border-color: transparent;
  background: transparent;
}

.kx-async-state.kx-async-state--inline {
  min-height: 40px;
  padding: var(--app-space-3);
}

.kx-async-state.kx-async-state--popover {
  min-height: 36px;
  padding: var(--app-space-3) var(--app-space-4);
  font-size: var(--app-text-sm);
}

.kx-async-state--section.kx-async-state--empty {
  flex-direction: column;
}

.kx-async-state__body {
  display: inline-flex;
  min-width: 0;
  max-width: 100%;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}

.kx-async-state--loading .kx-async-state__body,
.kx-async-state--error .kx-async-state__body,
.kx-async-state--warning .kx-async-state__body,
.kx-async-state--info .kx-async-state__body,
.kx-async-state--success .kx-async-state__body {
  align-items: flex-start;
}

.kx-async-state__title {
  color: var(--app-text-color);
  font-size: var(--app-text-base);
  font-weight: 650;
}

.kx-async-state__message,
.kx-async-state__description {
  min-width: 0;
  overflow-wrap: anywhere;
}

.kx-async-state__description {
  color: var(--app-text-color-3);
  font-size: var(--app-text-sm);
}

.kx-async-state__actions {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--app-space-2);
  flex-wrap: wrap;
}

.kx-async-state__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
}

@media (max-width: 640px) {
  .kx-async-state__actions {
    width: 100%;
  }
}
</style>
