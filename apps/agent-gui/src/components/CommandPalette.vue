<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useCommandRegistry, type CommandDef } from "@/composables/useCommandRegistry";

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
  (e: "select-command", command: CommandDef): void;
  (e: "select-skill", skillId: string): void;
  (e: "select-model-profile", alias: string): void;
  (e: "close"): void;
}>();

const registry = useCommandRegistry();

const paletteEl = ref<HTMLElement | null>(null);
const selectedIndex = ref(0);

watch(
  () => props.filterText,
  () => {
    registry.setFilter(props.filterText);
    selectedIndex.value = 0;
  }
);

watch(
  () => props.visible,
  (v) => {
    if (v) selectedIndex.value = 0;
  }
);

const displayedItems = computed(() => registry.allItems());

watch(selectedIndex, async () => {
  await nextTick();
  const el = paletteEl.value?.querySelector(".command-palette__item--selected");
  el?.scrollIntoView?.({ block: "nearest" });
});

function selectItem(index: number) {
  const item = displayedItems.value[index];
  if (!item) return;
  if (item.kind === "command") {
    if (item.command.handler) {
      void item.command.handler();
      emit("close");
    } else if (item.command.insertText !== undefined) {
      emit("select-command", item.command);
    }
  } else if (item.kind === "skill") {
    emit("select-skill", item.skillId);
    emit("close");
  } else if (item.kind === "model-profile") {
    emit("select-model-profile", item.alias);
    emit("close");
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, displayedItems.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === "Enter") {
    e.preventDefault();
    selectItem(selectedIndex.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}

function highlightMatch(text: string): string {
  const q = props.filterText;
  if (!q) return escapeHtml(text);
  const escaped = escapeHtml(text);
  const escapedPattern = q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return escaped.replace(new RegExp(`(${escapedPattern})`, "gi"), "<mark>$1</mark>");
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

defineExpose({ handleKeydown });
</script>

<template>
  <div
    ref="paletteEl"
    v-if="visible && displayedItems.length > 0"
    class="command-palette"
    data-test="command-palette"
    @keydown="handleKeydown"
  >
    <div class="command-palette__header">Commands, Models & Skills</div>
    <div
      v-for="(item, i) in displayedItems"
      :key="
        item.kind === 'command'
          ? item.command.id
          : item.kind === 'skill'
            ? `skill-${item.skillId}`
            : `model-${item.alias}`
      "
      class="command-palette__item"
      :class="{ 'command-palette__item--selected': i === selectedIndex }"
      :data-test="`palette-item-${
        item.kind === 'command'
          ? item.command.id
          : item.kind === 'skill'
            ? item.skillId
            : item.alias
      }`"
      @click="selectItem(i)"
      @mouseenter="selectedIndex = i"
    >
      <!-- eslint-disable-next-line vue/no-v-html -->
      <span
        class="command-palette__label"
        v-html="
          highlightMatch(
            item.kind === 'command'
              ? item.command.label
              : item.kind === 'skill'
                ? `/skills ${item.displayName}`
                : item.displayName
          )
        "
      ></span>
      <!-- eslint-disable-next-line vue/no-v-html -->
      <span
        class="command-palette__desc"
        v-html="
          highlightMatch(
            item.kind === 'command'
              ? item.command.description
              : item.kind === 'skill'
                ? 'Run skill'
                : 'Switch model'
          )
        "
      ></span>
    </div>
  </div>
</template>

<style scoped>
.command-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 4px;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  box-shadow: var(--app-overlay-shadow);
  max-height: 320px;
  overflow-y: auto;
  z-index: var(--app-z-palette);
}

.command-palette__header {
  padding: 6px 12px;
  font-size: 11px;
  color: var(--app-text-color-2);
  text-transform: uppercase;
  letter-spacing: 0;
  border-bottom: 1px solid var(--app-border-color);
}

.command-palette__item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
}

.command-palette__item--selected {
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}

.command-palette__label {
  font-weight: 600;
  font-size: 13px;
}

.command-palette__desc {
  font-size: 12px;
  color: var(--app-text-color-2);
}

.command-palette__label :deep(mark),
.command-palette__desc :deep(mark) {
  background: color-mix(in srgb, var(--app-primary-color) 25%, transparent);
  color: var(--app-primary-color);
  font-weight: 700;
  border-radius: 2px;
  padding: 0 1px;
}
</style>
