<script setup lang="ts">
type EmptyStateDensity = "section" | "inline" | "popover";

const props = withDefaults(
  defineProps<{
    title?: string;
    description?: string;
    role?: string;
    dataTest?: string;
    compact?: boolean;
    density?: EmptyStateDensity;
  }>(),
  {
    title: undefined,
    description: undefined,
    role: undefined,
    dataTest: undefined,
    compact: false,
    density: "section"
  }
);
</script>

<template>
  <KxAsyncState
    :class="['kx-empty-state', `kx-empty-state--${props.density}`]"
    tone="empty"
    :title="props.title"
    :description="props.description"
    :role="props.role"
    :data-test="props.dataTest"
    :compact="props.compact"
    :density="props.density"
  >
    <slot />
    <template v-if="$slots.icon" #icon>
      <slot name="icon" />
    </template>
    <template v-if="$slots.actions" #actions>
      <slot name="actions" />
    </template>
  </KxAsyncState>
</template>
