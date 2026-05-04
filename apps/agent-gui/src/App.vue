<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTauriEvents } from "./composables/useTauriEvents";
import { addNotification } from "./composables/useNotifications";
import { sessionState, recoverSessions, setProjection } from "./stores/session";
import type { SessionProjection } from "./types";
import ChatPanel from "./components/ChatPanel.vue";
import SessionsSidebar from "./components/SessionsSidebar.vue";
import StatusBar from "./components/StatusBar.vue";
import TraceTimeline from "./components/TraceTimeline.vue";
import PermissionCenter from "./components/PermissionCenter.vue";
import NotificationToast from "./components/NotificationToast.vue";

useTauriEvents();

onMounted(async () => {
  // Listen for backend error events
  await listen<{ type: string; error: string; session_id: string }>(
    "session-error",
    (event) => {
      addNotification("error", event.payload.error);
    }
  );

  // Try to recover existing workspace and sessions from metadata store
  const recovered = await recoverSessions();

  if (!recovered) {
    // First-run: initialize a new workspace
    try {
      const workspaceInfo: { workspace_id: string; path: string } =
        await invoke("initialize_workspace");
      sessionState.initialized = true;
      sessionState.workspaceId = workspaceInfo.workspace_id;
      sessionState.sessions = await invoke("list_sessions");
      if (sessionState.sessions.length > 0) {
        const firstSession = sessionState.sessions[0];
        sessionState.currentSessionId = firstSession.id;
        sessionState.currentProfile = firstSession.profile;
        // Load projection (including task graph) for the initial session
        try {
          const projection = await invoke("switch_session", {
            sessionId: firstSession.id
          });
          setProjection(projection as SessionProjection);
        } catch {
          // Non-critical: initial session may have minimal data
        }
      }
    } catch (e) {
      console.error("Failed to initialize workspace:", e);
      addNotification("error", `Failed to initialize workspace: ${e}`);
    }
  }
});
</script>

<template>
  <main class="workbench">
    <SessionsSidebar />
    <ChatPanel />
    <aside class="right-sidebar">
      <TraceTimeline />
      <PermissionCenter />
    </aside>
  </main>
  <StatusBar />
  <NotificationToast />
</template>

<style scoped>
.workbench {
  display: grid;
  grid-template-columns: 220px 1fr 280px;
  flex: 1;
  overflow: hidden;
}
.right-sidebar {
  display: flex;
  flex-direction: column;
  border-left: 1px solid #d7d7d7;
  overflow: hidden;
}
</style>
