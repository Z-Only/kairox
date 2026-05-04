import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { addNotification } from "../composables/useNotifications";

export interface MemoryItem {
  id: string;
  scope: string;
  key: string | null;
  content: string;
  accepted: boolean;
}

export const memoryState = reactive({
  memories: [] as MemoryItem[],
  loading: false,
  filter: "all" as "all" | "session" | "user" | "workspace",
  searchQuery: ""
});

export async function loadMemories(): Promise<void> {
  memoryState.loading = true;
  try {
    const scope = memoryState.filter === "all" ? null : memoryState.filter;
    const keywords = memoryState.searchQuery
      ? memoryState.searchQuery.split(/\s+/).filter(Boolean)
      : null;
    memoryState.memories = await invoke("query_memories", {
      scope,
      keywords,
      limit: 100
    });
  } catch (e) {
    console.error("Failed to load memories:", e);
    addNotification("error", `Failed to load memories: ${e}`);
  } finally {
    memoryState.loading = false;
  }
}

export async function deleteMemoryItem(id: string): Promise<void> {
  try {
    await invoke("delete_memory", { id });
    memoryState.memories = memoryState.memories.filter((m) => m.id !== id);
  } catch (e) {
    console.error("Failed to delete memory:", e);
    addNotification("error", `Failed to delete memory: ${e}`);
  }
}

export function setMemoryFilter(filter: typeof memoryState.filter): void {
  memoryState.filter = filter;
  loadMemories();
}
