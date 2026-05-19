<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    title?: string;
    description?: string;
    role?: string;
    dataTest?: string;
    compact?: boolean;
  }>(),
  {
    title: undefined,
    description: undefined,
    role: undefined,
    dataTest: undefined,
    compact: false
  }
);
</script>

<template>
  <KxAsyncState
    class="kx-empty-state"
    tone="empty"
    :title="props.title"
    :description="props.description"
    :role="props.role"
    :data-test="props.dataTest"
    :compact="props.compact"
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
