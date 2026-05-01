<script setup lang="ts">
import { sessionState } from "../stores/session";
</script>

<template>
  <footer class="status-bar">
    <span class="status-item">profile: {{ sessionState.currentProfile }}</span>
    <span class="status-divider">│</span>
    <span class="status-item"
      >sessions: {{ sessionState.sessions.length }}</span
    >
    <span class="status-divider">│</span>
    <span class="status-item">{{
      sessionState.isStreaming ? "streaming..." : "idle"
    }}</span>
    <span class="status-divider">│</span>
    <span class="status-item">{{
      sessionState.connected ? "connected" : "disconnected"
    }}</span>
    <span class="status-divider">│</span>
    <span class="status-item">mode: interactive</span>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 16px;
  background: #f5f5f5;
  border-top: 1px solid #d7d7d7;
  font-size: 11px;
  color: #555;
}
.status-divider {
  color: #ccc;
}
</style>
