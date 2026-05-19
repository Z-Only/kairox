<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useMentionSearch } from "@/composables/useMentionSearch";

const props = withDefaults(
  defineProps<{
    visible: boolean;
    filterText: string;
    workspacePath: string;
  }>(),
  {
    visible: false,
    filterText: "",
    workspacePath: ""
  }
);

const emit = defineEmits<{
  (e: "select-file", path: string): void;
  (e: "close"): void;
}>();

const mention = useMentionSearch();
const { t } = useI18n();

const paletteEl = ref<HTMLElement | null>(null);
const selectedIndex = ref(0);
const filesLoaded = ref(false);

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
    if (v) {
      selectedIndex.value = 0;
      if (!filesLoaded.value && props.workspacePath) {
        mention.loadFiles(props.workspacePath);
        filesLoaded.value = true;
      } else if (!props.workspacePath) {
        mention.fileList.value = [];
        filesLoaded.value = false;
      }
    }
  }
);

watch(
  () => props.workspacePath,
  (newPath) => {
    if (newPath) {
      mention.loadFiles(newPath);
      filesLoaded.value = true;
    } else {
      mention.fileList.value = [];
      filesLoaded.value = false;
    }
  }
);

const displayedFiles = computed(() => mention.matchingFiles());
const hasWorkspace = computed(() => props.workspacePath.trim().length > 0);
const showLoading = computed(
  () => hasWorkspace.value && !mention.loaded.value && displayedFiles.value.length === 0
);
const emptyMessage = computed(() =>
  hasWorkspace.value ? t("chat.fileMentionNoMatches") : t("chat.fileMentionNoWorkspace")
);

interface HighlightSegment {
  text: string;
  match: boolean;
}

function highlightSegments(path: string, filter: string): HighlightSegment[] {
  const segments: HighlightSegment[] = [];
  if (!filter) {
    segments.push({ text: path, match: false });
    return segments;
  }
  const lower = path.toLowerCase();
  const q = filter.toLowerCase();
  let qi = 0;
  let buf = "";
  for (let i = 0; i < path.length; i++) {
    if (qi < q.length && lower[i] === q[qi]) {
      if (buf) {
        segments.push({ text: buf, match: false });
        buf = "";
      }
      segments.push({ text: path[i], match: true });
      qi++;
    } else {
      buf += path[i];
    }
  }
  if (buf) {
    segments.push({ text: buf, match: false });
  }
  return segments;
}

watch(selectedIndex, async () => {
  await nextTick();
  const el = paletteEl.value?.querySelector(".file-mention-palette__item--selected");
  el?.scrollIntoView?.({ block: "nearest" });
});

function selectFile(index: number) {
  const path = displayedFiles.value[index];
  if (path) {
    emit("select-file", path);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (displayedFiles.value.length === 0) return;
    selectedIndex.value = Math.min(selectedIndex.value + 1, displayedFiles.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (displayedFiles.value.length === 0) return;
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === "Enter") {
    e.preventDefault();
    selectFile(selectedIndex.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}

defineExpose({ handleKeydown });
</script>

<template>
  <div
    ref="paletteEl"
    v-if="visible"
    class="kx-popover-content kx-popover-content--palette file-mention-palette"
    data-test="file-mention-palette"
    @keydown="handleKeydown"
  >
    <div class="kx-popover-panel__header file-mention-palette__header">
      {{ t("chat.fileMentionHeader") }}
    </div>
    <KxAsyncState
      v-if="showLoading"
      class="file-mention-palette__empty"
      tone="loading"
      density="popover"
      data-test="file-mention-loading"
    >
      {{ t("common.loading") }}
    </KxAsyncState>
    <KxEmptyState
      v-else-if="displayedFiles.length === 0"
      class="file-mention-palette__empty"
      density="popover"
      data-test="file-mention-empty"
    >
      {{ emptyMessage }}
    </KxEmptyState>
    <template v-else>
      <div
        v-for="(path, i) in displayedFiles"
        :key="path"
        class="kx-popover-option file-mention-palette__item"
        :class="{
          'file-mention-palette__item--selected': i === selectedIndex,
          'kx-popover-option--selected': i === selectedIndex
        }"
        data-test="mention-file-item"
        @click="selectFile(i)"
        @mouseenter="selectedIndex = i"
      >
        <span class="kx-popover-option__label file-mention-palette__label"
          >@<template v-for="(seg, si) in highlightSegments(path, props.filterText)" :key="si">
            <mark v-if="seg.match" class="kx-popover-mark file-mention-palette__match">
              {{ seg.text }}
            </mark>
            <template v-else>{{ seg.text }}</template>
          </template></span
        >
      </div>
    </template>
  </div>
</template>

<style scoped>
.file-mention-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 4px;
}

.file-mention-palette__item {
  align-items: center;
}

.file-mention-palette__label {
  font-family: monospace;
}

.file-mention-palette__match {
  font-weight: 650;
}
</style>
