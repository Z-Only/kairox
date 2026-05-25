import type { Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SessionProjection, SessionInfoResponse, DomainEvent } from "@/types";
import { uniqueSessionTitle, filterOrdinarySessions } from "@/stores/session";
import { clearTrace, applyTraceEvent } from "@/composables/useTraceStore";
import type { useUiStore } from "@/stores/ui";
import type { useTaskGraphStore } from "@/stores/taskGraph";

async function listOrdinarySessions(): Promise<SessionInfoResponse[]> {
  const sessionList = await invoke<SessionInfoResponse[]>("list_sessions");
  return filterOrdinarySessions(sessionList);
}

export interface SessionActionDeps {
  sessions: Ref<SessionInfoResponse[]>;
  currentSessionId: Ref<string | null>;
  currentProfile: Ref<string>;
  currentReasoningEffort: Ref<string | null>;
  permissionMode: Ref<string>;
  profileInfos: Ref<{ alias: string }[]>;

  resetProjection(): void;
  setProjection(next: SessionProjection): void;
  getTaskGraphStore(): ReturnType<typeof useTaskGraphStore>;
  getUiStore(): ReturnType<typeof useUiStore>;
}

export async function switchToKnownSession(
  sessionId: string,
  target: SessionInfoResponse,
  deps: SessionActionDeps
): Promise<void> {
  if (sessionId === deps.currentSessionId.value) return;
  deps.resetProjection();
  clearTrace();
  deps.getTaskGraphStore().clearTaskGraph();
  const next = await invoke<SessionProjection>("switch_session", { sessionId });
  deps.currentSessionId.value = sessionId;
  globalThis.localStorage?.setItem("kairox.last-active-session-id", sessionId);
  deps.currentProfile.value = target.profile;
  deps.currentReasoningEffort.value = null;
  if (target.permission_mode) {
    deps.permissionMode.value = target.permission_mode;
  }
  if (deps.currentProfile.value === "default" && deps.profileInfos.value.length > 0) {
    deps.currentProfile.value = deps.profileInfos.value[0].alias;
  }
  deps.setProjection(next);
  const traceStrings = await invoke<string[]>("get_trace", { sessionId });
  for (const jsonStr of traceStrings) {
    try {
      const event = JSON.parse(jsonStr) as DomainEvent;
      if (event.payload.type === "ModelProfileSwitched") {
        deps.currentReasoningEffort.value = event.payload.reasoning_effort ?? null;
      }
      applyTraceEvent(event);
    } catch {
      // Skip malformed trace entries
    }
  }
}

export async function createSession(
  profileArg: string | undefined,
  permissionModeArg: string | undefined,
  deps: SessionActionDeps
): Promise<{ id: string; title: string; profile: string }> {
  const result = await invoke<{ id: string; title: string; profile: string }>("start_session", {
    profile: profileArg ?? deps.currentProfile.value,
    permissionMode: permissionModeArg ?? deps.permissionMode.value
  });

  deps.sessions.value = await listOrdinarySessions();
  const existingTitles = deps.sessions.value.filter((s) => s.id !== result.id).map((s) => s.title);
  const title = uniqueSessionTitle("New Session", existingTitles);

  if (title !== result.title) {
    try {
      await invoke("rename_session", { sessionId: result.id, title });
    } catch (e) {
      console.error("Failed to set deduped session title:", e);
    }
  }

  deps.currentProfile.value = result.profile;
  deps.currentReasoningEffort.value = null;
  deps.currentSessionId.value = result.id;
  globalThis.localStorage?.setItem("kairox.last-active-session-id", result.id);
  deps.resetProjection();
  clearTrace();
  deps.getTaskGraphStore().clearTaskGraph();
  return { id: result.id, title, profile: result.profile };
}

export async function deleteSession(
  sessionId: string,
  deps: SessionActionDeps,
  switchSessionFn: (sessionId: string) => Promise<void>
): Promise<void> {
  const ui = deps.getUiStore();
  try {
    await invoke("delete_session", { sessionId });
    deps.sessions.value = deps.sessions.value.filter((s) => s.id !== sessionId);
    if (deps.currentSessionId.value === sessionId) {
      if (deps.sessions.value.length > 0) {
        await switchSessionFn(deps.sessions.value[0].id);
      } else {
        deps.currentSessionId.value = null;
        deps.resetProjection();
        clearTrace();
        deps.getTaskGraphStore().clearTaskGraph();
      }
    }
  } catch (e) {
    console.error("Failed to delete session:", e);
    ui.pushNotification("error", `Failed to delete session: ${e}`);
  }
}

export async function renameSession(
  sessionId: string,
  title: string,
  deps: SessionActionDeps
): Promise<void> {
  const ui = deps.getUiStore();
  try {
    await invoke("rename_session", { sessionId, title });
    const session = deps.sessions.value.find((s) => s.id === sessionId);
    if (session) {
      session.title = title;
    }
  } catch (e) {
    console.error("Failed to rename session:", e);
    ui.pushNotification("error", `Failed to rename session: ${e}`);
  }
}
