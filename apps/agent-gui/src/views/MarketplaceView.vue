<script setup lang="ts">
import MarketplacePane from "@/components/MarketplacePane.vue";
</script>

<template>
  <div class="marketplace card">
    <MarketplacePane />
  </div>
</template>

<style scoped>
.marketplace {
  height: 100%;
  overflow-y: auto;
  padding: 16px;
}
</style>
