<script setup lang="ts">
import type { ServerEntryResponse } from "../../generated/commands";

defineProps<{ entry: ServerEntryResponse }>();
defineEmits<{ click: [] }>();
</script>

<template>
  <button class="card" data-test="catalog-card" @click="$emit('click')">
    <div class="card__head">
      <span class="icon">{{ entry.icon || "🔌" }}</span>
      <strong>{{ entry.display_name }}</strong>
      <span class="trust" :class="entry.trust">{{ entry.trust }}</span>
    </div>
    <p class="summary">{{ entry.summary }}</p>
    <div class="tags">
      <span v-for="t in entry.tags" :key="t" class="tag">{{ t }}</span>
    </div>
  </button>
</template>

<style scoped>
.card {
  text-align: left;
  padding: 12px;
  border: 1px solid var(--border, #ddd);
  cursor: pointer;
  background: transparent;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.card__head {
  display: flex;
  align-items: center;
  gap: 6px;
}
.trust {
  margin-left: auto;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 3px;
  background: #eee;
}
.trust.verified {
  background: #cfc;
}
.trust.community {
  background: #ffd;
}
.summary {
  font-size: 13px;
  color: var(--muted, #555);
  margin: 0;
}
.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
.tag {
  font-size: 10px;
  padding: 1px 4px;
  border: 1px solid #ddd;
  border-radius: 2px;
}
</style>
