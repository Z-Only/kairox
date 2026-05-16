<script setup lang="ts">
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const session = useSessionStore();

const streamingDotType = computed<"warning" | "default">(() =>
  session.isStreaming ? "warning" : "default"
);
const connectedDotType = computed<"success" | "error">(() =>
  session.connected ? "success" : "error"
);

onMounted(() => {
  void session.loadProfileInfo();
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
        <span class="status-value">{{ session.permissionMode }}</span>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  padding: 6px 16px;
  background: var(--app-card-color);
  border-top: 1px solid var(--app-border-color);
  font-size: var(--app-text-xs);
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
  opacity: 0.8;
  font-size: 11px;
}
.status-value {
  color: var(--app-text-color);
  font-weight: 500;
  font-size: var(--app-text-xs);
}
.status-dot {
  width: 7px;
  height: 7px;
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
