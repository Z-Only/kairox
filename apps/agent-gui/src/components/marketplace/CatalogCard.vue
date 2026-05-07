<script setup lang="ts">
import type { ServerEntryResponse } from "../../generated/commands";

const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ click: [] }>();

// Map catalog trust level → NTag `type` so the badge follows the
// surrounding NaiveUI theme palette (verified=success, community=warning,
// unverified=default).
const trustTagType = computed<"success" | "warning" | "default">(() => {
  if (props.entry.trust === "verified") return "success";
  if (props.entry.trust === "community") return "warning";
  return "default";
});
</script>

<template>
  <button
    type="button"
    class="card-button"
    data-test="catalog-card"
    @click="emit('click')"
  >
    <NCard size="small" :bordered="true" class="catalog-card">
      <div class="card-head">
        <span class="icon">{{ entry.icon || "🔌" }}</span>
        <NText strong>{{ entry.display_name }}</NText>
        <NTag
          size="small"
          :type="trustTagType"
          :bordered="false"
          class="trust-tag"
        >
          {{ entry.trust }}
        </NTag>
      </div>
      <NText depth="2" class="summary">{{ entry.summary }}</NText>
      <div class="tags">
        <NTag v-for="t in entry.tags" :key="t" size="small" :bordered="false">
          {{ t }}
        </NTag>
      </div>
    </NCard>
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
  outline: 2px solid var(--n-color-target, #18a058);
  outline-offset: 2px;
  border-radius: 6px;
}
.catalog-card :deep(.n-card__content) {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
}
.trust-tag {
  margin-left: auto;
}
.summary {
  font-size: 13px;
  display: block;
}
.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
</style>
