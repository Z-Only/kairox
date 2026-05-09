<script setup lang="ts">
import { useSessionStore } from "@/stores/session";
import { useUiStore } from "@/stores/ui";
import SessionsSidebar from "@/components/SessionsSidebar.vue";
import ChatPanel from "@/components/ChatPanel.vue";
import TraceTimeline from "@/components/TraceTimeline.vue";
import PermissionCenter from "@/components/PermissionCenter.vue";
import StatusBar from "@/components/StatusBar.vue";

const route = useRoute();
const router = useRouter();
const session = useSessionStore();
const ui = useUiStore();
const { t } = useI18n();
const { currentSessionId } = storeToRefs(session);

const routeSessionId = computed(() => {
  const v = route.params.sessionId;
  return Array.isArray(v) ? v[0] : v;
});

// Guard against the URL ↔ store ping-pong: when `syncRouteToSession` triggers
// a `router.replace({ name: "workbench" })` for a not-found id, the reverse
// watcher below would otherwise observe the still-stale `currentSessionId`
// and immediately rewrite the URL back to the bad id, undoing the redirect.
const syncing = ref(false);

async function syncRouteToSession(id: string | undefined) {
  if (!id) return;
  if (id === currentSessionId.value) return;
  syncing.value = true;
  try {
    await session.switchSession(id);
  } catch (err) {
    console.error("[WorkbenchView] switchSession failed:", err);
    ui.pushNotification("error", t("workbench.sessionNotFound", { id }));
    await router.replace({ name: "workbench" });
  } finally {
    syncing.value = false;
  }
}

onMounted(() => {
  void syncRouteToSession(routeSessionId.value);
});

watch(routeSessionId, (next) => {
  void syncRouteToSession(next);
});

// Reflect store changes back into URL.
watch(currentSessionId, (next) => {
  if (syncing.value) return;
  if (next && next !== routeSessionId.value) {
    void router.replace({ name: "workbench", params: { sessionId: next } });
  }
});
</script>

<template>
  <main class="workbench" data-test="view-workbench">
    <h1 class="workbench-heading" data-test="workbench-heading">
      {{ t("nav.workbench") }}
    </h1>
    <SessionsSidebar />
    <ChatPanel />
    <aside class="right-sidebar">
      <TraceTimeline />
      <PermissionCenter />
    </aside>
    <StatusBar />
  </main>
</template>

<style scoped>
.workbench {
  display: grid;
  grid-template-columns: 220px 1fr 280px;
  grid-template-rows: 1fr auto;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.workbench-heading {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}
.right-sidebar {
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--app-border-color);
  overflow: hidden;
}
:deep(.status-bar) {
  grid-column: 1 / -1;
}
</style>
