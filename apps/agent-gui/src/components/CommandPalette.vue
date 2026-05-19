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

const { t } = useI18n();
const registry = useCommandRegistry(t);

const paletteEl = ref<HTMLElement | null>(null);
const selectedIndex = ref(0);
registry.setFilter(props.filterText);

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
    if (displayedItems.value.length === 0) return;
    selectedIndex.value = Math.min(selectedIndex.value + 1, displayedItems.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (displayedItems.value.length === 0) return;
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

function itemDescription(item: (typeof displayedItems.value)[number]): string {
  if (item.kind === "command") return item.command.description;
  if (item.kind === "skill") return t("chat.commandPaletteRunSkill");
  return t("chat.commandPaletteSwitchModel");
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
    v-if="visible"
    class="kx-popover-content kx-popover-content--palette command-palette"
    data-test="command-palette"
    @keydown="handleKeydown"
  >
    <div class="kx-popover-panel__header command-palette__header">
      {{ t("chat.commandPaletteHeader") }}
    </div>
    <KxEmptyState
      v-if="displayedItems.length === 0"
      class="command-palette__empty"
      density="popover"
      data-test="command-palette-empty"
    >
      {{ t("chat.commandPaletteNoMatches") }}
    </KxEmptyState>
    <template v-else>
      <div
        v-for="(item, i) in displayedItems"
        :key="
          item.kind === 'command'
            ? item.command.id
            : item.kind === 'skill'
              ? `skill-${item.skillId}`
              : `model-${item.alias}`
        "
        class="kx-popover-option command-palette__item"
        :class="{
          'command-palette__item--selected': i === selectedIndex,
          'kx-popover-option--selected': i === selectedIndex
        }"
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
          class="kx-popover-option__label command-palette__label"
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
          class="kx-popover-option__meta command-palette__desc"
          v-html="highlightMatch(itemDescription(item))"
        ></span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.command-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 4px;
}

.command-palette__item {
  justify-content: space-between;
  align-items: center;
}

.command-palette__label {
  flex: 0 1 auto;
}

.command-palette__desc {
  flex: 0 0 auto;
  font-size: 12px;
}

.command-palette__label :deep(mark),
.command-palette__desc :deep(mark) {
  font-weight: 700;
}
</style>
