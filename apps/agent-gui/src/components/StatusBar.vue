<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const session = useSessionStore();
const permissionMode = ref("interactive");

const streamingDotType = computed<"warning" | "default">(() =>
  session.isStreaming ? "warning" : "default"
);
const connectedDotType = computed<"success" | "error">(() =>
  session.connected ? "success" : "error"
);

onMounted(async () => {
  void session.loadProfileInfo();
  try {
    const mode: string = await invoke("get_permission_mode");
    permissionMode.value = mode.toLowerCase();
  } catch {
    permissionMode.value = "interactive";
  }
});
</script>

<template>
  <footer class="status-bar" data-test="status-bar">
    <div class="status-items">
      <!-- Sessions -->
      <div class="status-item">
        <span class="status-label">{{ t("statusBar.sessionsLabel") }}:</span>
        <span class="status-value">{{ session.sessions.length }}</span>
      </div>

      <!-- Streaming -->
      <div class="status-item">
        <span class="status-label">{{ t("statusBar.streamingLabel") }}:</span>
        <span class="status-dot" :class="`dot-${streamingDotType}`"></span>
        <span class="status-value">{{
          session.isStreaming ? t("common.yes") : t("common.no")
        }}</span>
      </div>

      <!-- Connected -->
      <div class="status-item">
        <span class="status-label">{{ t("statusBar.connectedLabel") }}:</span>
        <span class="status-dot" :class="`dot-${connectedDotType}`"></span>
        <span class="status-value">{{ session.connected ? t("common.yes") : t("common.no") }}</span>
      </div>

      <!-- Mode -->
      <div class="status-item">
        <span class="status-label">{{ t("statusBar.modeLabel") }}:</span>
        <span class="status-value">{{ permissionMode }}</span>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  padding: 6px 16px;
  background: var(--app-card-color);
  border-top: 1px solid var(--app-border-color);
  font-size: 12px;
  color: var(--app-text-color);
}
.status-items {
  display: flex;
  gap: 16px;
  align-items: center;
  flex-wrap: nowrap;
}
.status-item {
  display: flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}
.status-label {
  color: var(--app-text-color);
  opacity: 0.7;
  font-size: 11px;
}
.status-value {
  color: var(--app-text-color);
  font-weight: 500;
  font-size: 11px;
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}
.dot-success {
  background: var(--app-success-color, #52c41a);
}
.dot-error {
  background: var(--app-error-color, #ff4d4f);
}
.dot-warning {
  background: var(--app-warning-color, #faad14);
}
.dot-default {
  background: var(--app-text-color);
  opacity: 0.4;
}
</style>
