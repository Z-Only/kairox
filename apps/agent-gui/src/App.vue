<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvents } from "./composables/useTauriEvents";
import { sessionState } from "./stores/session";
import { traceState } from "./composables/useTraceStore";
import ChatPanel from "./components/ChatPanel.vue";
import SessionsSidebar from "./components/SessionsSidebar.vue";
import StatusBar from "./components/StatusBar.vue";
import TraceTimeline from "./components/TraceTimeline.vue";
import PermissionCenter from "./components/PermissionCenter.vue";

useTauriEvents();

onMounted(async () => {
  try {
    await invoke("initialize_workspace");
    sessionState.initialized = true;
    sessionState.sessions = await invoke("list_sessions");
    if (sessionState.sessions.length > 0) {
      const firstSession = sessionState.sessions[0];
      sessionState.currentSessionId = firstSession.id;
      sessionState.currentProfile = firstSession.profile;
      // Add a trace entry for the initial session since the event
      // was broadcast before the event forwarder was listening
      traceState.entries.push({
        id: `init-${firstSession.id}`,
        kind: "tool",
        status: "completed",
        toolId: "task",
        title: firstSession.title,
        startedAt: Date.now(),
        expanded: false
      });
    }
  } catch (e) {
    console.error("Failed to initialize workspace:", e);
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
