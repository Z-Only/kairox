import { describe, it, expect, beforeEach, vi } from "vitest";

const mockListWorkspaceFiles = vi.fn();
vi.mock("@/generated/commands", () => ({
  commands: {
    listWorkspaceFiles: (...args: unknown[]) => mockListWorkspaceFiles(...args)
  }
}));

import { useMentionSearch } from "./useMentionSearch";

beforeEach(() => {
  mockListWorkspaceFiles.mockReset();
});

describe("useMentionSearch", () => {
  describe("loadFiles", () => {
    it("populates fileList on successful load", async () => {
      mockListWorkspaceFiles.mockResolvedValue({
        status: "ok",
        data: { paths: ["src/main.ts", "src/app.vue", "README.md"] }
      });

      const { loadFiles, fileList, loaded } = useMentionSearch();
      await loadFiles("/workspace");

      expect(mockListWorkspaceFiles).toHaveBeenCalledWith("/workspace");
      expect(fileList.value).toEqual(["src/main.ts", "src/app.vue", "README.md"]);
      expect(loaded.value).toBe(true);
    });

    it("sets fileList to empty on error status", async () => {
      mockListWorkspaceFiles.mockResolvedValue({
        status: "error",
        error: "not found"
      });

      const { loadFiles, fileList, loaded } = useMentionSearch();
      await loadFiles("/bad-path");

      expect(fileList.value).toEqual([]);
      expect(loaded.value).toBe(true);
    });

    it("sets fileList to empty on exception", async () => {
      mockListWorkspaceFiles.mockRejectedValue(new Error("IPC timeout"));

      const { loadFiles, fileList, loaded } = useMentionSearch();
      await loadFiles("/workspace");

      expect(fileList.value).toEqual([]);
      expect(loaded.value).toBe(true);
    });

    it("sets loaded to false before loading starts", async () => {
      let resolvePromise: (v: unknown) => void;
      mockListWorkspaceFiles.mockReturnValue(
        new Promise((r) => {
          resolvePromise = r;
        })
      );

      const { loadFiles, loaded } = useMentionSearch();
      const loadPromise = loadFiles("/workspace");

      expect(loaded.value).toBe(false);

      resolvePromise!({ status: "ok", data: { paths: [] } });
      await loadPromise;

      expect(loaded.value).toBe(true);
    });
  });

  describe("matchingFiles (fuzzy filter)", () => {
    it("returns first 20 files when filter is empty", () => {
      const { fileList, matchingFiles } = useMentionSearch();
      const paths = Array.from({ length: 30 }, (_, i) => `file${i}.ts`);
      fileList.value = paths;

      const result = matchingFiles();
      expect(result).toHaveLength(20);
      expect(result).toEqual(paths.slice(0, 20));
    });

    it("performs subsequence (fuzzy) matching", () => {
      const { fileList, setFilter, matchingFiles } = useMentionSearch();
      fileList.value = [
        "src/components/Header.vue",
        "src/composables/useHelp.ts",
        "src/utils/hash.ts",
        "README.md"
      ];

      setFilter("hv");
      const result = matchingFiles();
      // "hv" matches "Header.vue" (H...e...a...d...e...r....[v]...u...e) — h then v
      expect(result).toContain("src/components/Header.vue");
      expect(result).not.toContain("README.md");
    });

    it("is case-insensitive", () => {
      const { fileList, setFilter, matchingFiles } = useMentionSearch();
      fileList.value = ["src/App.vue", "src/main.ts"];

      setFilter("APP");
      const result = matchingFiles();
      expect(result).toContain("src/App.vue");
    });

    it("limits results to 20", () => {
      const { fileList, setFilter, matchingFiles } = useMentionSearch();
      fileList.value = Array.from({ length: 50 }, (_, i) => `a${i}.ts`);

      setFilter("a");
      const result = matchingFiles();
      expect(result).toHaveLength(20);
    });

    it("returns empty array when nothing matches", () => {
      const { fileList, setFilter, matchingFiles } = useMentionSearch();
      fileList.value = ["src/main.ts", "src/app.vue"];

      setFilter("zzz");
      const result = matchingFiles();
      expect(result).toEqual([]);
    });
  });

  describe("setFilter", () => {
    it("updates filterText", () => {
      const { filterText, setFilter } = useMentionSearch();
      expect(filterText.value).toBe("");
      setFilter("hello");
      expect(filterText.value).toBe("hello");
    });
  });
});
