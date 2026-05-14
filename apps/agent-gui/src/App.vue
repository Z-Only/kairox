<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvents } from "@/composables/useTauriEvents";
import { useUpdater } from "@/composables/useUpdater";
import { useSessionStore } from "@/stores/session";
import { useUiStore } from "@/stores/ui";
import AppLayout from "@/layouts/AppLayout.vue";

const session = useSessionStore();
const ui = useUiStore();

// Sync the resolved dark-mode flag to `<html class="dark">` so that
// `theme.css`'s `html.dark { ... }` selector activates the dark palette.
watchEffect(() => {
  document.documentElement.classList.toggle("dark", ui.isDark);
});

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
      const lastActiveId = localStorage.getItem("kairox.last-active-session-id");
      const targetId =
        lastActiveId && session.sessions.some((s: { id: string }) => s.id === lastActiveId)
          ? lastActiveId
          : session.sessions[0].id;
      await session.switchSession(targetId);
    }
  } catch (e) {
    console.error("Failed to initialize workspace:", e);
    ui.pushNotification("error", `Failed to initialize workspace: ${e}`);
  }
});
</script>

<template>
  <AppLayout v-if="session.initialized" />
  <div v-else class="app-loading" data-test="app-loading">
    <span class="loading-spinner" />
  </div>
</template>

<style>
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  line-height: 1.5;
}

code,
pre,
.markdown-body code,
.markdown-body pre {
  font-family: "SF Mono", "Fira Code", "Cascadia Code", "Consolas", monospace;
}

h1,
h2,
h3,
h4,
h5,
h6 {
  line-height: 1.3;
}

.app-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
}

.loading-spinner {
  width: 24px;
  height: 24px;
  border: 3px solid var(--app-border-color);
  border-top-color: var(--app-primary-color);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
