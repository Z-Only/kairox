<script setup lang="ts">
import { ref, computed, watch } from "vue";
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
  (e: "close"): void;
}>();

const registry = useCommandRegistry();

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
</script>

<template>
  <div
    v-if="visible && displayedItems.length > 0"
    class="command-palette"
    data-test="command-palette"
    @keydown="handleKeydown"
  >
    <div class="command-palette__header">Commands & Skills</div>
    <div
      v-for="(item, i) in displayedItems"
      :key="item.kind === 'command' ? item.command.id : `skill-${item.skillId}`"
      class="command-palette__item"
      :class="{ 'command-palette__item--selected': i === selectedIndex }"
      :data-test="`palette-item-${item.kind === 'command' ? item.command.id : item.skillId}`"
      @click="selectItem(i)"
      @mouseenter="selectedIndex = i"
    >
      <span class="command-palette__label">
        {{ item.kind === "command" ? item.command.label : `/skills ${item.displayName}` }}
      </span>
      <span class="command-palette__desc">
        {{ item.kind === "command" ? item.command.description : "Run skill" }}
      </span>
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
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  max-height: 320px;
  overflow-y: auto;
  z-index: 100;
}

.command-palette__header {
  padding: 6px 12px;
  font-size: 11px;
  color: var(--app-text-color-2);
  text-transform: uppercase;
  letter-spacing: 0.05em;
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
</style>
