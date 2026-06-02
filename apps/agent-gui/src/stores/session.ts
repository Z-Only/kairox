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
import { useUiStore } from "@/stores/ui";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useAgentsStore } from "@/stores/agents";
import { useProjectStore, type ProjectSessionInfo } from "@/stores/project";
import { clearTrace } from "@/composables/useTraceStore";
import {
  emptyProjection,
  applySessionEvent,
  setProjectionFromSnapshot,
  resetProjectionState,
  appendAssistantErrorMessage,
  type EventReducerContext
} from "@/stores/sessionEvents";
import {
  switchToKnownSession as switchToKnownSessionImpl,
  createSession as createSessionImpl,
  deleteSession as deleteSessionImpl,
  renameSession as renameSessionImpl,
  type SessionActionDeps
} from "@/stores/sessionActions";

export const DEFAULT_REASONING_EFFORTS = ["low", "middle", "high", "xhigh"] as const;
const LAST_WORKBENCH_STATE_KEY = "kairox.last-workbench-state";

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
    visibility: projectSession.visibility,
    deleted_at: projectSession.deletedAt
  };
}

type PendingSessionDraft =
  | { kind: "ordinary" }
  | { kind: "project"; projectId: string; branch: string | null };

type PendingSessionSettings = {
  profile: string;
  reasoningEffort: string | null;
  approval: string;
  sandboxJson: string;
};

type PersistedWorkbenchState =
  | { kind: "session"; sessionId: string; projectId: string | null }
  | { kind: "ordinary-draft" }
  | { kind: "project-draft"; projectId: string; branch: string | null };

type LoadProfileInfoOptions = {
  force?: boolean;
  refreshConfig?: boolean;
};

function readPersistedWorkbenchState(): PersistedWorkbenchState | null {
  try {
    const raw = globalThis.localStorage?.getItem(LAST_WORKBENCH_STATE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<PersistedWorkbenchState>;
    if (parsed.kind === "ordinary-draft") return { kind: "ordinary-draft" };
    if (parsed.kind === "project-draft" && typeof parsed.projectId === "string") {
      return {
        kind: "project-draft",
        projectId: parsed.projectId,
        branch: typeof parsed.branch === "string" && parsed.branch.trim() ? parsed.branch : null
      };
    }
    if (parsed.kind === "session" && typeof parsed.sessionId === "string") {
      return {
        kind: "session",
        sessionId: parsed.sessionId,
        projectId: typeof parsed.projectId === "string" ? parsed.projectId : null
      };
    }
  } catch {
    // Best-effort restore only.
  }
  return null;
}

function writePersistedWorkbenchState(state: PersistedWorkbenchState): void {
  try {
    globalThis.localStorage?.setItem(LAST_WORKBENCH_STATE_KEY, JSON.stringify(state));
  } catch {
    // Best-effort persistence only.
  }
}

export const useSessionStore = defineStore("session", () => {
  // ── state ────────────────────────────────────────────────────────
  const sessions = ref<SessionInfoResponse[]>([]);
  const currentSessionId = ref<string | null>(null);
  const workspaceId = ref<string | null>(null);
  const projection = ref<SessionProjection>(emptyProjection());
  const currentProfile = ref<string>("fast");
  const currentReasoningEffort = ref<string | null>(null);
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
  let profileInfoLoad: Promise<void> | null = null;
  const lastSendError = ref<string | null>(null);
  const approvalPolicy = ref<string>("on_request");
  const sandboxPolicy = ref<string>(
    '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
  );
  const pendingSessionDraft = ref<PendingSessionDraft | null>(null);

  const eventCtx: EventReducerContext = {
    projection,
    isStreaming,
    lastSendError,
    lastContextUsage,
    compacting,
    lastCompactionError,
    currentProfile,
    currentReasoningEffort,
    modelLimits
  };

  const sessionActionDeps: SessionActionDeps = {
    sessions,
    currentSessionId,
    currentProfile,
    currentReasoningEffort,
    approvalPolicy,
    sandboxPolicy,
    profileInfos,
    resetProjection() {
      resetProjectionState(eventCtx, useAgentsStore(), streamsByTask);
    },
    setProjection(next: SessionProjection) {
      setProjectionFromSnapshot(next, eventCtx, useTaskGraphStore(), currentSessionId.value);
    },
    getTaskGraphStore() {
      return useTaskGraphStore();
    },
    getUiStore() {
      return useUiStore();
    }
  };

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

  function capturePendingSessionSettings(): PendingSessionSettings {
    return {
      profile: currentProfile.value,
      reasoningEffort: currentReasoningEffort.value,
      approval: approvalPolicy.value,
      sandboxJson: sandboxPolicy.value
    };
  }

  function updateSessionProfile(sessionId: string, profile: string): void {
    const session = sessions.value.find((entry) => entry.id === sessionId);
    if (session) {
      session.profile = profile;
      return;
    }

    const projectStore = useProjectStore();
    let changed = false;
    const nextSessionsByProject = new Map(projectStore.sessionsByProject);
    for (const [projectId, projectSessions] of nextSessionsByProject.entries()) {
      const nextSessions = projectSessions.map((projectSession) => {
        if (projectSession.sessionId !== sessionId) return projectSession;
        changed = true;
        return { ...projectSession, profile };
      });
      if (changed) {
        nextSessionsByProject.set(projectId, nextSessions);
        break;
      }
    }
    if (changed) {
      projectStore.sessionsByProject = nextSessionsByProject;
    }
  }

  async function persistCurrentModelSelection(settings: PendingSessionSettings): Promise<void> {
    const sessionId = currentSessionId.value;
    if (!sessionId) {
      throw new Error("No active session");
    }
    const payload: {
      sessionId: string;
      profileAlias: string;
      reasoningEffort?: string;
    } = {
      sessionId,
      profileAlias: settings.profile
    };
    if (settings.reasoningEffort) {
      payload.reasoningEffort = settings.reasoningEffort;
    }
    await invoke("switch_model", payload);
    currentProfile.value = settings.profile;
    currentReasoningEffort.value = settings.reasoningEffort;
    updateSessionProfile(sessionId, settings.profile);
  }

  async function persistApprovalPolicy(approval: string): Promise<string> {
    const result: string = await invoke("set_session_approval_policy", { approval });
    approvalPolicy.value = result;
    const session = sessions.value.find((s) => s.id === currentSessionId.value);
    if (session) session.approval_policy = result;
    return result;
  }

  async function persistSandboxPolicy(sandboxJson: string): Promise<string> {
    const result: string = await invoke("set_session_sandbox_policy", { sandboxJson });
    sandboxPolicy.value = result;
    const session = sessions.value.find((s) => s.id === currentSessionId.value);
    if (session) session.sandbox_policy = result;
    return result;
  }

  async function applyPendingSessionSettings(settings: PendingSessionSettings): Promise<void> {
    await persistCurrentModelSelection(settings);
    await persistApprovalPolicy(settings.approval);
    await persistSandboxPolicy(settings.sandboxJson);
  }

  const currentSessionInfo = computed<SessionInfoResponse | null>(() => {
    if (!currentSessionId.value) {
      const pending = pendingSessionDraft.value;
      if (pending?.kind !== "project") return null;
      const projectStore = useProjectStore();
      const project = projectStore.projects.find((entry) => entry.projectId === pending.projectId);
      return {
        id: `new-project-session:${pending.projectId}`,
        title: "New Session",
        profile: currentProfile.value,
        approval_policy: approvalPolicy.value,
        sandbox_policy: sandboxPolicy.value,
        project_id: pending.projectId,
        worktree_path: project?.rootPath ?? null,
        branch: pending.branch,
        visibility: "draft_hidden",
        deleted_at: null
      };
    }
    return findSessionInfo(currentSessionId.value) ?? null;
  });

  const composerDraftKey = computed<string | null>(() => {
    if (currentSessionId.value) return currentSessionId.value;
    const pending = pendingSessionDraft.value;
    if (!pending) return null;
    if (pending.kind === "ordinary") return "new-session:ordinary";
    return `new-session:project:${pending.projectId}`;
  });

  const activeProfileInfo = computed(() =>
    profileInfos.value.find((profile) => profile.alias === currentProfile.value)
  );

  const sortedProfileInfos = computed(() =>
    [...profileInfos.value].sort((a, b) => a.alias.localeCompare(b.alias))
  );

  const activeProfileDisplay = computed(() => {
    const profile = activeProfileInfo.value;
    if (!profile) {
      const firstProfile = sortedProfileInfos.value[0];
      if (firstProfile) return formatProfileDisplay(firstProfile);
      return currentProfile.value;
    }
    const display = formatProfileDisplay(profile);
    if (!profile.supports_reasoning) return display;
    return currentReasoningEffort.value ? `${display} · ${currentReasoningEffort.value}` : display;
  });

  // ── actions ──────────────────────────────────────────────────────
  function reportSendError(message: string) {
    lastSendError.value = message;
    appendAssistantErrorMessage(projection.value, message);
    projection.value.token_stream = "";
    isStreaming.value = false;
  }

  function applyEvent(event: DomainEvent) {
    applySessionEvent(event, eventCtx, useAgentsStore());
  }

  function setProjection(next: SessionProjection) {
    setProjectionFromSnapshot(next, eventCtx, useTaskGraphStore(), currentSessionId.value);
  }

  function resetProjection() {
    resetProjectionState(eventCtx, useAgentsStore(), streamsByTask);
  }

  async function switchToKnownSession(
    sessionId: string,
    target: SessionInfoResponse
  ): Promise<void> {
    pendingSessionDraft.value = null;
    await switchToKnownSessionImpl(sessionId, target, sessionActionDeps);
    writePersistedWorkbenchState({
      kind: "session",
      sessionId,
      projectId: target.project_id ?? null
    });
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
    profile?: string
  ): Promise<{ id: string; title: string; profile: string }> {
    const result = await createSessionImpl(profile, sessionActionDeps);
    pendingSessionDraft.value = null;
    writePersistedWorkbenchState({
      kind: "session",
      sessionId: result.id,
      projectId: null
    });
    return result;
  }

  function resetForPendingDraft(nextDraft: PendingSessionDraft): void {
    pendingSessionDraft.value = nextDraft;
    currentSessionId.value = null;
    resetProjection();
    clearTrace();
    useTaskGraphStore().clearTaskGraph();
    rememberPendingDraft();
  }

  function selectAvailableDraftProfile(): void {
    if (profileInfos.value.length === 0) return;
    if (profileInfos.value.some((profile) => profile.alias === currentProfile.value)) return;
    currentProfile.value = profileInfos.value[0].alias;
    currentReasoningEffort.value = null;
  }

  async function startOrdinaryDraftSession(): Promise<void> {
    resetForPendingDraft({ kind: "ordinary" });
    await loadProfileInfo({ refreshConfig: true });
    selectAvailableDraftProfile();
  }

  async function startProjectDraftSession(projectId: string): Promise<void> {
    resetForPendingDraft({ kind: "project", projectId, branch: null });
    const projectStore = useProjectStore();
    await projectStore.refreshProjectConfig(projectId);
    selectAvailableDraftProfile();
    try {
      const status = await projectStore.getProjectGitStatus(projectId);
      setPendingProjectBranch(status.branch);
    } catch {
      // Non-git projects can still open a placeholder chat.
    }
  }

  function setPendingProjectBranch(branch: string | null): void {
    const pending = pendingSessionDraft.value;
    if (pending?.kind !== "project") return;
    pendingSessionDraft.value = {
      ...pending,
      branch: branch?.trim() || null
    };
    rememberPendingDraft();
  }

  function rememberPendingDraft(): void {
    const pending = pendingSessionDraft.value;
    if (!pending) return;
    if (pending.kind === "ordinary") {
      writePersistedWorkbenchState({ kind: "ordinary-draft" });
      return;
    }
    writePersistedWorkbenchState({
      kind: "project-draft",
      projectId: pending.projectId,
      branch: pending.branch
    });
  }

  async function restorePersistedWorkbenchState(): Promise<boolean> {
    const persisted = readPersistedWorkbenchState();
    if (!persisted) return false;

    if (persisted.kind === "ordinary-draft") {
      await startOrdinaryDraftSession();
      return true;
    }

    const projectStore = useProjectStore();
    if (persisted.kind === "project-draft") {
      if (projectStore.projects.length === 0) {
        await projectStore.loadProjects();
      }
      if (!projectStore.projects.some((project) => project.projectId === persisted.projectId)) {
        return false;
      }
      await startProjectDraftSession(persisted.projectId);
      if (persisted.branch) {
        setPendingProjectBranch(persisted.branch);
      }
      return true;
    }

    if (persisted.projectId) {
      if (projectStore.projects.length === 0) {
        await projectStore.loadProjects();
      }
      if (projectStore.projects.some((project) => project.projectId === persisted.projectId)) {
        await projectStore.loadProjectSessions(persisted.projectId);
      }
    }

    const target = findSessionInfo(persisted.sessionId);
    if (!target) return false;
    await switchToKnownSession(persisted.sessionId, target);
    return true;
  }

  async function restoreLastKnownSession(): Promise<void> {
    const restoredWorkbench = await restorePersistedWorkbenchState();
    if (restoredWorkbench) return;

    const lastActiveId = globalThis.localStorage?.getItem("kairox.last-active-session-id");
    const targetId =
      lastActiveId && sessions.value.some((s) => s.id === lastActiveId)
        ? lastActiveId
        : sessions.value[0]?.id;
    if (targetId) {
      await switchSession(targetId);
    }
  }

  async function ensureSessionForSend(): Promise<void> {
    if (currentSessionId.value) return;
    const settings = capturePendingSessionSettings();
    const pending = pendingSessionDraft.value;
    if (pending?.kind !== "project") {
      await createSession(settings.profile);
      await applyPendingSessionSettings(settings);
      return;
    }

    const projectStore = useProjectStore();
    const selectedBranch = pending.branch?.trim() || null;
    let currentBranch: string | null = null;
    try {
      currentBranch = (await projectStore.getProjectGitStatus(pending.projectId)).branch;
    } catch {
      currentBranch = null;
    }

    const projectSession =
      selectedBranch && selectedBranch !== currentBranch
        ? await projectStore.createProjectWorktreeSession(pending.projectId, selectedBranch)
        : await projectStore.createProjectDraftSession(pending.projectId);
    await switchProjectSession(projectSession);
    await applyPendingSessionSettings(settings);
  }

  async function deleteSession(sessionId: string) {
    await deleteSessionImpl(sessionId, sessionActionDeps, switchSession);
  }

  async function renameSession(sessionId: string, title: string) {
    await renameSessionImpl(sessionId, title, sessionActionDeps);
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
      if (sessions.value.length > 0 || readPersistedWorkbenchState()) {
        try {
          await restoreLastKnownSession();
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

  async function loadProfileInfo(options: LoadProfileInfoOptions = {}): Promise<void> {
    if (loadingProfileInfo.value && !profileInfoLoad && !options.force && !options.refreshConfig) {
      return;
    }
    if (profileInfoLoad) {
      await profileInfoLoad;
      if (!options.force && !options.refreshConfig) return;
    }

    const nextLoad = (async () => {
      loadingProfileInfo.value = true;
      try {
        if (options.refreshConfig) {
          await invoke("refresh_config");
        }
        profileInfos.value = await invoke<ProfileInfo[]>("get_profile_info");
      } catch (error) {
        console.error("Failed to load profile info:", error);
      } finally {
        loadingProfileInfo.value = false;
      }
    })();

    profileInfoLoad = nextLoad;
    try {
      await nextLoad;
    } finally {
      if (profileInfoLoad === nextLoad) {
        profileInfoLoad = null;
      }
    }
  }

  async function refreshProfileInfoForCurrentContext(): Promise<void> {
    const sessionInfo = currentSessionInfo.value;
    if (sessionInfo?.project_id) {
      const projectStore = useProjectStore();
      const rootPath =
        sessionInfo.worktree_path ??
        projectStore.projects.find((entry) => entry.projectId === sessionInfo.project_id)?.rootPath;
      if (rootPath) {
        await projectStore.refreshProjectConfigRoot(rootPath);
        return;
      }
    }

    await loadProfileInfo({ refreshConfig: true });
  }

  async function setApprovalPolicy(approval: string): Promise<void> {
    if (!currentSessionId.value) {
      approvalPolicy.value = approval;
      return;
    }
    const ui = useUiStore();
    try {
      await persistApprovalPolicy(approval);
    } catch (e) {
      console.error("Failed to set approval policy:", e);
      ui.pushNotification("error", `Failed to set approval policy: ${e}`);
    }
  }

  async function setSandboxPolicy(sandboxJson: string): Promise<void> {
    if (!currentSessionId.value) {
      sandboxPolicy.value = sandboxJson;
      return;
    }
    const ui = useUiStore();
    try {
      await persistSandboxPolicy(sandboxJson);
    } catch (e) {
      console.error("Failed to set sandbox policy:", e);
      ui.pushNotification("error", `Failed to set sandbox policy: ${e}`);
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
      if (sessions.value.length > 0 || readPersistedWorkbenchState()) {
        await restoreLastKnownSession();
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
    currentReasoningEffort,
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
    approvalPolicy,
    sandboxPolicy,
    pendingSessionDraft,
    currentSessionInfo,
    composerDraftKey,
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
    startOrdinaryDraftSession,
    startProjectDraftSession,
    setPendingProjectBranch,
    ensureSessionForSend,
    deleteSession,
    renameSession,
    initializeWorkspace,
    loadProfileInfo,
    refreshProfileInfoForCurrentContext,
    recoverSessions,
    setConnected,
    setApprovalPolicy,
    setSandboxPolicy
  };
});
