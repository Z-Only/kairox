<script setup lang="ts">
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
const { leftSidebarCollapsed, rightSidebarCollapsed, leftSidebarWidth, rightSidebarWidth } =
  storeToRefs(ui);

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

const workbenchStyle = computed(() => {
  const leftWidth = leftSidebarCollapsed.value ? 0 : leftSidebarWidth.value;
  const rightWidth = rightSidebarCollapsed.value ? 0 : rightSidebarWidth.value;

  return {
    gridTemplateColumns: `${leftWidth}px 8px minmax(0, 1fr) 8px ${rightWidth}px`
  };
});

const leftToggleLabel = computed(() =>
  leftSidebarCollapsed.value ? "Expand left sidebar" : "Collapse left sidebar"
);
const rightToggleLabel = computed(() =>
  rightSidebarCollapsed.value ? "Expand right sidebar" : "Collapse right sidebar"
);

function toggleSidebar(side: "left" | "right") {
  ui.toggleSidebar(side);
}

function startResize(side: "left" | "right", event: PointerEvent) {
  if (
    (side === "left" && leftSidebarCollapsed.value) ||
    (side === "right" && rightSidebarCollapsed.value)
  ) {
    return;
  }

  event.preventDefault();
  const startX = event.clientX;
  const startWidth = side === "left" ? leftSidebarWidth.value : rightSidebarWidth.value;

  const onPointerMove = (moveEvent: PointerEvent) => {
    const delta = moveEvent.clientX - startX;
    ui.setSidebarWidth(side, side === "left" ? startWidth + delta : startWidth - delta);
  };
  const onPointerUp = () => {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    document.documentElement.classList.remove("workbench-resizing");
  };

  document.documentElement.classList.add("workbench-resizing");
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp, { once: true });
}
</script>

<template>
  <main
    class="workbench"
    :class="{
      'workbench--left-collapsed': leftSidebarCollapsed,
      'workbench--right-collapsed': rightSidebarCollapsed
    }"
    :style="workbenchStyle"
    data-test="view-workbench"
  >
    <h1 class="workbench-heading" data-test="workbench-heading">
      {{ t("nav.workbench") }}
    </h1>
    <SessionsSidebar
      class="left-sidebar"
      :aria-hidden="leftSidebarCollapsed ? 'true' : undefined"
    />
    <div class="sidebar-divider sidebar-divider--left">
      <button
        type="button"
        class="sidebar-resizer"
        data-test="left-sidebar-resizer"
        aria-label="Resize left sidebar"
        title="Resize left sidebar"
        :disabled="leftSidebarCollapsed"
        @pointerdown="startResize('left', $event)"
      >
        <span class="resize-grip" aria-hidden="true"></span>
      </button>
      <button
        type="button"
        class="sidebar-toggle sidebar-toggle--left"
        data-test="left-sidebar-toggle"
        :aria-label="leftToggleLabel"
        :title="leftToggleLabel"
        @click="toggleSidebar('left')"
      >
        <span
          :class="[
            'collapse-glyph',
            leftSidebarCollapsed ? 'collapse-glyph--expand-left' : 'collapse-glyph--collapse-left'
          ]"
          aria-hidden="true"
        ></span>
      </button>
    </div>
    <ChatPanel />
    <div class="sidebar-divider sidebar-divider--right">
      <button
        type="button"
        class="sidebar-resizer"
        data-test="right-sidebar-resizer"
        aria-label="Resize right sidebar"
        title="Resize right sidebar"
        :disabled="rightSidebarCollapsed"
        @pointerdown="startResize('right', $event)"
      >
        <span class="resize-grip" aria-hidden="true"></span>
      </button>
      <button
        type="button"
        class="sidebar-toggle sidebar-toggle--right"
        data-test="right-sidebar-toggle"
        :aria-label="rightToggleLabel"
        :title="rightToggleLabel"
        @click="toggleSidebar('right')"
      >
        <span
          :class="[
            'collapse-glyph',
            rightSidebarCollapsed
              ? 'collapse-glyph--expand-right'
              : 'collapse-glyph--collapse-right'
          ]"
          aria-hidden="true"
        ></span>
      </button>
    </div>
    <aside class="right-sidebar" :aria-hidden="rightSidebarCollapsed ? 'true' : undefined">
      <TraceTimeline />
      <PermissionCenter />
    </aside>
  </main>
</template>

<style scoped>
.workbench {
  display: grid;
  grid-template-rows: 1fr;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  position: relative;
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
/* 统一左侧面板与中间面板的分隔线 */
.left-sidebar {
  overflow: hidden;
}
.workbench--left-collapsed .left-sidebar,
.workbench--right-collapsed .right-sidebar {
  visibility: hidden;
  pointer-events: none;
}
.right-sidebar {
  display: flex;
  flex-direction: column;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
}
.sidebar-divider {
  position: relative;
  display: flex;
  align-items: stretch;
  justify-content: center;
  background: var(--app-body-color);
}
.sidebar-divider--left {
  border-left: 1px solid var(--app-border-color);
}
.sidebar-divider--right {
  border-right: 1px solid var(--app-border-color);
}
.sidebar-resizer,
.sidebar-toggle {
  appearance: none;
  border: 0;
  color: var(--app-text-color-3);
  background: transparent;
  padding: 0;
}
.sidebar-resizer {
  width: 8px;
  cursor: col-resize;
}
.sidebar-resizer:disabled {
  cursor: default;
}
.resize-grip {
  display: block;
  width: 4px;
  height: 18px;
  margin: 0 auto;
  border-left: 1px solid currentColor;
  border-right: 1px solid currentColor;
  opacity: 0;
  transition:
    opacity 120ms ease,
    color 120ms ease;
}
.sidebar-divider:hover .resize-grip,
.sidebar-resizer:focus-visible .resize-grip,
:global(.workbench-resizing) .resize-grip {
  opacity: 0.8;
}
.sidebar-resizer:disabled .resize-grip {
  opacity: 0;
}
.sidebar-toggle {
  position: absolute;
  top: 10px;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-body-color);
  box-shadow: 0 1px 2px rgb(15 23 42 / 12%);
  cursor: pointer;
}
.sidebar-toggle--left {
  left: -11px;
}
.sidebar-toggle--right {
  right: -11px;
}
.sidebar-toggle:hover,
.sidebar-toggle:focus-visible {
  color: var(--app-primary-color);
  border-color: var(--app-primary-color);
}
.collapse-glyph {
  width: 7px;
  height: 7px;
  border-top: 1.5px solid currentColor;
  border-right: 1.5px solid currentColor;
}
.collapse-glyph--collapse-left,
.collapse-glyph--expand-right {
  transform: rotate(-135deg);
}
.collapse-glyph--expand-left,
.collapse-glyph--collapse-right {
  transform: rotate(45deg);
}
</style>
