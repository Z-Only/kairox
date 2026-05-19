<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    title: string;
    closeLabel?: string;
    width?: string;
    panelDataTest?: string;
    bodyDataTest?: string;
  }>(),
  {
    closeLabel: "Close",
    width: "480px",
    panelDataTest: undefined,
    bodyDataTest: undefined
  }
);

const emit = defineEmits<{
  close: [];
}>();

const panelStyle = computed(() => ({
  "--kx-drawer-width": props.width
}));

function onOverlayClick(event: MouseEvent): void {
  if (event.target === event.currentTarget) {
    emit("close");
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="kx-drawer__overlay" @click="onOverlayClick">
      <aside class="kx-drawer" :style="panelStyle" :data-test="panelDataTest">
        <header class="kx-drawer__header">
          <span class="kx-drawer__title">{{ title }}</span>
          <button
            class="btn kx-drawer__close drawer-close-btn"
            type="button"
            :aria-label="closeLabel"
            @click="emit('close')"
          >
            x
          </button>
        </header>

        <div class="kx-drawer__body" :data-test="bodyDataTest">
          <slot />
        </div>

        <footer v-if="$slots.footer" class="kx-drawer__footer">
          <slot name="footer" />
        </footer>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.kx-drawer__overlay {
  position: fixed;
  inset: 0;
  z-index: var(--app-z-modal);
  background: var(--app-backdrop-color);
}

.kx-drawer {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  display: flex;
  width: var(--kx-drawer-width);
  max-width: 90vw;
  flex-direction: column;
  background: var(--app-body-color);
  box-shadow: var(--app-shadow-2);
}

.kx-drawer__header,
.kx-drawer__footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-color: var(--app-border-color);
}

.kx-drawer__header {
  justify-content: space-between;
  border-bottom: 1px solid var(--app-border-color);
}

.kx-drawer__footer {
  border-top: 1px solid var(--app-border-color);
}

.kx-drawer__title {
  min-width: 0;
  overflow: hidden;
  color: var(--app-text-color);
  font-size: 16px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kx-drawer__close {
  padding: 2px 8px;
  font-size: 16px;
  line-height: 1.2;
}

.kx-drawer__body {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}
</style>
