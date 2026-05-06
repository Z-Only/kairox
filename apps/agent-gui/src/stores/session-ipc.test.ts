import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

const pushNotificationSpy = vi.fn();
vi.mock("@/stores/ui", () => ({
  useUiStore: () => createUiStoreMock({ pushNotification: pushNotificationSpy })
}));

vi.mock("@/composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));

vi.mock("@/stores/taskGraph", () => ({
  useTaskGraphStore: () => ({
    tasks: [],
    currentSessionId: null,
    loading: false,
    setTaskGraph: vi.fn(),
    clearTaskGraph: vi.fn(),
    applyTaskEvent: vi.fn()
  })
}));

vi.mock("@/stores/agents", () => ({
  useAgentsStore: () => ({
    agents: new Map(),
    clearAgents: vi.fn(),
    applyAgentEvent: vi.fn()
  })
}));

import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";

const mockedInvoke = vi.mocked(invoke);

const makeSession = (id: string, title: string, profile = "fast") => ({
  id,
  title,
  profile,
  model_id: null,
  provider: null
});

const emptyProjection = {
  messages: [],
  task_titles: [],
  task_graph: { tasks: [] },
  token_stream: "",
  cancelled: false
};

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  pushNotificationSpy.mockClear();
});

describe("deleteSession", () => {
  it("removes session from the list on success", async () => {
    const session = useSessionStore();
    session.sessions = [
      makeSession("s1", "Session 1"),
      makeSession("s2", "Session 2")
    ] as never[];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await session.deleteSession("s2");
    expect(session.sessions).toHaveLength(1);
    expect(session.sessions[0].id).toBe("s1");
  });

  it("switches to first remaining session when deleting current", async () => {
    const session = useSessionStore();
    session.sessions = [
      makeSession("s1", "Session 1", "slow"),
      makeSession("s2", "Session 2", "fast")
    ] as never[];
    session.currentSessionId = "s2";
    mockedInvoke.mockResolvedValueOnce(undefined); // delete_session
    mockedInvoke.mockResolvedValueOnce(emptyProjection); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace
    await session.deleteSession("s2");
    expect(session.currentSessionId).toBe("s1");
  });

  it("notifies on error", async () => {
    const session = useSessionStore();
    mockedInvoke.mockRejectedValueOnce(new Error("delete failed"));
    await session.deleteSession("s1");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("delete failed")
    );
  });
});

describe("renameSession", () => {
  it("updates local title on success", async () => {
    const session = useSessionStore();
    session.sessions = [makeSession("s1", "Old Title")] as never[];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await session.renameSession("s1", "New Title");
    expect(session.sessions[0].title).toBe("New Title");
  });

  it("notifies on error", async () => {
    const session = useSessionStore();
    mockedInvoke.mockRejectedValueOnce(new Error("rename failed"));
    await session.renameSession("s1", "New Title");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("rename failed")
    );
  });
});

describe("recoverSessions", () => {
  it("restores workspace and sessions on success", async () => {
    const session = useSessionStore();
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
    const result = await session.recoverSessions();
    expect(result).toBe(true);
    expect(session.workspaceId).toBe("ws1");
    expect(session.sessions).toHaveLength(1);
    expect(session.currentSessionId).toBe("s1");
  });

  it("returns false when no workspaces exist", async () => {
    const session = useSessionStore();
    mockedInvoke.mockResolvedValueOnce([]); // list_workspaces
    const result = await session.recoverSessions();
    expect(result).toBe(false);
  });
});
