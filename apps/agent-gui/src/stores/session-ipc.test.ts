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
  cancelled: false,
  last_context_usage: null,
  model_limits: null,
  compaction: { type: "Idle" }
};

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  pushNotificationSpy.mockClear();
});

describe("switchSession", () => {
  it("updates the current session only after the backend switch completes", async () => {
    const session = useSessionStore();
    session.sessions = [makeSession("s1", "Session 1", "fast")] as never[];

    let resolveSwitch!: (value: typeof emptyProjection) => void;
    const switchPromise = new Promise<typeof emptyProjection>((resolve) => {
      resolveSwitch = resolve;
    });
    mockedInvoke.mockReturnValueOnce(switchPromise as never); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace

    const pendingSwitch = session.switchSession("s1");

    expect(session.currentSessionId).toBeNull();
    resolveSwitch(emptyProjection);
    await pendingSwitch;
    expect(session.currentSessionId).toBe("s1");
  });
});

describe("deleteSession", () => {
  it("removes session from the list on success", async () => {
    const session = useSessionStore();
    session.sessions = [makeSession("s1", "Session 1"), makeSession("s2", "Session 2")] as never[];
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
    expect(mockedInvoke).toHaveBeenCalledWith("switch_session", { sessionId: "s1" });
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

describe("createSession", () => {
  it("calls start_session, refreshes the session list, resets the projection, and clears the trace", async () => {
    const session = useSessionStore();
    // Pre-existing local state that the new action must reset so the
    // workbench starts clean for the newly-created session. The view layer
    // must NOT have to do these mutations itself (Task 5 NIT #10).
    session.projection.messages = [{ role: "user", content: "stale" }];
    session.projection.token_stream = "stale tokens";
    session.isStreaming = true;
    session.streamsByTask.set("t-old", "partial");
    session.currentProfile = "old";

    mockedInvoke
      .mockResolvedValueOnce({ id: "s-new", title: "New Session", profile: "fast" }) // start_session
      .mockResolvedValueOnce([makeSession("s-new", "New Session", "fast")] as never[]); // list_sessions

    const result = await session.createSession("fast");

    expect(result).toEqual({ id: "s-new", title: "New Session", profile: "fast" });
    expect(session.sessions).toHaveLength(1);
    expect(session.sessions[0].id).toBe("s-new");
    expect(session.currentProfile).toBe("fast");
    // Side effects that used to live in the view (per Task 5 NIT #10) now
    // belong to the action.
    expect(session.projection.messages).toEqual([]);
    expect(session.projection.token_stream).toBe("");
    expect(session.isStreaming).toBe(false);
    expect(session.streamsByTask.size).toBe(0);

    // start_session was invoked with the requested profile alias.
    const startCall = mockedInvoke.mock.calls.find(([cmd]) => cmd === "start_session");
    expect(startCall).toBeDefined();
    expect(startCall?.[1]).toEqual({ profile: "fast", permissionMode: "suggest" });
  });

  it("propagates start_session failures to the caller (the view surfaces them)", async () => {
    const session = useSessionStore();
    mockedInvoke.mockRejectedValueOnce(new Error("backend offline"));
    await expect(session.createSession("fast")).rejects.toThrow("backend offline");
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

describe("setPermissionMode", () => {
  it("maps legacy mode to dual-axis policy IPCs and updates local state", async () => {
    const session = useSessionStore();
    expect(session.permissionMode).toBe("suggest");

    mockedInvoke.mockResolvedValueOnce("on_request"); // set_session_approval_policy
    mockedInvoke.mockResolvedValueOnce(
      '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    ); // set_session_sandbox_policy
    await session.setPermissionMode("agent");

    expect(mockedInvoke).toHaveBeenCalledWith("set_session_approval_policy", {
      approval: "on_request"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("set_session_sandbox_policy", {
      sandboxJson: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    });
    expect(session.permissionMode).toBe("agent");
    expect(session.approvalPolicy).toBe("on_request");
    expect(session.sandboxPolicy).toBe(
      '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    );
  });

  it("notifies on IPC failure and preserves previous mode", async () => {
    const session = useSessionStore();
    session.permissionMode = "suggest";

    mockedInvoke.mockRejectedValueOnce(new Error("IPC offline"));
    mockedInvoke.mockRejectedValueOnce(new Error("IPC offline"));
    await session.setPermissionMode("autonomous");

    expect(session.permissionMode).toBe("suggest");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to set permission mode")
    );
  });

  it("rejects unknown permission mode without invoking IPC", async () => {
    const session = useSessionStore();
    await session.setPermissionMode("bogus");
    expect(mockedInvoke).not.toHaveBeenCalled();
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Unknown permission mode")
    );
  });

  it("round-trips each valid permission mode value", async () => {
    const session = useSessionStore();
    const modes = ["read_only", "suggest", "agent", "autonomous", "interactive"];

    for (const mode of modes) {
      mockedInvoke.mockResolvedValueOnce("ok"); // approval
      mockedInvoke.mockResolvedValueOnce("ok"); // sandbox
      await session.setPermissionMode(mode);
      expect(session.permissionMode).toBe(mode);
    }

    expect(mockedInvoke).toHaveBeenCalledTimes(modes.length * 2);
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
