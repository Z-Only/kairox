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
import { useProjectStore, type ProjectSessionInfo } from "@/stores/project";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import { traceState } from "@/composables/useTraceStore";
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
  localStorage.clear();
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

    if (command === "refresh_config") {
      return Promise.resolve(null);
    }

    if (command === "get_profile_info") {
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
      deleted_at: null,
      visibility: null
    };
    const projectSession: SessionInfoResponse = {
      id: "s2",
      title: "Project Draft",
      profile: "fast",
      project_id: "p1",
      worktree_path: "/tmp/demo",
      branch: null,
      deleted_at: null,
      visibility: "draft_hidden"
    };

    expect(filterOrdinarySessions([ordinarySession, projectSession])).toEqual([ordinarySession]);
  });
});

describe("project session metadata", () => {
  it("starts an ordinary placeholder session without creating backend state", async () => {
    const session = useSessionStore();
    const workspaceUi = useWorkspaceUiStore();
    session.currentSessionId = "regular-1";
    session.projection.messages = [{ role: "user", content: "stale" }];
    workspaceUi.gitReviewContext = { sessionId: "regular-1", projectId: null };
    workspaceUi.gitReview = {
      kind: "ok",
      branch: "main",
      worktreePath: "/repo",
      message: null,
      changedFiles: ["README.md"],
      fileCount: 1,
      additions: 1,
      deletions: 0,
      staged: null,
      unstaged: null,
      untracked: null
    };
    workspaceUi.gitReviewError = "stale error";
    traceState.entries.push({
      id: "ctx-stale",
      kind: "tool",
      status: "completed",
      toolId: "context",
      title: "Context assembled",
      startedAt: Date.now(),
      expanded: false
    });

    await session.startOrdinaryDraftSession();

    expect(mockedInvoke).not.toHaveBeenCalledWith("start_session", expect.anything());
    expect(session.currentSessionId).toBeNull();
    expect(session.currentSessionInfo).toBeNull();
    expect(session.composerDraftKey).toBe("new-session:ordinary");
    expect(session.projection.messages).toEqual([]);
    expect(workspaceUi.gitReviewContext).toBeNull();
    expect(workspaceUi.gitReview).toBeNull();
    expect(workspaceUi.gitReviewError).toBeNull();
    expect(traceState.entries).toEqual([]);
    expect(JSON.parse(localStorage.getItem("kairox.last-workbench-state") ?? "{}")).toEqual({
      kind: "ordinary-draft",
      profile: "fast",
      reasoningEffort: null,
      approval: "on_request",
      sandboxJson: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    });
  });

  it("starts a project placeholder session with project metadata and an isolated draft key", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    mockedInvoke.mockImplementation((command) => {
      if (command === "refresh_config_for_project") {
        return Promise.resolve(null);
      }
      if (command === "get_profile_info") {
        return Promise.resolve([]);
      }
      if (command === "get_project_git_status") {
        return Promise.resolve({
          kind: "clean",
          branch: "main",
          worktree_path: "/repo",
          message: null
        });
      }
      return Promise.resolve(null);
    });

    await session.startProjectDraftSession("project-1");

    expect(mockedInvoke).not.toHaveBeenCalledWith("create_project_draft_session", {
      projectId: "project-1"
    });
    expect(session.currentSessionId).toBeNull();
    expect(session.currentSessionInfo?.project_id).toBe("project-1");
    expect(session.currentSessionInfo?.worktree_path).toBe("/repo");
    expect(session.currentSessionInfo?.branch).toBe("main");
    expect(session.composerDraftKey).toBe("new-session:project:project-1");

    session.setPendingProjectBranch("feat/chat");
    expect(session.currentSessionInfo?.branch).toBe("feat/chat");
    expect(JSON.parse(localStorage.getItem("kairox.last-workbench-state") ?? "{}")).toEqual({
      kind: "project-draft",
      projectId: "project-1",
      branch: "feat/chat",
      profile: "fast",
      reasoningEffort: null,
      approval: "on_request",
      sandboxJson: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    });
  });

  it("loads project metadata before opening a project placeholder from an empty project store", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    projectStore.projects = [];
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_projects") {
        return Promise.resolve([
          {
            project_id: "project-1",
            display_name: "Demo",
            root_path: "/repo",
            removed_at: null,
            sort_order: 0,
            expanded: true,
            path_exists: true
          }
        ]);
      }
      if (command === "refresh_config_for_project") {
        return Promise.resolve(null);
      }
      if (command === "get_profile_info") {
        return Promise.resolve([]);
      }
      if (command === "get_project_git_status") {
        return Promise.resolve({
          kind: "clean",
          branch: "main",
          worktree_path: "/repo",
          message: null
        });
      }
      return Promise.resolve(null);
    });

    await session.startProjectDraftSession("project-1");

    expect(mockedInvoke).toHaveBeenCalledWith("list_projects");
    expect(projectStore.projects[0]?.displayName).toBe("Demo");
    expect(session.currentSessionInfo?.project_id).toBe("project-1");
    expect(session.currentSessionInfo?.worktree_path).toBe("/repo");
    expect(session.currentSessionInfo?.branch).toBe("main");
  });

  it("recovers a persisted project placeholder and keeps its draft key", async () => {
    localStorage.setItem(
      "kairox.last-workbench-state",
      JSON.stringify({ kind: "project-draft", projectId: "project-1", branch: "feat/ui" })
    );
    const session = useSessionStore();
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_workspaces") {
        return Promise.resolve([{ workspace_id: "ws1", path: "/tmp" }]);
      }
      if (command === "restore_workspace") return Promise.resolve(null);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "list_projects") {
        return Promise.resolve([
          {
            project_id: "project-1",
            display_name: "Demo",
            root_path: "/repo",
            removed_at: null,
            sort_order: 0,
            expanded: true,
            path_exists: true
          }
        ]);
      }
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") return Promise.resolve([]);
      if (command === "get_project_git_status") {
        return Promise.resolve({
          kind: "clean",
          branch: "main",
          worktree_path: "/repo",
          message: null
        });
      }
      return Promise.resolve(null);
    });

    const recovered = await session.recoverSessions();

    expect(recovered).toBe(true);
    expect(session.currentSessionId).toBeNull();
    expect(session.currentSessionInfo?.project_id).toBe("project-1");
    expect(session.currentSessionInfo?.worktree_path).toBe("/repo");
    expect(session.currentSessionInfo?.branch).toBe("feat/ui");
    expect(session.composerDraftKey).toBe("new-session:project:project-1");
  });

  it("recovers persisted project draft settings before first send", async () => {
    const sandboxJson = '{"kind":"danger_full_access"}';
    localStorage.setItem(
      "kairox.last-workbench-state",
      JSON.stringify({
        kind: "project-draft",
        projectId: "project-1",
        branch: "main",
        profile: "ali-mo-claude",
        reasoningEffort: null,
        approval: "always",
        sandboxJson
      })
    );
    const session = useSessionStore();
    mockedInvoke.mockImplementation((command, args) => {
      if (command === "list_workspaces") {
        return Promise.resolve([{ workspace_id: "ws1", path: "/tmp" }]);
      }
      if (command === "restore_workspace") return Promise.resolve(null);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "list_projects") {
        return Promise.resolve([
          {
            project_id: "project-1",
            display_name: "Demo",
            root_path: "/repo",
            removed_at: null,
            sort_order: 0,
            expanded: true,
            path_exists: true
          }
        ]);
      }
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") {
        return Promise.resolve([
          {
            alias: "fake",
            provider: "fake",
            model_id: "fake",
            local: true,
            has_api_key: true
          },
          {
            alias: "ali-mo-claude",
            provider: "ali-mo",
            model_id: "claude-opus-4-6",
            local: false,
            has_api_key: true
          }
        ]);
      }
      if (command === "get_project_git_status") {
        return Promise.resolve({
          kind: "clean",
          branch: "main",
          worktree_path: "/repo",
          message: null
        });
      }
      if (command === "create_project_draft_session") {
        return Promise.resolve("draft-1");
      }
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
      if (command === "get_trace") return Promise.resolve([]);
      if (command === "switch_model") return Promise.resolve(null);
      if (command === "set_session_approval_policy") {
        return Promise.resolve((args as { approval: string }).approval);
      }
      if (command === "set_session_sandbox_policy") {
        return Promise.resolve((args as { sandboxJson: string }).sandboxJson);
      }
      return Promise.resolve(null);
    });

    const recovered = await session.recoverSessions();

    expect(recovered).toBe(true);
    expect(session.currentSessionId).toBeNull();
    expect(session.currentProfile).toBe("ali-mo-claude");
    expect(session.currentReasoningEffort).toBeNull();
    expect(session.approvalPolicy).toBe("always");
    expect(session.sandboxPolicy).toBe(sandboxJson);
    expect(session.currentSessionInfo?.profile).toBe("ali-mo-claude");
    expect(session.currentSessionInfo?.approval_policy).toBe("always");
    expect(session.currentSessionInfo?.sandbox_policy).toBe(sandboxJson);

    await session.ensureSessionForSend();

    expect(mockedInvoke).toHaveBeenCalledWith("switch_model", {
      sessionId: "draft-1",
      profileAlias: "ali-mo-claude"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("set_session_approval_policy", {
      approval: "always"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("set_session_sandbox_policy", {
      sandboxJson
    });
  });

  it("uses the recovered project profile list instead of the hardcoded default on first send", async () => {
    localStorage.setItem(
      "kairox.last-workbench-state",
      JSON.stringify({ kind: "project-draft", projectId: "project-1", branch: null })
    );
    const session = useSessionStore();
    mockedInvoke.mockImplementation((command, args) => {
      if (command === "list_workspaces") {
        return Promise.resolve([{ workspace_id: "ws1", path: "/tmp" }]);
      }
      if (command === "restore_workspace") return Promise.resolve(null);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "list_projects") {
        return Promise.resolve([
          {
            project_id: "project-1",
            display_name: "Demo",
            root_path: "/repo",
            removed_at: null,
            sort_order: 0,
            expanded: true,
            path_exists: true
          }
        ]);
      }
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") {
        return Promise.resolve([
          {
            alias: "ali-mo-claude",
            provider: "ali-mo",
            model_id: "claude-opus-4-6",
            local: false,
            has_api_key: true
          }
        ]);
      }
      if (command === "get_project_git_status") {
        return Promise.resolve({
          kind: "not_initialized",
          branch: null,
          worktree_path: "/repo",
          message: "not a git repository"
        });
      }
      if (command === "create_project_draft_session") {
        return Promise.resolve("draft-1");
      }
      if (command === "rename_session") return Promise.resolve(null);
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
      if (command === "get_trace") return Promise.resolve([]);
      if (command === "switch_model") return Promise.resolve(null);
      if (command === "set_session_approval_policy") {
        return Promise.resolve((args as { approval: string }).approval);
      }
      if (command === "set_session_sandbox_policy") {
        return Promise.resolve((args as { sandboxJson: string }).sandboxJson);
      }
      return Promise.resolve(null);
    });

    const recovered = await session.recoverSessions();
    await session.ensureSessionForSend();

    expect(recovered).toBe(true);
    expect(mockedInvoke).toHaveBeenCalledWith("switch_model", {
      sessionId: "draft-1",
      profileAlias: "ali-mo-claude"
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("switch_model", {
      sessionId: "draft-1",
      profileAlias: "fast"
    });
    expect(session.currentProfile).toBe("ali-mo-claude");
  });

  it("refreshes project config before showing a project draft model list", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    const calls: string[] = [];
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    mockedInvoke.mockImplementation((command) => {
      calls.push(command);
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") {
        return Promise.resolve([
          {
            alias: "tokensflow",
            provider: "openai_compatible",
            model_id: "tokensflow-chat",
            local: false,
            has_api_key: true
          }
        ]);
      }
      if (command === "get_project_git_status") {
        return Promise.resolve({
          kind: "clean",
          branch: "main",
          worktree_path: "/repo",
          message: null
        });
      }
      return Promise.resolve(null);
    });

    await session.startProjectDraftSession("project-1");

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_config_for_project", {
      projectRoot: "/repo"
    });
    expect(calls).toContain("get_profile_info");
    expect(session.profileInfos.map((profile) => profile.alias)).toEqual(["tokensflow"]);
  });

  it("refreshes global config for an ordinary historical session model list", async () => {
    const session = useSessionStore();
    const calls: string[] = [];
    session.sessions = [
      {
        id: "regular-1",
        title: "Regular",
        profile: "local",
        project_id: null,
        worktree_path: null,
        branch: null,
        deleted_at: null,
        visibility: null
      }
    ];
    session.currentSessionId = "regular-1";
    mockedInvoke.mockImplementation((command) => {
      calls.push(command);
      if (command === "refresh_config") return Promise.resolve(null);
      if (command === "get_profile_info") {
        return Promise.resolve([
          {
            alias: "tokensflow",
            provider: "openai_compatible",
            model_id: "tokensflow-chat",
            local: false,
            has_api_key: true
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.refreshProfileInfoForCurrentContext();

    expect(calls.slice(0, 2)).toEqual(["refresh_config", "get_profile_info"]);
    expect(session.profileInfos.map((profile) => profile.alias)).toEqual(["tokensflow"]);
  });

  it("refreshes project worktree config for a project historical session model list", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    const calls: Array<{ command: string; args?: unknown }> = [];
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    projectStore.sessionsByProject = new Map([
      [
        "project-1",
        [
          {
            sessionId: "project-session-1",
            title: "Project task",
            profile: "local",
            projectId: "project-1",
            worktreePath: "/repo/.worktrees/project-task",
            branch: "feat/project-task",
            deletedAt: null,
            visibility: "visible"
          }
        ]
      ]
    ]);
    session.currentSessionId = "project-session-1";
    mockedInvoke.mockImplementation((command, args) => {
      calls.push({ command, args });
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") {
        return Promise.resolve([
          {
            alias: "tokensflow",
            provider: "openai_compatible",
            model_id: "tokensflow-chat",
            local: false,
            has_api_key: true
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.refreshProfileInfoForCurrentContext();

    expect(calls[0]).toEqual({
      command: "refresh_config_for_project",
      args: { projectRoot: "/repo/.worktrees/project-task" }
    });
    expect(calls.map((entry) => entry.command)).toContain("get_profile_info");
    expect(calls.map((entry) => entry.command)).not.toContain("refresh_config");
    expect(session.profileInfos.map((profile) => profile.alias)).toEqual(["tokensflow"]);
  });

  it("materializes a project placeholder as a draft or worktree session on first send", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    vi.spyOn(projectStore, "getProjectGitStatus").mockResolvedValue({
      kind: "clean",
      branch: "main",
      worktreePath: "/repo",
      message: null
    });
    const createDraft = vi.spyOn(projectStore, "createProjectDraftSession").mockResolvedValue({
      sessionId: "draft-1",
      title: "New Session",
      profile: "fast",
      projectId: "project-1",
      worktreePath: "/repo",
      branch: "main",
      visibility: "draft_hidden",
      deletedAt: null
    });
    const createWorktree = vi
      .spyOn(projectStore, "createProjectWorktreeSession")
      .mockResolvedValue({
        sessionId: "wt-1",
        title: "New Session (feat/chat)",
        profile: "fast",
        projectId: "project-1",
        worktreePath: "/repo/.kairox/worktrees/feat-chat",
        branch: "feat/chat",
        visibility: "visible",
        deletedAt: null
      });

    await session.startProjectDraftSession("project-1");
    await session.ensureSessionForSend();
    expect(createDraft).toHaveBeenCalledWith("project-1");
    expect(session.currentSessionId).toBe("draft-1");

    await session.startProjectDraftSession("project-1");
    session.setPendingProjectBranch("feat/chat");
    await session.ensureSessionForSend();
    expect(createWorktree).toHaveBeenCalledWith("project-1", "feat/chat");
    expect(session.currentSessionId).toBe("wt-1");
  });

  it("refreshes current project session metadata from the project session list", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    const placeholder: ProjectSessionInfo = {
      sessionId: "draft-1",
      title: "New Session",
      profile: "ali-mo-claude",
      projectId: "project-1",
      worktreePath: "/repo",
      branch: "main",
      visibility: "draft_hidden",
      deletedAt: null,
      approvalPolicy: "always",
      sandboxPolicy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    };
    projectStore.sessionsByProject = new Map([["project-1", [placeholder]]]);
    session.currentSessionId = "draft-1";
    mockedInvoke.mockImplementation((command) => {
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") return Promise.resolve([]);
      if (command === "list_project_sessions") {
        return Promise.resolve([
          {
            id: "draft-1",
            title: "请严格按顺序调用工具验证工作区写入",
            profile: "ali-mo-claude",
            project_id: "project-1",
            worktree_path: "/repo",
            branch: "main",
            visibility: "visible",
            deleted_at: null,
            approval_policy: "always",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await (
      session as typeof session & {
        refreshCurrentSessionMetadata: () => Promise<void>;
      }
    ).refreshCurrentSessionMetadata();

    expect(mockedInvoke).toHaveBeenCalledWith("list_project_sessions", {
      projectId: "project-1"
    });
    expect(projectStore.sessionsByProject.get("project-1")?.[0].title).toBe(
      "请严格按顺序调用工具验证工作区写入"
    );
    expect(session.currentSessionInfo?.title).toBe("请严格按顺序调用工具验证工作区写入");
  });

  it("syncs the current ordinary session profile after loading session metadata", async () => {
    const session = useSessionStore();
    session.sessions = [
      {
        id: "ses_current",
        title: "Current",
        profile: "fast",
        project_id: null,
        worktree_path: null,
        branch: null,
        visibility: null,
        deleted_at: null,
        approval_policy: "on_request",
        sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
      }
    ];
    session.currentSessionId = "ses_current";
    session.currentProfile = "fast";
    session.currentReasoningEffort = "high";
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "ses_current",
            title: "Current",
            profile: "ali-mo-claude",
            project_id: null,
            worktree_path: null,
            branch: null,
            visibility: null,
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.loadSessions();

    expect(session.currentProfile).toBe("ali-mo-claude");
    expect(session.currentReasoningEffort).toBeNull();
    expect(session.currentSessionInfo?.profile).toBe("ali-mo-claude");
  });

  it("maps current metadata default profile to the first selectable profile", async () => {
    const session = useSessionStore();
    session.profileInfos = [
      {
        alias: "tokensflow",
        provider: "openai_compatible",
        model_id: "tokensflow-chat",
        local: false,
        has_api_key: true
      }
    ];
    session.currentSessionId = "ses_current";
    session.currentProfile = "fast";
    session.currentReasoningEffort = "high";
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "ses_current",
            title: "Current",
            profile: "default",
            project_id: null,
            worktree_path: null,
            branch: null,
            visibility: null,
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.loadSessions();

    expect(session.currentProfile).toBe("tokensflow");
    expect(session.currentReasoningEffort).toBeNull();
    expect(session.currentSessionInfo?.profile).toBe("default");
  });

  it("keeps reasoning effort when current metadata profile already matches", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_current";
    session.currentProfile = "fast";
    session.currentReasoningEffort = "high";
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "ses_current",
            title: "Current",
            profile: "fast",
            project_id: null,
            worktree_path: null,
            branch: null,
            visibility: null,
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.loadSessions();

    expect(session.currentProfile).toBe("fast");
    expect(session.currentReasoningEffort).toBe("high");
  });

  it("does not sync currentProfile from non-current ordinary session metadata", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_current";
    session.currentProfile = "fast";
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "ses_current",
            title: "Current",
            profile: "fast",
            project_id: null,
            worktree_path: null,
            branch: null,
            visibility: null,
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          },
          {
            id: "ses_other",
            title: "Other",
            profile: "ali-mo-claude",
            project_id: null,
            worktree_path: null,
            branch: null,
            visibility: null,
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.loadSessions();

    expect(session.currentProfile).toBe("fast");
  });

  it("syncs the current project session profile after refreshing project metadata", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    projectStore.sessionsByProject = new Map([
      [
        "project-1",
        [
          {
            sessionId: "project-session-1",
            title: "Project task",
            profile: "fast",
            projectId: "project-1",
            worktreePath: "/repo",
            branch: "main",
            visibility: "visible",
            deletedAt: null,
            approvalPolicy: "on_request",
            sandboxPolicy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]
      ]
    ]);
    session.currentSessionId = "project-session-1";
    session.currentProfile = "fast";
    session.currentReasoningEffort = "xhigh";
    mockedInvoke.mockImplementation((command) => {
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") return Promise.resolve([]);
      if (command === "list_project_sessions") {
        return Promise.resolve([
          {
            id: "project-session-1",
            title: "Project task",
            profile: "ali-mo-claude",
            project_id: "project-1",
            worktree_path: "/repo",
            branch: "main",
            visibility: "visible",
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.refreshCurrentSessionMetadata();

    expect(session.currentProfile).toBe("ali-mo-claude");
    expect(session.currentReasoningEffort).toBeNull();
    expect(session.currentSessionInfo?.profile).toBe("ali-mo-claude");
  });

  it("does not overwrite a pending draft model selection from background metadata loads", async () => {
    const session = useSessionStore();
    mockedInvoke.mockImplementation((command) => {
      if (command === "refresh_config") return Promise.resolve(null);
      if (command === "get_profile_info") return Promise.resolve([]);
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "ses_background",
            title: "Background",
            profile: "ali-mo-claude",
            project_id: null,
            worktree_path: null,
            branch: null,
            visibility: null,
            deleted_at: null,
            approval_policy: "on_request",
            sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await session.startOrdinaryDraftSession();
    session.setPendingModelSelection("draft-model", "high");
    await session.loadSessions();

    expect(session.currentSessionId).toBeNull();
    expect(session.currentProfile).toBe("draft-model");
    expect(session.currentReasoningEffort).toBe("high");
  });

  it("optimistically titles a materialized project session from the first message", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    projectStore.sessionsByProject = new Map([
      [
        "project-1",
        [
          {
            sessionId: "draft-1",
            title: "New Session",
            profile: "ali-mo-claude",
            projectId: "project-1",
            worktreePath: "/repo",
            branch: "main",
            visibility: "draft_hidden",
            deletedAt: null,
            approvalPolicy: "always",
            sandboxPolicy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
          }
        ]
      ]
    ]);
    session.currentSessionId = "draft-1";

    await session.refreshCurrentSessionMetadata(
      "请不要调用工具，直接用中文回复 TITLE-REFRESH-9B2C-PASS。"
    );

    const updated = projectStore.sessionsByProject.get("project-1")?.[0];
    expect(updated?.title).toBe("请不要调用工具，直接用中文回复 TITLE-REFRESH-9B2C-PASS。");
    expect(updated?.visibility).toBe("visible");
    expect(session.currentSessionInfo?.title).toBe(
      "请不要调用工具，直接用中文回复 TITLE-REFRESH-9B2C-PASS。"
    );
    expect(mockedInvoke).not.toHaveBeenCalledWith("list_project_sessions", {
      projectId: "project-1"
    });
  });

  it("optimistically titles an auto-titled ordinary session from the first message", async () => {
    const session = useSessionStore();
    session.sessions = [
      {
        id: "ses_1",
        title: "Session using fake",
        profile: "ali-mo-claude",
        project_id: null,
        worktree_path: null,
        branch: null,
        visibility: null,
        deleted_at: null,
        approval_policy: "on_request",
        sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
      }
    ];
    session.currentSessionId = "ses_1";

    await session.refreshCurrentSessionMetadata("hello from bootstrap session");

    expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
      sessionId: "ses_1",
      title: "hello from bootstrap session"
    });
    expect(session.sessions[0].title).toBe("hello from bootstrap session");
    expect(session.sessions[0].visibility).toBe("visible");
  });

  it("does not overwrite a user-titled ordinary session after later sends", async () => {
    const session = useSessionStore();
    session.sessions = [
      {
        id: "ses_1",
        title: "Release planning",
        profile: "ali-mo-claude",
        project_id: null,
        worktree_path: null,
        branch: null,
        visibility: "visible",
        deleted_at: null,
        approval_policy: "on_request",
        sandbox_policy: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
      }
    ];
    session.currentSessionId = "ses_1";

    await session.refreshCurrentSessionMetadata("second message should not rename");

    expect(mockedInvoke).not.toHaveBeenCalledWith("rename_session", {
      sessionId: "ses_1",
      title: "second message should not rename"
    });
    expect(session.sessions[0].title).toBe("Release planning");
    expect(session.sessions[0].visibility).toBe("visible");
  });

  it("applies pending model and policy selections when materializing a project session", async () => {
    const session = useSessionStore();
    const projectStore = useProjectStore();
    const readOnlySandbox = '{"kind":"read_only"}';
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Demo",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    vi.spyOn(projectStore, "getProjectGitStatus").mockResolvedValue({
      kind: "clean",
      branch: "main",
      worktreePath: "/repo",
      message: null
    });
    vi.spyOn(projectStore, "createProjectDraftSession").mockResolvedValue({
      sessionId: "draft-1",
      title: "New Session",
      profile: "fake",
      projectId: "project-1",
      worktreePath: "/repo",
      branch: "main",
      visibility: "draft_hidden",
      deletedAt: null
    });
    mockedInvoke.mockImplementation((command, args) => {
      if (command === "refresh_config_for_project") return Promise.resolve(null);
      if (command === "get_profile_info") return Promise.resolve([]);
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
      if (command === "get_trace") return Promise.resolve([]);
      if (command === "switch_model") return Promise.resolve(null);
      if (command === "set_session_approval_policy") {
        return Promise.resolve((args as { approval: string }).approval);
      }
      if (command === "set_session_sandbox_policy") {
        return Promise.resolve((args as { sandboxJson: string }).sandboxJson);
      }
      return Promise.resolve(null);
    });

    await session.startProjectDraftSession("project-1");
    session.currentProfile = "ali-mo-claude";
    session.currentReasoningEffort = "xhigh";
    await session.setApprovalPolicy("always");
    await session.setSandboxPolicy(readOnlySandbox);

    expect(mockedInvoke).not.toHaveBeenCalledWith("set_session_approval_policy", {
      approval: "always"
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("set_session_sandbox_policy", {
      sandboxJson: readOnlySandbox
    });

    await session.ensureSessionForSend();

    expect(mockedInvoke).toHaveBeenCalledWith("switch_model", {
      sessionId: "draft-1",
      profileAlias: "ali-mo-claude",
      reasoningEffort: "xhigh"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("set_session_approval_policy", {
      approval: "always"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("set_session_sandbox_policy", {
      sandboxJson: readOnlySandbox
    });
    expect(session.currentProfile).toBe("ali-mo-claude");
    expect(session.currentReasoningEffort).toBe("xhigh");
    expect(session.approvalPolicy).toBe("always");
    expect(session.sandboxPolicy).toBe(readOnlySandbox);
  });

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
        deleted_at: null,
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
            deletedAt: null,
            visibility: "draft_hidden",
            approvalPolicy: "always",
            sandboxPolicy: '{"kind":"danger_full_access"}'
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
    expect(session.approvalPolicy).toBe("always");
    expect(session.sandboxPolicy).toBe('{"kind":"danger_full_access"}');
    expect(session.projection.messages).toEqual([]);
    expect(session.currentSessionInfo?.project_id).toBe("project-1");
    expect(session.currentSessionInfo?.worktree_path).toBe("/repo/.worktrees/project-task");
    expect(session.currentSessionInfo?.branch).toBe("feat/project-task");
    expect(session.currentSessionInfo?.visibility).toBe("draft_hidden");
    expect(JSON.parse(localStorage.getItem("kairox.last-workbench-state") ?? "{}")).toEqual({
      kind: "session",
      sessionId: "project-session-1",
      projectId: "project-1"
    });
  });
});
