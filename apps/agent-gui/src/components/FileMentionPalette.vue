<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useMentionSearch } from "@/composables/useMentionSearch";

const props = withDefaults(
  defineProps<{
    visible: boolean;
    filterText: string;
  }>(),
  {
    visible: false,
    filterText: ""
  }
);

const emit = defineEmits<{
  (e: "select-file", path: string): void;
  (e: "close"): void;
}>();

const mention = useMentionSearch();

const selectedIndex = ref(0);

mention.setFilter(props.filterText);

watch(
  () => props.filterText,
  () => {
    mention.setFilter(props.filterText);
    selectedIndex.value = 0;
  }
);

watch(
  () => props.visible,
  (v) => {
    if (v) selectedIndex.value = 0;
  }
);

const displayedFiles = computed(() => mention.matchingFiles());

function selectFile(index: number) {
  const path = displayedFiles.value[index];
  if (path) {
    emit("select-file", path);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, displayedFiles.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === "Enter") {
    e.preventDefault();
    selectFile(selectedIndex.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}
</script>

<template>
  <div
    v-if="visible && displayedFiles.length > 0"
    class="file-mention-palette"
    data-test="file-mention-palette"
    @keydown="handleKeydown"
  >
    <div class="file-mention-palette__header">Files</div>
    <div
      v-for="(path, i) in displayedFiles"
      :key="path"
      class="file-mention-palette__item"
      :class="{ 'file-mention-palette__item--selected': i === selectedIndex }"
      data-test="mention-file-item"
      @click="selectFile(i)"
      @mouseenter="selectedIndex = i"
    >
      <span class="file-mention-palette__label">@{{ path }}</span>
    </div>
  </div>
</template>

<style scoped>
.file-mention-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 4px;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  max-height: 320px;
  overflow-y: auto;
  z-index: 100;
}

.file-mention-palette__header {
  padding: 6px 12px;
  font-size: 11px;
  color: var(--app-text-color-2);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  border-bottom: 1px solid var(--app-border-color);
}

.file-mention-palette__item {
  padding: 8px 12px;
  cursor: pointer;
}

.file-mention-palette__item--selected {
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}

.file-mention-palette__label {
  font-size: 13px;
  font-family: monospace;
}
</style>
