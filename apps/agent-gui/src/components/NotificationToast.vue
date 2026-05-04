<script setup lang="ts">
import {
  notifications,
  dismissNotification
} from "../composables/useNotifications";
</script>

<template>
  <div v-if="notifications.length > 0" class="notification-container">
    <div
      v-for="notif in notifications.slice(-3)"
      :key="notif.id"
      :class="['notification', `notification--${notif.type}`]"
    >
      <span class="notification-icon">
        {{
          notif.type === "error" ? "✕" : notif.type === "warning" ? "⚠" : "ℹ"
        }}
      </span>
      <span class="notification-message">{{ notif.message }}</span>
      <button
        class="notification-dismiss"
        @click="dismissNotification(notif.id)"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<style scoped>
.notification-container {
  position: fixed;
  bottom: 32px;
  right: 16px;
  z-index: 300;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: 380px;
}
.notification {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 6px;
  font-size: 13px;
  line-height: 1.4;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  animation: slide-in 0.2s ease-out;
}
@keyframes slide-in {
  from {
    opacity: 0;
    transform: translateX(20px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
.notification--error {
  background: #fef2f2;
  border: 1px solid #fca5a5;
  color: #991b1b;
}
.notification--warning {
  background: #fffbeb;
  border: 1px solid #fcd34d;
  color: #92400e;
}
.notification--info {
  background: #eff6ff;
  border: 1px solid #93c5fd;
  color: #1e40af;
}
.notification-icon {
  flex-shrink: 0;
  font-weight: 600;
}
.notification-message {
  flex: 1;
  overflow-wrap: anywhere;
}
.notification-dismiss {
  flex-shrink: 0;
  background: none;
  border: none;
  cursor: pointer;
  font-size: 14px;
  padding: 0;
  color: inherit;
  opacity: 0.6;
}
.notification-dismiss:hover {
  opacity: 1;
}
</style>
