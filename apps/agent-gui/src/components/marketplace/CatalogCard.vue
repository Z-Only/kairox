<script setup lang="ts">
import type { ServerEntryResponse } from "../../generated/commands";

const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ click: [] }>();

// Map catalog trust level to tag CSS modifier class.
const trustTagClass = computed<string>(() => {
  if (props.entry.trust === "verified") return "tag-success";
  if (props.entry.trust === "community") return "tag-warning";
  return "";
});
</script>

<template>
  <button type="button" class="card-button" data-test="catalog-card" @click="emit('click')">
    <div class="card catalog-card">
      <div class="card-body">
        <div class="card-head">
          <span class="icon">{{ entry.icon || "🔌" }}</span>
          <span class="display-name">{{ entry.display_name }}</span>
          <span class="tag trust-tag" :class="trustTagClass">
            {{ entry.trust }}
          </span>
        </div>
        <span class="summary">{{ entry.summary }}</span>
        <div class="tags">
          <span v-for="t in entry.tags" :key="t" class="tag">
            {{ t }}
          </span>
        </div>
      </div>
    </div>
  </button>
</template>

<style scoped>
.card-button {
  all: unset;
  display: block;
  width: 100%;
  cursor: pointer;
  text-align: left;
}

.card-button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  border-radius: 6px;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.display-name {
  font-weight: 600;
}

.trust-tag {
  margin-left: auto;
}

.summary {
  font-size: 13px;
  display: block;
  color: var(--app-text-color-2);
}

.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
</style>
