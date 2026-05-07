<script setup lang="ts">
import MarketplacePane from "@/components/MarketplacePane.vue";
</script>

<template>
  <NCard class="marketplace" :bordered="false" content-style="padding: 16px;">
    <MarketplacePane />
  </NCard>
</template>

<style scoped>
.marketplace {
  height: 100%;
  overflow-y: auto;
}
</style>
