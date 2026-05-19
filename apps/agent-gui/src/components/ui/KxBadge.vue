<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: "neutral" | "success" | "warning" | "error" | "info" | "muted";
    size?: "sm" | "md";
    dataTest?: string;
  }>(),
  {
    tone: "neutral",
    size: "sm",
    dataTest: undefined
  }
);
</script>

<template>
  <KxTag class="kx-badge" :tone="tone" :size="size" :data-test="dataTest">
    <slot />
  </KxTag>
</template>

<style scoped>
.kx-badge {
  font-weight: 650;
}
</style>
