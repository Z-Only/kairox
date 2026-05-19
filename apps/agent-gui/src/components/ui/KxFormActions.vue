<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    align?: "start" | "end";
    dataTest?: string;
  }>(),
  {
    align: "start",
    dataTest: undefined
  }
);
</script>

<template>
  <div :class="['kx-form-actions', `kx-form-actions--${props.align}`]" :data-test="props.dataTest">
    <slot />
  </div>
</template>

<style scoped>
.kx-form-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--app-space-2);
  margin-top: var(--app-space-1);
}

.kx-form-actions--end {
  justify-content: flex-end;
}
</style>
