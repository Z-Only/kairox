import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

vi.mock("../composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));

vi.mock("./taskGraph", () => ({
  taskGraphState: { tasks: [], currentSessionId: null, loading: false },
  clearTaskGraph: vi.fn(),
  setTaskGraph: vi.fn()
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import {
  sessionState,
  deleteSession,
  renameSession,
  recoverSessions,
  resetProjection
} from "./session";

const makeSession = (id: string, title: string, profile = "fast") =>
  ({
    id,
    title,
    profile,
    model_id: null,
    provider: null
  }) as Parameters<typeof deleteSession>[0] extends string
    ? never
    : (typeof sessionState.sessions)[number];

const emptyProjection = {
  messages: [],
  task_titles: [],
  task_graph: { tasks: [] },
  token_stream: "",
  cancelled: false
};

beforeEach(() => {
  sessionState.sessions = [];
  sessionState.currentSessionId = null;
  sessionState.workspaceId = null;
  sessionState.currentProfile = "fast";
  sessionState.initialized = false;
  sessionState.isStreaming = false;
  resetProjection();
  vi.clearAllMocks();
});

describe("deleteSession", () => {
  it("removes session from the list on success", async () => {
    sessionState.sessions = [
      makeSession("s1", "Session 1"),
      makeSession("s2", "Session 2")
    ] as never[];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await deleteSession("s2");
    expect(sessionState.sessions).toHaveLength(1);
    expect(sessionState.sessions[0].id).toBe("s1");
  });

  it("switches to first remaining session when deleting current", async () => {
    sessionState.sessions = [
      makeSession("s1", "Session 1", "slow"),
      makeSession("s2", "Session 2", "fast")
    ] as never[];
    sessionState.currentSessionId = "s2";
    mockedInvoke.mockResolvedValueOnce(undefined); // delete_session
    mockedInvoke.mockResolvedValueOnce(emptyProjection); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace
    await deleteSession("s2");
    expect(sessionState.currentSessionId).toBe("s1");
  });

  it("notifies on error", async () => {
    const { addNotification } = await import("../composables/useNotifications");
    mockedInvoke.mockRejectedValueOnce(new Error("delete failed"));
    await deleteSession("s1");
    expect(addNotification).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("delete failed")
    );
  });
});

describe("renameSession", () => {
  it("updates local title on success", async () => {
    sessionState.sessions = [makeSession("s1", "Old Title")] as never[];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await renameSession("s1", "New Title");
    expect(sessionState.sessions[0].title).toBe("New Title");
  });

  it("notifies on error", async () => {
    const { addNotification } = await import("../composables/useNotifications");
    mockedInvoke.mockRejectedValueOnce(new Error("rename failed"));
    await renameSession("s1", "New Title");
    expect(addNotification).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("rename failed")
    );
  });
});

describe("recoverSessions", () => {
  it("restores workspace and sessions on success", async () => {
    mockedInvoke.mockResolvedValueOnce([{ workspace_id: "ws1", path: "/tmp" }]); // list_workspaces
    mockedInvoke.mockResolvedValueOnce(undefined); // restore_workspace
    mockedInvoke.mockResolvedValueOnce([
      {
        id: "s1",
        title: "Recovered",
        profile: "fast",
        model_id: null,
        provider: null
      }
    ]); // list_sessions
    mockedInvoke.mockResolvedValueOnce(emptyProjection); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace
    const result = await recoverSessions();
    expect(result).toBe(true);
    expect(sessionState.workspaceId).toBe("ws1");
    expect(sessionState.sessions).toHaveLength(1);
    expect(sessionState.currentSessionId).toBe("s1");
  });

  it("returns false when no workspaces exist", async () => {
    mockedInvoke.mockResolvedValueOnce([]); // list_workspaces
    const result = await recoverSessions();
    expect(result).toBe(false);
  });
});
