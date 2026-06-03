import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const pushNotificationSpy = vi.fn();
vi.mock("@/stores/ui", () => ({
  useUiStore: () => createUiStoreMock({ pushNotification: pushNotificationSpy })
}));

import { invoke } from "@tauri-apps/api/core";
import { useMemoryStore } from "@/stores/memory";

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  pushNotificationSpy.mockClear();
});

describe("loadMemories", () => {
  it("invokes query_memories with null scope when filter is all", async () => {
    const memory = useMemoryStore();
    mockedInvoke.mockResolvedValueOnce([]);
    await memory.loadMemories();
    expect(mockedInvoke).toHaveBeenCalledWith("query_memories", {
      scope: null,
      keywords: null,
      limit: 100
    });
  });

  it("sets loading state during fetch", async () => {
    const memory = useMemoryStore();
    let resolvePromise: (value: unknown) => void;
    const promise = new Promise((resolve) => {
      resolvePromise = resolve;
    });
    mockedInvoke.mockReturnValueOnce(promise as Promise<unknown>);

    const loadPromise = memory.loadMemories();
    expect(memory.loading).toBe(true);

    resolvePromise!([]);
    await loadPromise;
    expect(memory.loading).toBe(false);
  });

  it("notifies on error", async () => {
    const memory = useMemoryStore();
    mockedInvoke.mockRejectedValueOnce(new Error("db error"));
    await memory.loadMemories();
    expect(pushNotificationSpy).toHaveBeenCalledWith("error", expect.stringContaining("db error"));
  });
});

describe("deleteMemoryItem", () => {
  it("removes item from memories on success", async () => {
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: true
      },
      {
        id: "m2",
        scope: "session",
        key: null,
        content: "temp",
        accepted: true
      }
    ];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await memory.deleteMemoryItem("m1");
    expect(memory.memories).toHaveLength(1);
    expect(memory.memories[0].id).toBe("m2");
  });

  it("notifies on error and keeps item in local state", async () => {
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: true
      }
    ];
    mockedInvoke.mockRejectedValueOnce(new Error("not found"));
    await memory.deleteMemoryItem("m1");
    expect(pushNotificationSpy).toHaveBeenCalledWith("error", expect.stringContaining("not found"));
    expect(memory.memories).toHaveLength(1);
  });
});

describe("acceptMemoryItem", () => {
  it("marks an item accepted on success", async () => {
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: false
      }
    ];
    mockedInvoke.mockResolvedValueOnce(undefined);

    await memory.acceptMemoryItem("m1");

    expect(mockedInvoke).toHaveBeenCalledWith("accept_memory", { id: "m1" });
    expect(memory.memories[0].accepted).toBe(true);
  });

  it("notifies on error and keeps item pending", async () => {
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: false
      }
    ];
    mockedInvoke.mockRejectedValueOnce(new Error("db error"));

    await memory.acceptMemoryItem("m1");

    expect(pushNotificationSpy).toHaveBeenCalledWith("error", expect.stringContaining("db error"));
    expect(memory.memories[0].accepted).toBe(false);
  });
});

describe("rejectMemoryItem", () => {
  it("removes an item on success", async () => {
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: false
      }
    ];
    mockedInvoke.mockResolvedValueOnce(undefined);

    await memory.rejectMemoryItem("m1");

    expect(mockedInvoke).toHaveBeenCalledWith("reject_memory", { id: "m1" });
    expect(memory.memories).toHaveLength(0);
  });

  it("notifies on error and keeps the pending item", async () => {
    const memory = useMemoryStore();
    memory.memories = [
      {
        id: "m1",
        scope: "user",
        key: "lang",
        content: "Rust",
        accepted: false
      }
    ];
    mockedInvoke.mockRejectedValueOnce(new Error("db error"));

    await memory.rejectMemoryItem("m1");

    expect(pushNotificationSpy).toHaveBeenCalledWith("error", expect.stringContaining("db error"));
    expect(memory.memories).toHaveLength(1);
  });
});

describe("setMemoryFilter", () => {
  it("updates filter and triggers loadMemories", async () => {
    const memory = useMemoryStore();
    mockedInvoke.mockResolvedValueOnce([]);
    memory.setMemoryFilter("user");
    expect(memory.filter).toBe("user");
    await vi.waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("query_memories", {
        scope: "user",
        keywords: null,
        limit: 100
      });
    });
  });
});
