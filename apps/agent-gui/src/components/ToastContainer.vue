<script setup lang="ts">
import { useUiStore, type ToastItem } from "@/stores/ui";

const ui = useUiStore();
const { toasts } = storeToRefs(ui);

function dismiss(id: string) {
  ui.removeToast(id);
}

function onEntered(toast: ToastItem) {
  if (toast.duration > 0) {
    setTimeout(() => ui.removeToast(toast.id), toast.duration);
  }
}

const iconMap: Record<ToastItem["type"], string> = {
  success: "✓",
  error: "✕",
  warning: "⚠",
  info: "ℹ"
};
</script>

<template>
  <Teleport to="body">
    <div class="toast-container" aria-live="polite">
      <TransitionGroup name="toast">
        <div
          v-for="toast in toasts"
          :key="toast.id"
          :class="['toast', `toast--${toast.type}`]"
          role="alert"
          @vue:mounted="onEntered(toast)"
        >
          <span class="toast__icon">{{ iconMap[toast.type] }}</span>
          <span class="toast__message">{{ toast.message }}</span>
          <button class="toast__close" aria-label="Dismiss" @click="dismiss(toast.id)">×</button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-container {
  position: fixed;
  top: 12px;
  right: 12px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-radius: 8px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  border: 1px solid var(--app-border-color);
  box-shadow: 0 4px 12px rgb(0 0 0 / 12%);
  font-size: 13px;
  pointer-events: auto;
  max-width: 400px;
}

.toast--success {
  border-left: 3px solid var(--app-success-color);
}
.toast--error {
  border-left: 3px solid var(--app-error-color);
}
.toast--warning {
  border-left: 3px solid var(--app-warning-color);
}
.toast--info {
  border-left: 3px solid var(--app-info-color);
}

.toast__icon {
  font-size: 16px;
  flex-shrink: 0;
}

.toast--success .toast__icon {
  color: var(--app-success-color);
}
.toast--error .toast__icon {
  color: var(--app-error-color);
}
.toast--warning .toast__icon {
  color: var(--app-warning-color);
}
.toast--info .toast__icon {
  color: var(--app-info-color);
}

.toast__message {
  flex: 1;
}

.toast__close {
  background: none;
  border: none;
  color: var(--app-text-color-3);
  cursor: pointer;
  font-size: 18px;
  padding: 0 2px;
  line-height: 1;
}

.toast__close:hover {
  color: var(--app-text-color);
}

@media (prefers-reduced-motion: no-preference) {
  .toast-enter-active {
    animation: toast-in 0.25s ease-out;
  }
  .toast-leave-active {
    animation: toast-out 0.2s ease-in;
  }
}

@keyframes toast-in {
  from {
    opacity: 0;
    transform: translateX(16px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes toast-out {
  from {
    opacity: 1;
    transform: translateX(0);
  }
  to {
    opacity: 0;
    transform: translateX(16px);
  }
}
</style>
