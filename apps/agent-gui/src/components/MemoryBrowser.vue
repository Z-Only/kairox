<script setup lang="ts">
import { onMounted, watch, ref } from "vue";
import {
  memoryState,
  loadMemories,
  deleteMemoryItem,
  setMemoryFilter
} from "../stores/memory";
import { sessionState } from "../stores/session";
import ConfirmDialog from "./ConfirmDialog.vue";

const deleteTargetId = ref("");
const deleteTargetContent = ref("");
const showDeleteDialog = ref(false);

onMounted(() => {
  loadMemories();
});

// Reload memories when switching sessions
watch(
  () => sessionState.currentSessionId,
  () => {
    loadMemories();
  }
);

function promptDelete(id: string, content: string) {
  deleteTargetId.value = id;
  deleteTargetContent.value = content;
  showDeleteDialog.value = true;
}

async function confirmDelete() {
  await deleteMemoryItem(deleteTargetId.value);
  showDeleteDialog.value = false;
}

function cancelDelete() {
  showDeleteDialog.value = false;
}

const scopeFilters: Array<{
  label: string;
  value: typeof memoryState.filter;
}> = [
  { label: "All", value: "all" },
  { label: "Session", value: "session" },
  { label: "User", value: "user" },
  { label: "Workspace", value: "workspace" }
];

const scopeIcon: Record<string, string> = {
  session: "📋",
  user: "👤",
  workspace: "🗂️"
};

const scopeColor: Record<string, string> = {
  session: "#6b7280",
  user: "#0077cc",
  workspace: "#22a06b"
};

function handleSearchKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    loadMemories();
  }
}
</script>

<template>
  <div class="memory-browser">
    <header class="memory-header">
      <h2>Memories</h2>
      <button class="refresh-btn" title="Refresh" @click="loadMemories">
        ↻
      </button>
    </header>

    <div class="memory-controls">
      <div class="scope-filters">
        <button
          v-for="f in scopeFilters"
          :key="f.value"
          :class="['scope-btn', { active: memoryState.filter === f.value }]"
          @click="setMemoryFilter(f.value)"
        >
          {{ f.label }}
        </button>
      </div>
      <input
        v-model="memoryState.searchQuery"
        class="search-input"
        placeholder="Search memories..."
        @keydown="handleSearchKeydown"
      />
    </div>

    <div v-if="memoryState.loading" class="memory-empty">Loading...</div>
    <div v-else-if="memoryState.memories.length === 0" class="memory-empty">
      No memories
    </div>
    <ul v-else class="memory-list">
      <li v-for="mem in memoryState.memories" :key="mem.id" class="memory-item">
        <div class="memory-meta">
          <span
            class="memory-scope"
            :style="{ color: scopeColor[mem.scope] || '#666' }"
          >
            {{ scopeIcon[mem.scope] || "•" }} {{ mem.scope }}
          </span>
          <span v-if="mem.key" class="memory-key">{{ mem.key }}</span>
          <span v-if="!mem.accepted" class="memory-pending">pending</span>
        </div>
        <div class="memory-content">{{ mem.content }}</div>
        <div class="memory-actions">
          <button
            class="memory-delete-btn"
            title="Delete"
            @click="promptDelete(mem.id, mem.content)"
          >
            🗑️
          </button>
        </div>
      </li>
    </ul>

    <ConfirmDialog
      v-if="showDeleteDialog"
      title="Delete Memory?"
      :message="`Delete this memory: '${deleteTargetContent.slice(0, 60)}${deleteTargetContent.length > 60 ? '…' : ''}'?`"
      confirm-label="Delete"
      :confirm-danger="true"
      @confirm="confirmDelete"
      @cancel="cancelDelete"
    />
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
  border-bottom: 1px solid #d7d7d7;
}
.memory-header h2 {
  margin: 0;
  font-size: 14px;
}
.refresh-btn {
  background: none;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  cursor: pointer;
  font-size: 14px;
  padding: 2px 6px;
}
.refresh-btn:hover {
  background: #f0f0f0;
}
.memory-controls {
  padding: 8px 12px;
  border-bottom: 1px solid #eee;
}
.scope-filters {
  display: flex;
  gap: 4px;
  margin-bottom: 6px;
}
.scope-btn {
  padding: 2px 8px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  background: white;
  cursor: pointer;
  font-size: 11px;
}
.scope-btn.active {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
.search-input {
  width: 100%;
  padding: 4px 8px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  font-size: 12px;
}
.search-input:focus {
  outline: none;
  border-color: #0077cc;
}
.memory-empty {
  padding: 16px;
  color: #999;
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
  border-bottom: 1px solid #eee;
  position: relative;
}
.memory-item:hover {
  background: #f8f8f8;
}
.memory-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}
.memory-scope {
  font-size: 11px;
  font-weight: 600;
}
.memory-key {
  font-size: 11px;
  color: #666;
  background: #f0f0f0;
  padding: 1px 4px;
  border-radius: 3px;
}
.memory-pending {
  font-size: 10px;
  color: #b45309;
  background: #fffbeb;
  padding: 1px 4px;
  border-radius: 3px;
}
.memory-content {
  font-size: 12px;
  color: #333;
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
  font-size: 13px;
  padding: 2px;
}
.memory-delete-btn:hover {
  background: rgba(204, 51, 51, 0.1);
  border-radius: 3px;
}
</style>
