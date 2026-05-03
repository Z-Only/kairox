<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvents } from "./composables/useTauriEvents";
import { sessionState, recoverSessions } from "./stores/session";
import ChatPanel from "./components/ChatPanel.vue";
import SessionsSidebar from "./components/SessionsSidebar.vue";
import StatusBar from "./components/StatusBar.vue";
import TraceTimeline from "./components/TraceTimeline.vue";
import PermissionCenter from "./components/PermissionCenter.vue";

useTauriEvents();

onMounted(async () => {
  // Try to recover existing workspace and sessions from metadata store
  const recovered = await recoverSessions();

  if (!recovered) {
    // First-run: initialize a new workspace
    try {
      await invoke("initialize_workspace");
      sessionState.initialized = true;
      sessionState.sessions = await invoke("list_sessions");
      if (sessionState.sessions.length > 0) {
        const firstSession = sessionState.sessions[0];
        sessionState.currentSessionId = firstSession.id;
        sessionState.currentProfile = firstSession.profile;
      }
    } catch (e) {
      console.error("Failed to initialize workspace:", e);
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
