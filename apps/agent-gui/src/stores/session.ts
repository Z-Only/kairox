// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore` and `ref` explicitly.
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  SessionProjection,
  SessionInfoResponse,
  DomainEvent,
  ContextUsage,
  ProjectedModelLimits,
  ProfileInfo
} from "@/types";
import { agentRoleToProjectedRole } from "@/types";
import { clearTrace, applyTraceEvent } from "@/composables/useTraceStore";
import { useUiStore } from "@/stores/ui";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useAgentsStore } from "@/stores/agents";
import { useProjectStore, type ProjectSessionInfo } from "@/stores/project";

function emptyProjection(): SessionProjection {
  return {
    messages: [],
    task_titles: [],
    task_graph: { tasks: [] },
    token_stream: "",
    cancelled: false,
    last_context_usage: null,
    model_limits: null,
    compaction: { type: "Idle" }
  };
}

export function uniqueSessionTitle(base: string, existingTitles: string[]): string {
  if (!existingTitles.includes(base)) return base;
  let n = 1;
  while (existingTitles.includes(`${base} ${n}`)) {
    n++;
  }
  return `${base} ${n}`;
}

export function temporaryTitleFromFirstMessage(content: string): string {
  const trimmedContent = content.trim();
  if (!trimmedContent) return "New Session";

  const maxLength = 48;
  return trimmedContent.length > maxLength
    ? `${trimmedContent.slice(0, maxLength)}…`
    : trimmedContent;
}

function titleCaseWords(value: string): string {
  return value
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((word) => {
      const lower = word.toLowerCase();
      if (lower === "gpt") return "GPT";
      if (lower === "ai") return "AI";
      if (lower === "openai") return "OpenAI";
      return `${lower.charAt(0).toUpperCase()}${lower.slice(1)}`;
    })
    .join(" ");
}

function formatModelIdForDisplay(modelId: string): string {
  const parts = modelId.split("-").filter(Boolean);
  if (parts.length === 0) return modelId;

  const [family, ...restParts] = parts;
  const lowerFamily = family.toLowerCase();
  if (lowerFamily === "gpt" && restParts.length > 0) {
    const [version, ...suffixParts] = restParts;
    return [`GPT-${version}`, ...suffixParts.map(titleCaseWords)].join(" ");
  }

  if (
    lowerFamily === "claude" &&
    restParts.length >= 3 &&
    /^\d+$/.test(restParts[0]) &&
    /^\d+$/.test(restParts[1])
  ) {
    const [majorVersion, minorVersion, ...suffixParts] = restParts;
    return [`Claude ${majorVersion}.${minorVersion}`, ...suffixParts.map(titleCaseWords)].join(" ");
  }

  return parts.map(titleCaseWords).join(" ");
}

export function formatProfileDisplay(profile: ProfileInfo): string {
  if (profile.provider && profile.model_id) {
    return `${titleCaseWords(profile.provider)} · ${formatModelIdForDisplay(profile.model_id)}`;
  }
  if (profile.model_id) return formatModelIdForDisplay(profile.model_id);
  return profile.alias;
}

export function filterOrdinarySessions(sessionList: SessionInfoResponse[]): SessionInfoResponse[] {
  return sessionList.filter((session) => !session.project_id);
}

async function listOrdinarySessions(): Promise<SessionInfoResponse[]> {
  const sessionList = await invoke<SessionInfoResponse[]>("list_sessions");
  return filterOrdinarySessions(sessionList);
}

function normalizeProjectSessionInfo(projectSession: ProjectSessionInfo): SessionInfoResponse {
  return {
    id: projectSession.sessionId,
    title: projectSession.title,
    profile: projectSession.profile,
    project_id: projectSession.projectId,
    worktree_path: projectSession.worktreePath,
    branch: projectSession.branch,
    visibility: projectSession.visibility
  };
}

export const useSessionStore = defineStore("session", () => {
  // ── state ────────────────────────────────────────────────────────
  const sessions = ref<SessionInfoResponse[]>([]);
  const currentSessionId = ref<string | null>(null);
  const workspaceId = ref<string | null>(null);
  const projection = ref<SessionProjection>(emptyProjection());
  const currentProfile = ref<string>("fast");
  const lastContextUsage = ref<ContextUsage | null>(null);
  const modelLimits = ref<ProjectedModelLimits | null>(null);
  const compacting = ref(false);
  const lastCompactionError = ref<string | null>(null);
  const isStreaming = ref(false);
  const connected = ref(false);
  const initialized = ref(false);
  const streamsByTask = ref(new Map<string, string>());
  const profileInfos = ref<ProfileInfo[]>([]);
  const loadingProfileInfo = ref(false);
  const lastSendError = ref<string | null>(null);
  const permissionMode = ref<string>("suggest");

  function resolveSessionProfile(profile?: string): string {
    return profile ?? currentProfile.value;
  }

  function findProjectSessionInfo(sessionId: string): SessionInfoResponse | undefined {
    const projectStore = useProjectStore();
    for (const projectSessions of projectStore.sessionsByProject.values()) {
      const projectSession = projectSessions.find((entry) => entry.sessionId === sessionId);
      if (projectSession) return normalizeProjectSessionInfo(projectSession);
    }

    const archivedSession = projectStore.archivedSessions.find(
      (entry) => entry.sessionId === sessionId
    );
    return archivedSession ? normalizeProjectSessionInfo(archivedSession) : undefined;
  }

  function findSessionInfo(sessionId: string): SessionInfoResponse | undefined {
    return (
      sessions.value.find((session) => session.id === sessionId) ??
      findProjectSessionInfo(sessionId)
    );
  }

  const currentSessionInfo = computed<SessionInfoResponse | null>(() => {
    if (!currentSessionId.value) return null;
    return findSessionInfo(currentSessionId.value) ?? null;
  });

  const activeProfileInfo = computed(() =>
    profileInfos.value.find((profile) => profile.alias === currentProfile.value)
  );

  const activeProfileDisplay = computed(() => {
    const profile = activeProfileInfo.value;
    if (!profile) {
      const firstProfile = profileInfos.value[0];
      if (firstProfile) return formatProfileDisplay(firstProfile);
      return currentProfile.value;
    }
    return formatProfileDisplay(profile);
  });

  // ── actions ──────────────────────────────────────────────────────
  function reportSendError(message: string) {
    lastSendError.value = message;
    projection.value.messages.push({
      role: "assistant",
      content: `[error] ${message}`
    });
    projection.value.token_stream = "";
    isStreaming.value = false;
  }

  function applyEvent(event: DomainEvent) {
    const p = event.payload;
    const sourceAgentId = event.source_agent_id;
    const agents = useAgentsStore();

    switch (p.type) {
      case "UserMessageAdded": {
        lastSendError.value = null;
        projection.value.messages.push({
          role: "user",
          content: p.content
        });
        isStreaming.value = true;
        break;
      }
      case "ModelTokenDelta": {
        projection.value.token_stream += p.delta;
        break;
      }
      case "AssistantMessageCompleted": {
        const msg: (typeof projection.value.messages)[0] = {
          role: "assistant",
          content: p.content
        };
        if (sourceAgentId && sourceAgentId !== "agent_system") {
          msg.sourceAgentId = sourceAgentId;
          const agent = agents.agents.get(sourceAgentId);
          if (agent) {
            msg.role = agentRoleToProjectedRole(agent.role);
          }
        }
        projection.value.messages.push(msg);
        projection.value.token_stream = "";
        isStreaming.value = false;
        break;
      }
      case "SessionCancelled":
        projection.value.cancelled = true;
        isStreaming.value = false;
        break;
      case "AgentTaskCreated": {
        projection.value.task_titles.push(p.title);
        break;
      }
      case "AgentTaskStarted":
        break;
      case "AgentTaskCompleted": {
        isStreaming.value = false;
        break;
      }
      case "AgentTaskFailed": {
        projection.value.messages.push({
          role: "assistant",
          content: `[error] ${p.error || "Unknown error"}`
        });
        projection.value.token_stream = "";
        isStreaming.value = false;
        break;
      }
      case "TaskDecomposed": {
        projection.value.messages.push({
          role: "system",
          content: `Task decomposed into ${p.sub_task_ids.length} sub-tasks`
        });
        break;
      }
      case "TaskBlocked": {
        projection.value.messages.push({
          role: "system",
          content: `Task blocked: ${p.reason || "dependency failed"}`
        });
        break;
      }
      case "TaskRetried": {
        projection.value.messages.push({
          role: "system",
          content: `Task retry attempt ${p.attempt}`
        });
        break;
      }
      case "ContextAssembled": {
        lastContextUsage.value = p.usage;
        break;
      }
      case "ContextCompactionStarted": {
        compacting.value = true;
        lastCompactionError.value = null;
        break;
      }
      case "ContextCompactionCompleted": {
        compacting.value = false;
        break;
      }
      case "ContextCompactionFailed": {
        compacting.value = false;
        lastCompactionError.value = p.error;
        break;
      }
      case "ModelProfileSwitched": {
        currentProfile.value = p.to_profile;
        modelLimits.value = {
          context_window: p.context_window,
          output_limit: p.output_limit,
          source: p.limit_source
        };
        break;
      }
      case "AgentSpawned":
      case "AgentIdle":
        break;
      case "SessionInitialized":
      case "ModelRequestStarted":
      case "ModelToolCallRequested":
      case "ToolInvocationStarted":
      case "ToolInvocationCompleted":
      case "ToolInvocationFailed":
      case "PermissionRequested":
      case "PermissionGranted":
      case "PermissionDenied":
      case "FilePatchProposed":
      case "FilePatchApplied":
      case "MemoryProposed":
      case "MemoryAccepted":
      case "MemoryRejected":
      case "ReviewerFindingAdded":
      case "WorkspaceOpened":
        break;
    }
  }

  function setProjection(next: SessionProjection) {
    projection.value = next;
    isStreaming.value = false;
    if (next.task_graph?.tasks) {
      useTaskGraphStore().setTaskGraph(next.task_graph.tasks, currentSessionId.value);
    }
    // P3: hydrate context refs from the projection snapshot. The three P3
    // fields are `#[serde(default)]` on the Rust side (see
    // `crates/agent-core/src/projection.rs`), so they may be missing when a
    // legacy backend / test fixture sends a pre-P3 shape. Treat any missing
    // value as the same default the Rust side would emit.
    lastContextUsage.value = next.last_context_usage ?? null;
    modelLimits.value = next.model_limits ?? null;
    const status = next.compaction ?? { type: "Idle" };
    compacting.value = status.type === "Running";
    lastCompactionError.value = status.type === "Failed" ? status.error : null;
  }

  function resetProjection() {
    projection.value = emptyProjection();
    lastSendError.value = null;
    isStreaming.value = false;
    streamsByTask.value.clear();
    useAgentsStore().clearAgents();
    // P3: clear context refs.
    lastContextUsage.value = null;
    modelLimits.value = null;
    compacting.value = false;
    lastCompactionError.value = null;
  }

  async function switchToKnownSession(
    sessionId: string,
    target: SessionInfoResponse
  ): Promise<void> {
    if (sessionId === currentSessionId.value) return;
    resetProjection();
    clearTrace();
    useTaskGraphStore().clearTaskGraph();
    const next = await invoke<SessionProjection>("switch_session", {
      sessionId
    });
    currentSessionId.value = sessionId;
    localStorage.setItem("kairox.last-active-session-id", sessionId);
    currentProfile.value = target.profile;
    if (target.permission_mode) {
      permissionMode.value = target.permission_mode;
    }
    if (currentProfile.value === "default" && profileInfos.value.length > 0) {
      currentProfile.value = profileInfos.value[0].alias;
    }
    setProjection(next);
    const traceStrings = await invoke<string[]>("get_trace", { sessionId });
    for (const jsonStr of traceStrings) {
      try {
        applyTraceEvent(JSON.parse(jsonStr));
      } catch {
        // Skip malformed trace entries
      }
    }
  }

  async function switchSession(sessionId: string): Promise<void> {
    const target = findSessionInfo(sessionId);
    if (!target) {
      throw new Error(`Session not found: ${sessionId}`);
    }
    await switchToKnownSession(sessionId, target);
  }

  async function switchProjectSession(projectSession: ProjectSessionInfo): Promise<void> {
    await switchToKnownSession(
      projectSession.sessionId,
      normalizeProjectSessionInfo(projectSession)
    );
  }

  /**
   * Start a new session via the Tauri backend and reset projection state so
   * the workbench is clean before the caller navigates to the new session.
   *
   * Owns the post-create side-effects (`currentProfile = result.profile`,
   * `resetProjection()`, global `clearTrace()`) so the view layer only has
   * to call `router.push({ name: 'workbench', params: { sessionId } })`
   * with the returned id and never touches projection / trace state
   * directly. Throws on backend failure so the view can surface it.
   */
  async function createSession(
    profile?: string,
    permissionModeParam?: string
  ): Promise<{ id: string; title: string; profile: string }> {
    const result = await invoke<{ id: string; title: string; profile: string }>("start_session", {
      profile: resolveSessionProfile(profile),
      permissionMode: permissionModeParam ?? permissionMode.value
    });

    // Dedup: check all existing sessions (excluding the one just created)
    sessions.value = await listOrdinarySessions();
    const existingTitles = sessions.value.filter((s) => s.id !== result.id).map((s) => s.title);
    const title = uniqueSessionTitle("New Session", existingTitles);

    // Persist the deduped title if different from default
    if (title !== result.title) {
      try {
        await invoke("rename_session", { sessionId: result.id, title });
      } catch (e) {
        console.error("Failed to set deduped session title:", e);
      }
    }

    currentProfile.value = result.profile;
    resetProjection();
    clearTrace();
    useTaskGraphStore().clearTaskGraph();
    return { id: result.id, title, profile: result.profile };
  }

  async function deleteSession(sessionId: string) {
    const ui = useUiStore();
    try {
      await invoke("delete_session", { sessionId });
      sessions.value = sessions.value.filter((s) => s.id !== sessionId);
      if (currentSessionId.value === sessionId) {
        if (sessions.value.length > 0) {
          await switchSession(sessions.value[0].id);
        } else {
          currentSessionId.value = null;
          resetProjection();
          clearTrace();
          useTaskGraphStore().clearTaskGraph();
        }
      }
    } catch (e) {
      console.error("Failed to delete session:", e);
      ui.pushNotification("error", `Failed to delete session: ${e}`);
    }
  }

  async function renameSession(sessionId: string, title: string) {
    const ui = useUiStore();
    try {
      await invoke("rename_session", { sessionId, title });
      const session = sessions.value.find((s) => s.id === sessionId);
      if (session) {
        session.title = title;
      }
    } catch (e) {
      console.error("Failed to rename session:", e);
      ui.pushNotification("error", `Failed to rename session: ${e}`);
    }
  }

  /**
   * First-run workspace initialization: create a workspace via the Tauri
   * backend, persist its id, and seed the session list. Called by App.vue
   * when `recoverSessions()` returns false.
   *
   * Idempotent — safe to call on HMR re-mounts.
   */
  async function initializeWorkspace(): Promise<void> {
    if (initialized.value) return;
    const ui = useUiStore();
    try {
      const workspaceInfo: { workspace_id: string; path: string } =
        await invoke("initialize_workspace");
      const sessionList = await listOrdinarySessions();
      workspaceId.value = workspaceInfo.workspace_id;
      sessions.value = sessionList;
      initialized.value = true;
      if (sessions.value.length > 0) {
        try {
          const lastActiveId = localStorage.getItem("kairox.last-active-session-id");
          const targetId =
            lastActiveId && sessions.value.some((s) => s.id === lastActiveId)
              ? lastActiveId
              : sessions.value[0].id;
          await switchSession(targetId);
        } catch {
          // Initial session may have minimal data — non-critical.
        }
      }
    } catch (e) {
      console.error("Failed to initialize workspace:", e);
      ui.pushNotification("error", `Failed to initialize workspace: ${e}`);
    }
  }

  /**
   * Set the Tauri event-listener connection state.
   * Used by useTauriEvents.ts so writes go through the store boundary
   * instead of mutating session.connected from outside the store.
   */
  function setConnected(value: boolean): void {
    connected.value = value;
  }

  async function loadProfileInfo(): Promise<void> {
    if (loadingProfileInfo.value) return;
    loadingProfileInfo.value = true;
    try {
      profileInfos.value = await invoke<ProfileInfo[]>("get_profile_info");
    } catch (error) {
      console.error("Failed to load profile info:", error);
    } finally {
      loadingProfileInfo.value = false;
    }
  }

  async function setPermissionMode(mode: string): Promise<void> {
    const ui = useUiStore();
    try {
      const result: string = await invoke("set_permission_mode", { mode });
      permissionMode.value = result;
    } catch (e) {
      console.error("Failed to set permission mode:", e);
      ui.pushNotification("error", `Failed to set permission mode: ${e}`);
    }
  }

  async function recoverSessions(): Promise<boolean> {
    const ui = useUiStore();
    try {
      const workspaces: { workspace_id: string; path: string }[] = await invoke("list_workspaces");
      if (workspaces.length === 0) {
        return false;
      }
      const ws = workspaces[0];
      workspaceId.value = ws.workspace_id;
      await invoke("restore_workspace", { workspaceId: ws.workspace_id });
      sessions.value = await listOrdinarySessions();
      if (sessions.value.length > 0) {
        const lastActiveId = localStorage.getItem("kairox.last-active-session-id");
        const targetId =
          lastActiveId && sessions.value.some((s) => s.id === lastActiveId)
            ? lastActiveId
            : sessions.value[0].id;
        await switchSession(targetId);
      }
      initialized.value = true;
      return true;
    } catch (e) {
      console.error("Failed to recover sessions:", e);
      ui.pushNotification("error", `Failed to recover sessions: ${e}`);
      return false;
    }
  }

  return {
    // state
    sessions,
    currentSessionId,
    workspaceId,
    projection,
    currentProfile,
    lastContextUsage,
    modelLimits,
    compacting,
    lastCompactionError,
    isStreaming,
    connected,
    initialized,
    streamsByTask,
    profileInfos,
    loadingProfileInfo,
    lastSendError,
    permissionMode,
    currentSessionInfo,
    activeProfileInfo,
    activeProfileDisplay,
    findSessionInfo,
    // actions
    reportSendError,
    applyEvent,
    setProjection,
    resetProjection,
    switchSession,
    switchProjectSession,
    createSession,
    deleteSession,
    renameSession,
    initializeWorkspace,
    loadProfileInfo,
    recoverSessions,
    setConnected,
    setPermissionMode
  };
});
