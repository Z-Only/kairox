<script setup lang="ts">
import { useConfirm } from "@/composables/useConfirm";
import { useMemoryStore } from "@/stores/memory";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const { confirm } = useConfirm();

const memory = useMemoryStore();
const { memories, loading, filter, searchQuery } = storeToRefs(memory);
const session = useSessionStore();

onMounted(() => {
  memory.loadMemories();
});

// Reload memories when switching sessions
watch(
  () => session.currentSessionId,
  () => {
    memory.loadMemories();
  }
);

const scopeOptions = computed<{ label: string; value: string }[]>(() => [
  { label: t("memory.scopeAll"), value: "all" },
  { label: t("memory.scopeSession"), value: "session" },
  { label: t("memory.scopeUser"), value: "user" },
  { label: t("memory.scopeWorkspace"), value: "workspace" }
]);

const scopeIcon: Record<string, string> = {
  session: "📋",
  user: "👤",
  workspace: "🗂️"
};

const scopeTone: Record<string, "neutral" | "info" | "success"> = {
  session: "neutral",
  user: "info",
  workspace: "success"
};

function handleSearchKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    memory.loadMemories();
  }
}

function handleFilterChange(value: typeof filter.value) {
  memory.setMemoryFilter(value);
}

async function promptDelete(id: string, content: string) {
  const preview = content.length > 60 ? `${content.slice(0, 60)}…` : content;
  const confirmed = await confirm({
    title: t("common.confirm"),
    message: `${t("memory.deleteConfirm")}\n\n"${preview}"`,
    confirmText: t("common.delete"),
    cancelText: t("common.cancel"),
    type: "warning"
  });
  if (confirmed) {
    void memory.deleteMemoryItem(id);
  }
}
</script>

<template>
  <div class="memory-browser" data-test="memory-browser">
    <header class="memory-header">
      <h2>{{ t("memory.header") }}</h2>
      <KxIconButton
        class="refresh-btn"
        :label="t('common.refresh')"
        :title="t('common.refresh')"
        data-test="memory-refresh-btn"
        @click="memory.loadMemories()"
      >
        ↻
      </KxIconButton>
    </header>

    <div class="memory-controls">
      <div class="scope-row">
        <KxSelect
          :model-value="filter"
          aria-label="Memory scope"
          data-test="memory-scope-select"
          size="compact"
          @update:model-value="handleFilterChange($event as typeof filter)"
        >
          <option v-for="opt in scopeOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </KxSelect>
      </div>
      <KxInput
        v-model="searchQuery"
        type="text"
        :placeholder="t('memory.searchPlaceholder')"
        data-test="memory-search-input"
        size="compact"
        @keydown="handleSearchKeydown"
      />
    </div>

    <KxAsyncState
      v-if="loading"
      class="memory-panel-state memory-empty"
      tone="loading"
      data-test="memory-loading-state"
      compact
    >
      {{ t("common.loading") }}
    </KxAsyncState>
    <KxEmptyState
      v-else-if="memories.length === 0"
      class="memory-panel-state memory-empty memory-empty-state"
      data-test="memory-empty-state"
      compact
    >
      {{ t("memory.emptyHint") }}
    </KxEmptyState>
    <ul v-else class="memory-list" data-test="memory-list">
      <li v-for="mem in memories" :key="mem.id" class="memory-item" data-test="memory-item">
        <div class="memory-meta">
          <KxTag class="memory-scope" :tone="scopeTone[mem.scope] ?? 'neutral'">
            {{ scopeIcon[mem.scope] || "•" }} {{ mem.scope }}
          </KxTag>
          <KxTag v-if="mem.key" class="memory-key">
            {{ mem.key }}
          </KxTag>
          <KxBadge v-if="!mem.accepted" class="memory-pending" tone="warning"> pending </KxBadge>
        </div>
        <span class="memory-content">{{ mem.content }}</span>
        <div class="memory-actions">
          <KxIconButton
            class="memory-delete-btn"
            :label="t('common.delete')"
            :title="t('common.delete')"
            data-test="memory-delete-btn"
            @click="promptDelete(mem.id, mem.content)"
          >
            🗑️
          </KxIconButton>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.memory-browser {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.memory-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
}
.memory-header h2 {
  margin: 0;
  font-size: 14px;
}
.refresh-btn {
  font-size: 14px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
  color: var(--app-text-color-2, #555);
}
.refresh-btn:hover {
  background: var(--app-hover-color, #f0f4f8);
}
.memory-controls {
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #eee);
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.scope-row {
  display: flex;
  gap: 6px;
  align-items: center;
  width: 100%;
}
.memory-panel-state {
  margin: 12px;
  font-size: 12px;
}
.memory-list {
  list-style: none;
  padding: 0;
  margin: 0;
  overflow-y: auto;
  flex: 1;
}
.memory-item {
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #eee);
  position: relative;
}
.memory-item:hover {
  background: var(--app-hover-color, #f8f8f8);
}
.memory-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
  flex-wrap: wrap;
}
.memory-scope {
  font-weight: 600;
}
.memory-content {
  display: block;
  font-size: 12px;
  line-height: 1.4;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 60px;
  overflow: hidden;
}
.memory-actions {
  position: absolute;
  top: 8px;
  right: 8px;
  display: none;
}
.memory-item:hover .memory-actions {
  display: block;
}
.memory-delete-btn {
  background: none;
  border: none;
  cursor: pointer;
  padding: 2px;
  font-size: 12px;
  line-height: 1;
  border-radius: 50%;
}
.memory-delete-btn:hover {
  background: var(--app-hover-color, #f0f4f8);
}
</style>
