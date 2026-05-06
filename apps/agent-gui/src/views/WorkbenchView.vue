<script setup lang="ts">
import { onMounted, ref, watch, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "@/stores/session";
import { useUiStore } from "@/stores/ui";
import SessionsSidebar from "@/components/SessionsSidebar.vue";
import ChatPanel from "@/components/ChatPanel.vue";
import TraceTimeline from "@/components/TraceTimeline.vue";
import PermissionCenter from "@/components/PermissionCenter.vue";

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
    <SessionsSidebar />
    <ChatPanel />
    <aside class="right-sidebar">
      <TraceTimeline />
      <PermissionCenter />
    </aside>
  </main>
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
