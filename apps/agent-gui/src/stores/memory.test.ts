import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import {
  memoryState,
  loadMemories,
  deleteMemoryItem,
  setMemoryFilter
} from "./memory";

beforeEach(() => {
  memoryState.memories = [];
  memoryState.loading = false;
  memoryState.filter = "all";
  memoryState.searchQuery = "";
  vi.clearAllMocks();
});

describe("loadMemories", () => {
  it("invokes query_memories with null scope when filter is all", async () => {
    mockedInvoke.mockResolvedValueOnce([]);
    await loadMemories();
    expect(mockedInvoke).toHaveBeenCalledWith("query_memories", {
      scope: null,
      keywords: null,
      limit: 100
    });
  });

  it("sets loading state during fetch", async () => {
    let resolvePromise: (value: unknown) => void;
    const promise = new Promise((resolve) => {
      resolvePromise = resolve;
    });
    mockedInvoke.mockReturnValueOnce(promise as Promise<unknown>);

    const loadPromise = loadMemories();
    expect(memoryState.loading).toBe(true);

    resolvePromise!([]);
    await loadPromise;
    expect(memoryState.loading).toBe(false);
  });

  it("notifies on error", async () => {
    const { addNotification } = await import("../composables/useNotifications");
    mockedInvoke.mockRejectedValueOnce(new Error("db error"));
    await loadMemories();
    expect(addNotification).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("db error")
    );
  });
});

describe("deleteMemoryItem", () => {
  it("removes item from memories on success", async () => {
    memoryState.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Rust", accepted: true },
      { id: "m2", scope: "session", key: null, content: "temp", accepted: true }
    ];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await deleteMemoryItem("m1");
    expect(memoryState.memories).toHaveLength(1);
    expect(memoryState.memories[0].id).toBe("m2");
  });

  it("notifies on error and keeps item in local state", async () => {
    const { addNotification } = await import("../composables/useNotifications");
    memoryState.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Rust", accepted: true }
    ];
    mockedInvoke.mockRejectedValueOnce(new Error("not found"));
    await deleteMemoryItem("m1");
    expect(addNotification).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("not found")
    );
    expect(memoryState.memories).toHaveLength(1);
  });
});

describe("setMemoryFilter", () => {
  it("updates filter and triggers loadMemories", async () => {
    mockedInvoke.mockResolvedValueOnce([]);
    setMemoryFilter("user");
    expect(memoryState.filter).toBe("user");
    await vi.waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("query_memories", {
        scope: "user",
        keywords: null,
        limit: 100
      });
    });
  });
});
