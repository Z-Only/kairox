import { describe, it, expect, beforeEach, vi } from "vitest";
import { ref, type Ref } from "vue";
import type { SessionInfoResponse, DomainEvent } from "@/types";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

vi.mock("@/composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));

import { invoke } from "@tauri-apps/api/core";
import { clearTrace, applyTraceEvent } from "@/composables/useTraceStore";
import {
  switchToKnownSession,
  createSession,
  deleteSession,
  renameSession,
  type SessionActionDeps
} from "@/stores/sessionActions";
import { emptyProjection } from "@/stores/sessionEvents";

const mockedInvoke = vi.mocked(invoke);
const mockedClearTrace = vi.mocked(clearTrace);
const mockedApplyTraceEvent = vi.mocked(applyTraceEvent);

function makeSessionInfo(
  id: string,
  title: string,
  profile = "fast",
  overrides: Partial<SessionInfoResponse> = {}
): SessionInfoResponse {
  return {
    id,
    title,
    profile,
    project_id: null,
    worktree_path: null,
    branch: null,
    deleted_at: null,
    visibility: null,
    ...overrides
  };
}

function makeDeps(overrides: Partial<SessionActionDeps> = {}): SessionActionDeps {
  return {
    sessions: ref([]) as Ref<SessionInfoResponse[]>,
    currentSessionId: ref(null) as Ref<string | null>,
    currentProfile: ref("fast"),
    currentReasoningEffort: ref(null) as Ref<string | null>,
    approvalPolicy: ref("ask_always"),
    sandboxPolicy: ref("none"),
    profileInfos: ref([]) as Ref<{ alias: string }[]>,
    resetProjection: vi.fn(),
    setProjection: vi.fn(),
    getTaskGraphStore: vi.fn(() => ({
      tasks: [],
      currentSessionId: null,
      loading: false,
      setTaskGraph: vi.fn(),
      clearTaskGraph: vi.fn(),
      applyTaskEvent: vi.fn()
    })),
    getUiStore: vi.fn(() => createUiStoreMock()) as never,
    ...overrides
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockedInvoke.mockReset();
});

// ---------------------------------------------------------------------------
// switchToKnownSession
// ---------------------------------------------------------------------------
describe("switchToKnownSession", () => {
  it("no-ops when switching to the already-active session", async () => {
    const deps = makeDeps({ currentSessionId: ref("s1") as Ref<string | null> });
    const target = makeSessionInfo("s1", "Current");

    await switchToKnownSession("s1", target, deps);

    expect(mockedInvoke).not.toHaveBeenCalled();
    expect(deps.resetProjection).not.toHaveBeenCalled();
  });

  it("calls switch_session and updates currentSessionId + profile", async () => {
    const deps = makeDeps();
    const target = makeSessionInfo("s2", "Other", "slow");
    const proj = emptyProjection();

    mockedInvoke.mockResolvedValueOnce(proj); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace

    await switchToKnownSession("s2", target, deps);

    expect(deps.resetProjection).toHaveBeenCalled();
    expect(mockedClearTrace).toHaveBeenCalled();
    expect(mockedInvoke).toHaveBeenCalledWith("switch_session", { sessionId: "s2" });
    expect(deps.currentSessionId.value).toBe("s2");
    expect(deps.currentProfile.value).toBe("slow");
    expect(deps.setProjection).toHaveBeenCalledWith(proj);
  });

  it("applies approval_policy and sandbox_policy from the target session", async () => {
    const deps = makeDeps();
    const target = makeSessionInfo("s2", "Other", "fast", {
      approval_policy: "auto_approve",
      sandbox_policy: "container"
    } as never);

    mockedInvoke.mockResolvedValueOnce(emptyProjection()); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace

    await switchToKnownSession("s2", target, deps);

    expect(deps.approvalPolicy.value).toBe("auto_approve");
    expect(deps.sandboxPolicy.value).toBe("container");
  });

  it("falls back to first profile when target uses 'default'", async () => {
    const deps = makeDeps({
      profileInfos: ref([{ alias: "opus" }, { alias: "sonnet" }]) as Ref<{ alias: string }[]>
    });
    const target = makeSessionInfo("s2", "Other", "default");

    mockedInvoke.mockResolvedValueOnce(emptyProjection()); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace

    await switchToKnownSession("s2", target, deps);

    expect(deps.currentProfile.value).toBe("opus");
  });

  it("replays trace events and captures ModelProfileSwitched profile state", async () => {
    const deps = makeDeps();
    const target = makeSessionInfo("s2", "Other");
    const traceEvent: DomainEvent = {
      schema_version: 1,
      workspace_id: "w1",
      session_id: "s2",
      timestamp: "2026-05-06T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "ModelProfileSwitched",
      payload: {
        type: "ModelProfileSwitched",
        from_profile: "fast",
        to_profile: "opus",
        effective_at: "2026-05-06T00:00:00Z",
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry",
        reasoning_effort: "high"
      }
    } as DomainEvent;

    mockedInvoke.mockResolvedValueOnce(emptyProjection()); // switch_session
    mockedInvoke.mockResolvedValueOnce([JSON.stringify(traceEvent)]); // get_trace

    await switchToKnownSession("s2", target, deps);

    expect(deps.currentProfile.value).toBe("opus");
    expect(deps.currentReasoningEffort.value).toBe("high");
    expect(mockedApplyTraceEvent).toHaveBeenCalledWith(traceEvent);
  });

  it("skips malformed trace entries without throwing", async () => {
    const deps = makeDeps();
    const target = makeSessionInfo("s2", "Other");
    const validEvent: DomainEvent = {
      schema_version: 1,
      workspace_id: "w1",
      session_id: "s2",
      timestamp: "2026-05-06T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "UserMessageAdded",
      payload: { type: "UserMessageAdded", message_id: "m1", content: "hello" }
    } as DomainEvent;

    mockedInvoke.mockResolvedValueOnce(emptyProjection()); // switch_session
    mockedInvoke.mockResolvedValueOnce(["not valid json", JSON.stringify(validEvent)]); // get_trace

    await expect(switchToKnownSession("s2", target, deps)).resolves.toBeUndefined();
    // The first entry is invalid JSON (skipped), the second is valid and applied
    expect(mockedApplyTraceEvent).toHaveBeenCalledTimes(1);
  });

  it("resets reasoning_effort to null on switch", async () => {
    const deps = makeDeps({
      currentReasoningEffort: ref("high") as Ref<string | null>
    });
    const target = makeSessionInfo("s2", "Other");

    mockedInvoke.mockResolvedValueOnce(emptyProjection()); // switch_session
    mockedInvoke.mockResolvedValueOnce([]); // get_trace (no ModelProfileSwitched events)

    await switchToKnownSession("s2", target, deps);

    // switchToKnownSession explicitly sets currentReasoningEffort to null (line 42)
    expect(deps.currentReasoningEffort.value).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// createSession
// ---------------------------------------------------------------------------
describe("createSession", () => {
  it("calls start_session, refreshes sessions, and resets projection", async () => {
    const deps = makeDeps();
    const newSession = { id: "s-new", title: "New Session", profile: "fast" };

    mockedInvoke.mockResolvedValueOnce(newSession); // start_session
    mockedInvoke.mockResolvedValueOnce([makeSessionInfo("s-new", "New Session")]); // list_sessions

    const result = await createSession("fast", deps);

    expect(result).toEqual({ id: "s-new", title: "New Session", profile: "fast" });
    expect(mockedInvoke).toHaveBeenCalledWith("start_session", { profile: "fast" });
    expect(deps.currentSessionId.value).toBe("s-new");
    expect(deps.currentProfile.value).toBe("fast");
    expect(deps.currentReasoningEffort.value).toBeNull();
    expect(deps.resetProjection).toHaveBeenCalled();
    expect(mockedClearTrace).toHaveBeenCalled();
  });

  it("uses deps.currentProfile when profileArg is undefined", async () => {
    const deps = makeDeps({ currentProfile: ref("opus") });

    mockedInvoke.mockResolvedValueOnce({ id: "s1", title: "New Session", profile: "opus" });
    mockedInvoke.mockResolvedValueOnce([makeSessionInfo("s1", "New Session", "opus")]);

    await createSession(undefined, deps);

    expect(mockedInvoke).toHaveBeenCalledWith("start_session", { profile: "opus" });
  });

  it("deduplicates session title when it conflicts", async () => {
    const deps = makeDeps();
    const existing = makeSessionInfo("s-old", "New Session");

    mockedInvoke.mockResolvedValueOnce({
      id: "s-new",
      title: "New Session",
      profile: "fast"
    }); // start_session
    mockedInvoke.mockResolvedValueOnce([existing, makeSessionInfo("s-new", "New Session")]); // list_sessions
    mockedInvoke.mockResolvedValueOnce(undefined); // rename_session

    const result = await createSession("fast", deps);

    // Title should be deduplicated
    expect(result.title).toBe("New Session 1");
    expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
      sessionId: "s-new",
      title: "New Session 1"
    });
  });

  it("handles rename failure gracefully", async () => {
    const deps = makeDeps();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    mockedInvoke.mockResolvedValueOnce({
      id: "s-new",
      title: "New Session",
      profile: "fast"
    }); // start_session
    mockedInvoke.mockResolvedValueOnce([
      makeSessionInfo("s-old", "New Session"),
      makeSessionInfo("s-new", "New Session")
    ]); // list_sessions
    mockedInvoke.mockRejectedValueOnce(new Error("rename failed")); // rename_session

    // Should not throw — the rename error is caught internally
    const result = await createSession("fast", deps);
    expect(result.id).toBe("s-new");
    expect(consoleSpy).toHaveBeenCalled();
    consoleSpy.mockRestore();
  });

  it("clears task graph on create", async () => {
    const clearTaskGraph = vi.fn();
    const deps = makeDeps({
      getTaskGraphStore: vi.fn(() => ({
        tasks: [],
        currentSessionId: null,
        loading: false,
        setTaskGraph: vi.fn(),
        clearTaskGraph,
        applyTaskEvent: vi.fn()
      })) as never
    });

    mockedInvoke.mockResolvedValueOnce({ id: "s-new", title: "New Session", profile: "fast" });
    mockedInvoke.mockResolvedValueOnce([makeSessionInfo("s-new", "New Session")]);

    await createSession("fast", deps);

    expect(clearTaskGraph).toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// deleteSession
// ---------------------------------------------------------------------------
describe("deleteSession", () => {
  it("removes the session from deps.sessions on success", async () => {
    const deps = makeDeps({
      sessions: ref([makeSessionInfo("s1", "A"), makeSessionInfo("s2", "B")]) as Ref<
        SessionInfoResponse[]
      >
    });
    const switchFn = vi.fn();

    mockedInvoke.mockResolvedValueOnce(undefined); // delete_session

    await deleteSession("s2", deps, switchFn);

    expect(deps.sessions.value).toHaveLength(1);
    expect(deps.sessions.value[0].id).toBe("s1");
    expect(switchFn).not.toHaveBeenCalled();
  });

  it("switches to the first remaining session when the current one is deleted", async () => {
    const deps = makeDeps({
      sessions: ref([makeSessionInfo("s1", "A"), makeSessionInfo("s2", "B")]) as Ref<
        SessionInfoResponse[]
      >,
      currentSessionId: ref("s2") as Ref<string | null>
    });
    const switchFn = vi.fn();

    mockedInvoke.mockResolvedValueOnce(undefined); // delete_session

    await deleteSession("s2", deps, switchFn);

    expect(switchFn).toHaveBeenCalledWith("s1");
  });

  it("resets to null when the last session is deleted", async () => {
    const deps = makeDeps({
      sessions: ref([makeSessionInfo("s1", "Only")]) as Ref<SessionInfoResponse[]>,
      currentSessionId: ref("s1") as Ref<string | null>
    });
    const switchFn = vi.fn();

    mockedInvoke.mockResolvedValueOnce(undefined); // delete_session

    await deleteSession("s1", deps, switchFn);

    expect(deps.currentSessionId.value).toBeNull();
    expect(deps.resetProjection).toHaveBeenCalled();
    expect(mockedClearTrace).toHaveBeenCalled();
    expect(switchFn).not.toHaveBeenCalled();
  });

  it("pushes error notification on failure", async () => {
    const pushNotification = vi.fn();
    const deps = makeDeps({
      getUiStore: vi.fn(() => createUiStoreMock({ pushNotification })) as never
    });
    const switchFn = vi.fn();

    mockedInvoke.mockRejectedValueOnce(new Error("delete failed"));

    await deleteSession("s1", deps, switchFn);

    expect(pushNotification).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("delete failed")
    );
  });
});

// ---------------------------------------------------------------------------
// renameSession
// ---------------------------------------------------------------------------
describe("renameSession", () => {
  it("updates session title locally on success", async () => {
    const deps = makeDeps({
      sessions: ref([makeSessionInfo("s1", "Old Title")]) as Ref<SessionInfoResponse[]>
    });

    mockedInvoke.mockResolvedValueOnce(undefined); // rename_session

    await renameSession("s1", "New Title", deps);

    expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
      sessionId: "s1",
      title: "New Title"
    });
    expect(deps.sessions.value[0].title).toBe("New Title");
  });

  it("does not crash when session is not found locally", async () => {
    const deps = makeDeps({
      sessions: ref([]) as Ref<SessionInfoResponse[]>
    });

    mockedInvoke.mockResolvedValueOnce(undefined); // rename_session

    await renameSession("s-missing", "Title", deps);

    expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
      sessionId: "s-missing",
      title: "Title"
    });
  });

  it("pushes error notification on failure", async () => {
    const pushNotification = vi.fn();
    const deps = makeDeps({
      getUiStore: vi.fn(() => createUiStoreMock({ pushNotification })) as never
    });

    mockedInvoke.mockRejectedValueOnce(new Error("rename failed"));

    await renameSession("s1", "Title", deps);

    expect(pushNotification).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("rename failed")
    );
  });
});

// ---------------------------------------------------------------------------
// listOrdinarySessions (indirectly via createSession)
// ---------------------------------------------------------------------------
describe("listOrdinarySessions (via createSession)", () => {
  it("filters out project-bound sessions from the refreshed list", async () => {
    const deps = makeDeps();
    const ordinary = makeSessionInfo("s1", "Regular");
    const projectBound = makeSessionInfo("s2", "Project", "fast", {
      project_id: "p1",
      worktree_path: "/tmp/wt"
    });

    mockedInvoke.mockResolvedValueOnce({ id: "s1", title: "Regular", profile: "fast" }); // start_session
    mockedInvoke.mockResolvedValueOnce([ordinary, projectBound]); // list_sessions

    await createSession("fast", deps);

    // Only ordinary sessions should remain after filtering
    expect(deps.sessions.value).toHaveLength(1);
    expect(deps.sessions.value[0].id).toBe("s1");
  });
});
