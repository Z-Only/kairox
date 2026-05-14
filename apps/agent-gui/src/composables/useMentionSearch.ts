import { ref, shallowRef } from "vue";

export function useMentionSearch() {
  const filterText = ref("");
  const fileList = shallowRef<string[]>([]);
  const loaded = ref(false);

  async function loadFiles(_workspacePath: string) {
    // Simple glob: list files in workspace, cap at 500 for performance
    // In future, replace with a Tauri command for recursive listing
    fileList.value = [];
    loaded.value = false;
    try {
      // Use the existing glob or walk approach
      // For MVP, we use a simple prefix search against a preloaded list
      // The Tauri fs API or shell tool can enumerate files
    } catch {
      // Fail silently — file list is best-effort
    }
    loaded.value = true;
  }

  function matchingFiles(): string[] {
    const q = filterText.value.toLowerCase();
    if (!q) return fileList.value.slice(0, 20);

    // Simple fuzzy: filter paths containing the query chars in order
    return fileList.value
      .filter((path) => {
        const lower = path.toLowerCase();
        let qi = 0;
        for (let i = 0; i < lower.length && qi < q.length; i++) {
          if (lower[i] === q[qi]) qi++;
        }
        return qi === q.length;
      })
      .slice(0, 20);
  }

  function setFilter(text: string) {
    filterText.value = text;
  }

  return {
    filterText,
    fileList,
    loaded,
    loadFiles,
    matchingFiles,
    setFilter
  };
}
