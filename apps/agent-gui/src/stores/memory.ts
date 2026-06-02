// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore` and `ref` explicitly.
import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useUiStore } from "@/stores/ui";

export interface MemoryItem {
  id: string;
  scope: string;
  key: string | null;
  content: string;
  accepted: boolean;
}

export const useMemoryStore = defineStore("memory", () => {
  const memories = ref<MemoryItem[]>([]);
  const loading = ref(false);
  const filter = ref<"all" | "session" | "user" | "workspace">("all");
  const searchQuery = ref("");

  async function loadMemories(): Promise<void> {
    const ui = useUiStore();
    loading.value = true;
    try {
      const scope = filter.value === "all" ? null : filter.value;
      const keywords = searchQuery.value ? searchQuery.value.split(/\s+/).filter(Boolean) : null;
      memories.value = await invoke("query_memories", {
        scope,
        keywords,
        limit: 100
      });
    } catch (e) {
      console.error("Failed to load memories:", e);
      ui.pushNotification("error", `Failed to load memories: ${e}`);
    } finally {
      loading.value = false;
    }
  }

  async function deleteMemoryItem(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("delete_memory", { id });
      memories.value = memories.value.filter((m) => m.id !== id);
    } catch (e) {
      console.error("Failed to delete memory:", e);
      ui.pushNotification("error", `Failed to delete memory: ${e}`);
    }
  }

  async function acceptMemoryItem(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("accept_memory", { id });
      const item = memories.value.find((m) => m.id === id);
      if (item) {
        item.accepted = true;
      }
    } catch (e) {
      console.error("Failed to accept memory:", e);
      ui.pushNotification("error", `Failed to accept memory: ${e}`);
    }
  }

  async function rejectMemoryItem(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("reject_memory", { id });
      memories.value = memories.value.filter((m) => m.id !== id);
    } catch (e) {
      console.error("Failed to reject memory:", e);
      ui.pushNotification("error", `Failed to reject memory: ${e}`);
    }
  }

  function setMemoryFilter(next: typeof filter.value): void {
    filter.value = next;
    void loadMemories();
  }

  return {
    memories,
    loading,
    filter,
    searchQuery,
    loadMemories,
    deleteMemoryItem,
    acceptMemoryItem,
    rejectMemoryItem,
    setMemoryFilter
  };
});
