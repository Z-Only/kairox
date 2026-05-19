<script setup lang="ts">
type AccordionStateTone = "empty" | "loading" | "info" | "success" | "warning" | "error";

const props = withDefaults(
  defineProps<{
    tone?: AccordionStateTone;
    dataTest?: string;
  }>(),
  {
    tone: "empty",
    dataTest: undefined
  }
);
</script>

<template>
  <div class="kx-accordion-state" :data-test="props.dataTest">
    <KxAsyncState :tone="props.tone" compact>
      <slot />
    </KxAsyncState>
  </div>
</template>

<style scoped>
.kx-accordion-state {
  min-width: 0;
}
</style>
