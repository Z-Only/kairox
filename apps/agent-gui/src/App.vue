<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvents } from "@/composables/useTauriEvents";
import { useUpdater } from "@/composables/useUpdater";
import { useSessionStore } from "@/stores/session";
import { useUiStore } from "@/stores/ui";
import AppLayout from "@/layouts/AppLayout.vue";

const session = useSessionStore();
const ui = useUiStore();

useTauriEvents();
useUpdater();

onMounted(async () => {
  const recovered = await session.recoverSessions();
  if (recovered) return;

  try {
    const workspaceInfo: { workspace_id: string; path: string } =
      await invoke("initialize_workspace");
    session.workspaceId = workspaceInfo.workspace_id;
    session.initialized = true;
    session.sessions = await invoke("list_sessions");
    if (session.sessions.length > 0) {
      await session.switchSession(session.sessions[0].id);
    }
  } catch (e) {
    console.error("Failed to initialize workspace:", e);
    ui.pushNotification("error", `Failed to initialize workspace: ${e}`);
  }
});
</script>

<template>
  <AppLayout />
</template>
