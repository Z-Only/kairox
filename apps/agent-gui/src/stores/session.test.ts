import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import {
  filterOrdinarySessions,
  temporaryTitleFromFirstMessage,
  uniqueSessionTitle,
  useSessionStore
} from "@/stores/session";
import type { SessionInfoResponse } from "@/types";
import { useAgentsStore } from "@/stores/agents";
import { useProjectStore } from "@/stores/project";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

const mockedInvoke = vi.mocked(invoke);
import type { DomainEvent, AgentRole, EventPayload } from "@/types";

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedInvoke.mockImplementation((command) => {
    if (command === "switch_session") {
      return Promise.resolve({
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false,
        last_context_usage: null,
        model_limits: null,
        compaction: { type: "Idle" }
      });
    }

    if (command === "get_trace") {
      return Promise.resolve([]);
    }

    return Promise.resolve(null);
  });
});

function makeEvent(payload: EventPayload, sourceAgentId = "agent_system"): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_1",
    session_id: "ses_1",
    timestamp: "2026-05-06T00:00:00Z",
    source_agent_id: sourceAgentId,
    privacy: "full_trace",
    event_type: payload.type,
    payload
  } as DomainEvent;
}

describe("applyEvent", () => {
  it("projects UserMessageAdded", () => {
    const session = useSessionStore();
    session.applyEvent(
      makeEvent({
        type: "UserMessageAdded",
        message_id: "m1",
        content: "hello"
      })
    );
    expect(session.projection.messages).toHaveLength(1);
    expect(session.projection.messages[0].role).toBe("user");
    expect(session.projection.messages[0].content).toBe("hello");
    expect(session.isStreaming).toBe(true);
  });

  it("accumulates ModelTokenDelta into token_stream", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "ModelTokenDelta", delta: "hel" }));
    session.applyEvent(makeEvent({ type: "ModelTokenDelta", delta: "lo" }));
    expect(session.projection.token_stream).toBe("hello");
  });

  it("finalizes on AssistantMessageCompleted", () => {
    const session = useSessionStore();
    session.applyEvent(
      makeEvent({
        type: "AssistantMessageCompleted",
        message_id: "m2",
        content: "hi there"
      })
    );
    expect(session.projection.messages).toHaveLength(1);
    expect(session.projection.messages[0].role).toBe("assistant");
    expect(session.projection.messages[0].content).toBe("hi there");
    expect(session.projection.token_stream).toBe("");
    expect(session.isStreaming).toBe(false);
  });

  it("attributes AssistantMessageCompleted to agent when source_agent_id is known", () => {
    const session = useSessionStore();
    const agents = useAgentsStore();
    agents.agents.set("agent_w1", {
      id: "agent_w1",
      role: "Worker" as AgentRole,
      taskId: "t1",
      status: "running",
      startedAt: Date.now(),
      completedAt: null
    });
    session.applyEvent(
      makeEvent(
        {
          type: "AssistantMessageCompleted",
          message_id: "m3",
          content: "worker response"
        },
        "agent_w1"
      )
    );
    expect(session.projection.messages).toHaveLength(1);
    expect(session.projection.messages[0].role).toBe("worker");
    expect(session.projection.messages[0].sourceAgentId).toBe("agent_w1");
  });

  it("marks cancelled on SessionCancelled", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "SessionCancelled", reason: "user stopped" }));
    expect(session.projection.cancelled).toBe(true);
    expect(session.isStreaming).toBe(false);
  });

  it("handles TaskDecomposed event", () => {
    const session = useSessionStore();
    session.applyEvent(
      makeEvent({
        type: "TaskDecomposed",
        parent_task_id: "parent",
        sub_task_ids: ["sub1", "sub2", "sub3"]
      })
    );
    expect(session.projection.messages).toHaveLength(1);
    expect(session.projection.messages[0].role).toBe("system");
    expect(session.projection.messages[0].content).toContain("3 sub-tasks");
  });

  it("handles TaskBlocked event", () => {
    const session = useSessionStore();
    session.applyEvent(
      makeEvent({
        type: "TaskBlocked",
        task_id: "t1",
        blocking_task_id: "t0",
        reason: "dependency failed"
      })
    );
    expect(session.projection.messages).toHaveLength(1);
    expect(session.projection.messages[0].role).toBe("system");
    expect(session.projection.messages[0].content).toContain("blocked");
  });

  it("handles TaskRetried event", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "TaskRetried", task_id: "t1", attempt: 2 }));
    expect(session.projection.messages).toHaveLength(1);
    expect(session.projection.messages[0].role).toBe("system");
    expect(session.projection.messages[0].content).toContain("attempt 2");
  });

  it("ignores AgentSpawned and AgentIdle events gracefully", () => {
    const session = useSessionStore();
    session.applyEvent(
      makeEvent({
        type: "AgentSpawned",
        agent_id: "a1",
        role: "Worker",
        task_id: "t1"
      })
    );
    session.applyEvent(makeEvent({ type: "AgentIdle", agent_id: "a1" }));
    expect(session.projection.messages).toHaveLength(0);
  });

  it("ignores unknown event types gracefully", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "FutureEvent" } as never));
    expect(session.projection.messages).toHaveLength(0);
  });
});

describe("setProjection", () => {
  it("replaces the current projection", () => {
    const session = useSessionStore();
    session.setProjection({
      messages: [
        { role: "user", content: "existing" },
        { role: "assistant", content: "reply" }
      ],
      task_titles: ["task 1"],
      token_stream: "",
      cancelled: false,
      task_graph: { tasks: [] },
      last_context_usage: null,
      model_limits: null,
      compaction: { type: "Idle" }
    });
    expect(session.projection.messages).toHaveLength(2);
    expect(session.isStreaming).toBe(false);
  });
});

describe("resetProjection", () => {
  it("clears all projection state and agent state", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "UserMessageAdded", message_id: "m1", content: "hi" }));
    session.resetProjection();
    expect(session.projection.messages).toHaveLength(0);
    expect(session.projection.token_stream).toBe("");
    expect(session.projection.cancelled).toBe(false);
    expect(session.isStreaming).toBe(false);
    expect(session.streamsByTask.size).toBe(0);
  });
});

describe("temporaryTitleFromFirstMessage", () => {
  it("uses a fallback title for blank first messages", () => {
    expect(temporaryTitleFromFirstMessage("   \n\t  ")).toBe("New Session");
  });

  it("trims and truncates long first messages", () => {
    const title = temporaryTitleFromFirstMessage(
      "  Please help me implement project workspace navigation with archived sessions  "
    );

    expect(title).toBe("Please help me implement project workspace navig…");
  });
});

describe("uniqueSessionTitle", () => {
  it("returns base when no conflict exists", () => {
    expect(uniqueSessionTitle("New Session", ["Other"])).toBe("New Session");
  });

  it("appends 1 on first conflict", () => {
    expect(uniqueSessionTitle("New Session", ["New Session"])).toBe("New Session 1");
  });

  it("increments counter correctly for second conflict", () => {
    expect(uniqueSessionTitle("New Session", ["New Session", "New Session 1"])).toBe(
      "New Session 2"
    );
  });

  it("returns base when only numbered variants exist", () => {
    expect(uniqueSessionTitle("New Session", ["New Session 1"])).toBe("New Session");
  });
});

describe("filterOrdinarySessions", () => {
  it("excludes project-bound sessions from the ordinary session list", () => {
    const ordinarySession: SessionInfoResponse = {
      id: "s1",
      title: "Regular",
      profile: "fast",
      project_id: null,
      worktree_path: null,
      branch: null,
      visibility: null
    };
    const projectSession: SessionInfoResponse = {
      id: "s2",
      title: "Project Draft",
      profile: "fast",
      project_id: "p1",
      worktree_path: "/tmp/demo",
      branch: null,
      visibility: "draft_hidden"
    };

    expect(filterOrdinarySessions([ordinarySession, projectSession])).toEqual([ordinarySession]);
  });
});

describe("project session metadata", () => {
  it("switches to a project session through standard store side effects and exposes current session metadata", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    session.sessions = [
      {
        id: "regular-1",
        title: "Regular",
        profile: "fast",
        project_id: null,
        worktree_path: null,
        branch: null,
        visibility: null
      }
    ];
    projectStore.sessionsByProject = new Map([
      [
        "project-1",
        [
          {
            sessionId: "project-session-1",
            title: "Project task",
            profile: "slow",
            projectId: "project-1",
            worktreePath: "/repo/.worktrees/project-task",
            branch: "feat/project-task",
            visibility: "draft_hidden"
          }
        ]
      ]
    ]);
    session.projection.messages = [{ role: "user", content: "stale" }];

    await session.switchSession("project-session-1");

    expect(mockedInvoke).toHaveBeenCalledWith("switch_session", {
      sessionId: "project-session-1"
    });
    expect(session.currentSessionId).toBe("project-session-1");
    expect(session.currentProfile).toBe("slow");
    expect(session.projection.messages).toEqual([]);
    expect(session.currentSessionInfo?.project_id).toBe("project-1");
    expect(session.currentSessionInfo?.worktree_path).toBe("/repo/.worktrees/project-task");
    expect(session.currentSessionInfo?.branch).toBe("feat/project-task");
    expect(session.currentSessionInfo?.visibility).toBe("draft_hidden");
  });
});
