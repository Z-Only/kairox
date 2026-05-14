import { ref, shallowRef } from "vue";
import { commands } from "@/generated/commands";

export function useMentionSearch() {
  const filterText = ref("");
  const fileList = shallowRef<string[]>([]);
  const loaded = ref(false);

  async function loadFiles(workspacePath: string) {
    loaded.value = false;
    try {
      const result = await commands.listWorkspaceFiles(workspacePath);
      if (result.status === "ok") {
        fileList.value = result.data.paths;
      } else {
        fileList.value = [];
      }
    } catch {
      fileList.value = [];
    }
    loaded.value = true;
  }

  function matchingFiles(): string[] {
    const q = filterText.value.toLowerCase();
    if (!q) return fileList.value.slice(0, 20);

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
