<script setup lang="ts">
import { useDialog, type SelectOption } from "naive-ui";
import { useMemoryStore } from "@/stores/memory";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const dialog = useDialog();

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

const scopeOptions = computed<SelectOption[]>(() => [
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

const scopeTagType: Record<string, "default" | "info" | "success" | "warning"> =
  {
    session: "default",
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

// Promote the destructive confirmation to NaiveUI's `useDialog`. The view
// no longer owns visibility state — `dialog.warning` portals into
// `<NDialogProvider>` (mounted in `AppLayout.vue`), and the positive
// click delegates to `memory.deleteMemoryItem`. ConfirmDialog.vue is
// removed in this same commit since MemoryBrowser was its last consumer.
function promptDelete(id: string, content: string) {
  const preview = content.length > 60 ? `${content.slice(0, 60)}…` : content;
  dialog.warning({
    title: t("common.confirm"),
    // Render the preview on its own line so long content does not push
    // the dialog buttons off-screen.
    content: () =>
      h("div", null, [
        h("p", null, t("memory.deleteConfirm")),
        h(
          "p",
          { style: "margin-top: 8px; color: var(--n-text-color-3);" },
          `“${preview}”`
        )
      ]),
    positiveText: t("common.delete"),
    negativeText: t("common.cancel"),
    onPositiveClick: () => {
      void memory.deleteMemoryItem(id);
    }
  });
}
</script>

<template>
  <div class="memory-browser">
    <header class="memory-header">
      <h2>{{ t("memory.header") }}</h2>
      <NButton
        size="tiny"
        quaternary
        :title="t('common.refresh')"
        class="refresh-btn"
        @click="memory.loadMemories()"
      >
        ↻
      </NButton>
    </header>

    <div class="memory-controls">
      <NSpace :size="6" align="center" class="scope-row">
        <NSelect
          :value="filter"
          :options="scopeOptions"
          size="small"
          class="scope-select"
          data-test="memory-scope-select"
          @update:value="handleFilterChange"
        />
      </NSpace>
      <NInput
        v-model:value="searchQuery"
        size="small"
        :placeholder="t('memory.searchPlaceholder')"
        class="search-input"
        @keydown="handleSearchKeydown"
      />
    </div>

    <NSpin v-if="loading" class="memory-empty" :show="true">
      <span>{{ t("common.loading") }}</span>
    </NSpin>
    <NEmpty
      v-else-if="memories.length === 0"
      size="small"
      class="memory-empty"
      :description="t('memory.emptyHint')"
    />
    <ul v-else class="memory-list">
      <li v-for="mem in memories" :key="mem.id" class="memory-item">
        <div class="memory-meta">
          <NTag
            size="small"
            :type="scopeTagType[mem.scope] ?? 'default'"
            :bordered="false"
            class="memory-scope"
          >
            {{ scopeIcon[mem.scope] || "•" }} {{ mem.scope }}
          </NTag>
          <NTag
            v-if="mem.key"
            size="small"
            :bordered="false"
            class="memory-key"
          >
            {{ mem.key }}
          </NTag>
          <NTag
            v-if="!mem.accepted"
            size="small"
            type="warning"
            :bordered="false"
            class="memory-pending"
          >
            pending
          </NTag>
        </div>
        <NText class="memory-content">{{ mem.content }}</NText>
        <div class="memory-actions">
          <NButton
            quaternary
            circle
            size="tiny"
            :title="t('common.delete')"
            class="memory-delete-btn"
            @click="promptDelete(mem.id, mem.content)"
          >
            🗑️
          </NButton>
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
}
.memory-controls {
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #eee);
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.scope-row {
  width: 100%;
}
.scope-select {
  min-width: 140px;
}
.search-input {
  width: 100%;
}
.memory-empty {
  padding: 16px;
  font-size: 12px;
  text-align: center;
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
</style>
