<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTauriEvents } from "./composables/useTauriEvents";
import { useUpdater } from "./composables/useUpdater";
import { addNotification } from "./composables/useNotifications";
import { sessionState, recoverSessions, setProjection } from "./stores/session";
import type { SessionProjection } from "./types";
import ChatPanel from "./components/ChatPanel.vue";
import SessionsSidebar from "./components/SessionsSidebar.vue";
import StatusBar from "./components/StatusBar.vue";
import TraceTimeline from "./components/TraceTimeline.vue";
import PermissionCenter from "./components/PermissionCenter.vue";
import NotificationToast from "./components/NotificationToast.vue";
import Marketplace from "./views/Marketplace.vue";

type View = "workbench" | "marketplace";
const view = ref<View>("workbench");

useTauriEvents();
useUpdater();

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
  <nav class="app-nav">
    <button
      :class="{ active: view === 'workbench' }"
      data-test="nav-workbench"
      @click="view = 'workbench'"
    >
      Workbench
    </button>
    <button
      :class="{ active: view === 'marketplace' }"
      data-test="nav-marketplace"
      @click="view = 'marketplace'"
    >
      Marketplace
    </button>
  </nav>
  <main v-if="view === 'workbench'" class="workbench">
    <SessionsSidebar />
    <ChatPanel />
    <aside class="right-sidebar">
      <TraceTimeline />
      <PermissionCenter />
    </aside>
  </main>
  <Marketplace v-else />
  <StatusBar />
  <NotificationToast />
</template>

<style scoped>
.app-nav {
  display: flex;
  gap: 8px;
  padding: 6px 12px;
  border-bottom: 1px solid #d7d7d7;
  background: var(--surface-alt, #f7f7f7);
}
.app-nav button {
  padding: 4px 10px;
  border: 1px solid var(--border, #ccc);
  background: transparent;
  cursor: pointer;
  font-size: 12px;
}
.app-nav button.active {
  background: var(--accent, #345);
  color: #fff;
}
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
