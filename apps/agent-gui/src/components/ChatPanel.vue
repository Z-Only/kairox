<script setup lang="ts">
import type { ComponentPublicInstance } from "vue";
import { useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import { useChatStream } from "@/composables/useChatStream";
import type { ChatPermissionStreamItem } from "@/types/chatStream";
import ChatComposer from "@/components/ChatComposer.vue";
import ChatMessageItem from "@/components/chat/ChatMessageItem.vue";
import ChatToolCallItem from "@/components/chat/ChatToolCallItem.vue";
import ChatPermissionItem from "@/components/chat/ChatPermissionItem.vue";
import ChatCompactionItem from "@/components/chat/ChatCompactionItem.vue";
import ChatMonitorItem from "@/components/chat/ChatMonitorItem.vue";

const { t } = useI18n();
const session = useSessionStore();
const projectStore = useProjectStore();
const scrollbar = ref<HTMLElement | null>(null);

const chatStream = useChatStream();

// === Keyboard navigation across chat-stream items ========================
// j / ArrowDown — focus next item; k / ArrowUp — focus previous item.
// Enter / Space — activate the focused item's primary action (delegates to
// a native `.click()` on the item, which works for the embedded
// PermissionPrompt allow button or the ToolCall toggle row).
// gg — jump to the first item; G — jump to the last item.
// Modifier combos (Ctrl/Cmd/Alt) are reserved for host shortcuts and pass
// through untouched, as do keys originating from input-like targets so the
// composer textarea and form inputs keep their native typing behaviour.
const chatPanelRoot = ref<HTMLElement | null>(null);
const gPrefixTimer = ref<number | null>(null);

/** Detect targets where typing or text editing must be preserved. */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (target.isContentEditable) return true;
  return false;
}

function streamItemEls(): HTMLElement[] {
  const root = chatPanelRoot.value;
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>("[data-chat-stream-item]"));
}

function focusedStreamIndex(items: HTMLElement[]): number {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement)) return -1;
  for (let i = 0; i < items.length; i++) {
    if (items[i] === active) return i;
    if (items[i].contains(active)) return i;
  }
  return -1;
}

function clearGPrefix(): void {
  if (gPrefixTimer.value !== null) {
    clearTimeout(gPrefixTimer.value);
    gPrefixTimer.value = null;
  }
}

function armGPrefix(): void {
  clearGPrefix();
  // `setTimeout` returns a number in browsers (and the typed value the
  // jsdom shim returns) — Node returns a `Timeout` object. We only need a
  // handle good enough for `clearTimeout`; cast to `number` for storage.
  gPrefixTimer.value = setTimeout(() => {
    gPrefixTimer.value = null;
  }, 600) as unknown as number;
}

function handleChatKeydown(event: KeyboardEvent): void {
  // Reserve modifier combos for host shortcuts (Cmd+K palette, Ctrl+J
  // line-break, Alt+G workspace nav, etc.).
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  // Never hijack keystrokes while the user is typing into the composer
  // or any text input rendered inside the chat panel. We check both the
  // event target (real browser bubbling path) and `document.activeElement`
  // (covers programmatic dispatches in tests where the event is emitted
  // directly on the panel root rather than the focused input).
  if (isEditableTarget(event.target)) return;
  if (isEditableTarget(document.activeElement)) return;

  const key = event.key;
  const items = streamItemEls();
  if (items.length === 0) return;

  const currentIndex = focusedStreamIndex(items);

  switch (key) {
    case "j":
    case "ArrowDown": {
      const nextIndex = currentIndex < 0 ? 0 : Math.min(items.length - 1, currentIndex + 1);
      items[nextIndex]?.focus();
      clearGPrefix();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    case "k":
    case "ArrowUp": {
      const prevIndex = currentIndex < 0 ? 0 : Math.max(0, currentIndex - 1);
      items[prevIndex]?.focus();
      clearGPrefix();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    case "Enter":
    case " ": {
      if (currentIndex < 0) return;
      // Delegate to the focused item's primary click handler. For
      // Use the row's primary command when it has one: allow pending
      // permissions or toggle tool-call details. Plain messages fall back
      // to the wrapper click, which is currently inert.
      const focused = items[currentIndex];
      const primary = focused.querySelector<HTMLElement>(
        '[data-test="permission-allow"], [data-test="chat-tool-call-toggle"]'
      );
      if (primary) {
        primary.click();
      } else {
        focused.click();
      }
      clearGPrefix();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    case "g": {
      if (event.shiftKey) return; // `Shift+g` is the capital-G branch below.
      if (gPrefixTimer.value !== null) {
        // Second `g` within the window — jump to first item.
        items[0]?.focus();
        clearGPrefix();
      } else {
        // First `g` — arm the prefix and wait for a follow-up.
        armGPrefix();
      }
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    case "G": {
      items[items.length - 1]?.focus();
      clearGPrefix();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    default:
      // Any other key cancels a pending `g` prefix so an unrelated press
      // doesn't leave the navigator in a half-armed state.
      if (gPrefixTimer.value !== null) clearGPrefix();
  }
}

onBeforeUnmount(() => {
  clearGPrefix();
});

// === Jump-to-pending-permission CTA =====================================
// Slack/Discord-style floating pill that surfaces when an unresolved
// permission request is queued in the chat stream but has scrolled below
// (or above) the visible message-list viewport. Clicking it scrolls the
// first pending permission row back into view so the user doesn't miss
// the prompt.
//
// `useChatStream` already filters resolved permissions out of the chat
// feed, so every `kind === "permission"` item is by construction still
// pending. We attach an IntersectionObserver to each rendered
// `ChatPermissionItem` and treat any permission whose entry has not been
// observed as `isIntersecting=true` as "below the fold".
const pendingPermissionItems = computed<ChatPermissionStreamItem[]>(() =>
  chatStream.value.filter((item): item is ChatPermissionStreamItem => item.kind === "permission")
);
const firstPendingPermission = computed<ChatPermissionStreamItem | null>(
  () => pendingPermissionItems.value[0] ?? null
);

const permissionElementById = new Map<string, HTMLElement>();
const visiblePermissionIds = ref<Set<string>>(new Set());
let permissionIntersectionObserver: IntersectionObserver | null = null;

const showJumpPendingPermissionCta = computed(() => {
  const first = firstPendingPermission.value;
  if (!first) return false;
  // If the first pending permission's DOM node has never reported as
  // visible to the IntersectionObserver, assume it is offscreen and
  // surface the CTA. This is a safe default — once the observer fires
  // with `isIntersecting=true` the set gains the id and the CTA hides.
  return !visiblePermissionIds.value.has(first.id);
});

function bindPermissionRef(id: string, el: Element | ComponentPublicInstance | null): void {
  let domEl: HTMLElement | null = null;
  if (el instanceof HTMLElement) {
    domEl = el;
  } else if (el && typeof el === "object" && "$el" in el) {
    const candidate = (el as ComponentPublicInstance).$el;
    if (candidate instanceof HTMLElement) domEl = candidate;
  }

  const previous = permissionElementById.get(id);
  if (domEl) {
    if (previous !== domEl) {
      if (previous) permissionIntersectionObserver?.unobserve(previous);
      permissionElementById.set(id, domEl);
      permissionIntersectionObserver?.observe(domEl);
    }
  } else if (previous) {
    permissionIntersectionObserver?.unobserve(previous);
    permissionElementById.delete(id);
    if (visiblePermissionIds.value.has(id)) {
      const next = new Set(visiblePermissionIds.value);
      next.delete(id);
      visiblePermissionIds.value = next;
    }
  }
}

function jumpToPendingPermission(): void {
  const first = firstPendingPermission.value;
  if (!first) return;
  const target = permissionElementById.get(first.id);
  if (target) {
    target.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}

onMounted(() => {
  if (typeof IntersectionObserver === "undefined") return;
  permissionIntersectionObserver = new IntersectionObserver(
    (entries) => {
      const next = new Set(visiblePermissionIds.value);
      for (const entry of entries) {
        const target = entry.target;
        if (!(target instanceof HTMLElement)) continue;
        // Match the observed element back to the permission id by
        // scanning the map. The map is bounded by the number of pending
        // permissions, typically zero or one.
        let matchedId: string | null = null;
        for (const [id, el] of permissionElementById) {
          if (el === target) {
            matchedId = id;
            break;
          }
        }
        if (!matchedId) continue;
        if (entry.isIntersecting) next.add(matchedId);
        else next.delete(matchedId);
      }
      visiblePermissionIds.value = next;
    },
    {
      root: scrollbar.value,
      // Treat "partially visible" as visible — once any sliver of the
      // permission card enters the scroll viewport the CTA disappears.
      threshold: 0.01
    }
  );
  for (const el of permissionElementById.values()) {
    permissionIntersectionObserver.observe(el);
  }
});

onBeforeUnmount(() => {
  permissionIntersectionObserver?.disconnect();
  permissionIntersectionObserver = null;
  permissionElementById.clear();
});

const currentSession = computed(() => session.currentSessionInfo);
const currentProjectId = computed(() => currentSession.value?.project_id ?? null);
const currentProject = computed(() => {
  const projectId = currentProjectId.value;
  if (!projectId) return null;
  return projectStore.projects.find((p) => p.projectId === projectId) ?? null;
});
const resolvedGitBranch = ref<string | null>(null);
const resolvedGitBranchKey = ref<string | null>(null);

function normalizePathForCompare(path: string): string {
  return path.trim().replace(/[\\/]+$/, "");
}

function isWorktreeSession(sessionInfo: typeof currentSession.value): boolean {
  if (!sessionInfo?.worktree_path) return false;
  const worktreePath = normalizePathForCompare(sessionInfo.worktree_path);
  const projectRoot = currentProject.value?.rootPath;
  if (projectRoot) return worktreePath !== normalizePathForCompare(projectRoot);
  return (
    sessionInfo.worktree_path.includes("/.worktrees/") ||
    sessionInfo.worktree_path.includes("/.kairox/worktrees/")
  );
}

function gitBranchLookupKey(sessionInfo: NonNullable<typeof currentSession.value>): string {
  return [sessionInfo.id, sessionInfo.project_id ?? "", sessionInfo.worktree_path ?? ""].join("::");
}

function resolvedBranchFor(sessionInfo: NonNullable<typeof currentSession.value>): string | null {
  return resolvedGitBranchKey.value === gitBranchLookupKey(sessionInfo)
    ? resolvedGitBranch.value
    : null;
}

watch(
  currentSession,
  async (sessionInfo) => {
    resolvedGitBranch.value = null;
    resolvedGitBranchKey.value = null;
    if (!sessionInfo?.project_id || sessionInfo.branch) return;

    const lookupKey = gitBranchLookupKey(sessionInfo);
    try {
      const status = session.currentSessionId
        ? await projectStore.getSessionGitStatus(sessionInfo.id)
        : await projectStore.getProjectGitStatus(sessionInfo.project_id);
      if (currentSession.value && gitBranchLookupKey(currentSession.value) === lookupKey) {
        resolvedGitBranch.value = status.branch;
        resolvedGitBranchKey.value = lookupKey;
      }
    } catch {
      // Branch metadata is display-only; keep the composer path-free if git status fails.
    }
  },
  { immediate: true }
);

const sessionGitMeta = computed(() => {
  const sessionInfo = currentSession.value;
  if (!sessionInfo?.project_id && !sessionInfo?.worktree_path) return [];

  const branch = sessionInfo.branch ?? resolvedBranchFor(sessionInfo);
  if (!branch) return [];

  const gitMetaParts: string[] = [];
  if (isWorktreeSession(sessionInfo)) gitMetaParts.push("worktree");
  gitMetaParts.push(branch);
  return gitMetaParts;
});

const workspacePath = computed(() => {
  const sessionInfo = currentSession.value;
  if (sessionInfo?.worktree_path) return sessionInfo.worktree_path;
  const projectId = currentProjectId.value;
  if (!projectId) return "";
  const project = projectStore.projects.find((p) => p.projectId === projectId);
  return project?.rootPath ?? "";
});
const isEmptyProjectChat = computed(
  () =>
    Boolean(currentProjectId.value) &&
    session.projection.messages.length === 0 &&
    !session.projection.token_stream
);
const projectInstructionSummaryText = computed(() => {
  const projectId = currentProjectId.value;
  if (!projectId || !isEmptyProjectChat.value) return null;

  const instructionSummary = projectStore.instructionSummariesByProject.get(projectId);
  const sourceFileNames =
    instructionSummary?.sourcePaths
      .map((sourcePath) => sourcePath.split(/[\\/]/).filter(Boolean).at(-1))
      .filter((fileName): fileName is string => Boolean(fileName)) ?? [];
  if (!sourceFileNames.length) return null;

  return `Loaded ${sourceFileNames.join(", ")}`;
});

watch(
  () => currentProjectId.value,
  async (projectId) => {
    if (!projectId || projectStore.instructionSummariesByProject.has(projectId)) return;
    await projectStore.getProjectInstructionSummary(projectId);
  },
  { immediate: true }
);

watch(
  () => [session.projection.messages.length, session.projection.token_stream],
  async () => {
    await nextTick();
    if (scrollbar.value) {
      scrollbar.value.scrollTo({
        top: scrollbar.value.scrollHeight,
        behavior: "smooth"
      });
    }
  }
);
</script>

<template>
  <section
    ref="chatPanelRoot"
    class="chat-panel"
    data-test="chat-panel"
    @keydown="handleChatKeydown"
  >
    <header class="chat-header">
      <h2>{{ t("chat.header") }}</h2>
    </header>

    <div ref="scrollbar" class="message-list" data-test="message-list">
      <div class="message-list-inner">
        <KxEmptyState
          v-if="session.projection.messages.length === 0 && !session.projection.token_stream"
          class="chat-empty-state"
          data-test="chat-empty-state"
        >
          <template #icon>
            <svg
              width="40"
              height="40"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              aria-hidden="true"
            >
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
          </template>
          {{ t("chat.emptyState") }}
        </KxEmptyState>
        <template v-for="item in chatStream" :key="item.id">
          <div
            class="chat-stream-item"
            data-chat-stream-item
            :data-chat-stream-item-kind="item.kind"
            tabindex="0"
          >
            <ChatMessageItem
              v-if="item.kind === 'message'"
              :role="item.role"
              :content="item.content"
            />
            <ChatToolCallItem
              v-else-if="item.kind === 'tool_call'"
              :tool-call-id="item.id"
              :tool-id="item.toolId"
              :title="item.title"
              :status="item.status"
              :duration-ms="item.durationMs"
              :started-at="item.startedAt"
              :input="item.input"
              :output-preview="item.outputPreview"
              :scope="item.scope"
            />
            <ChatPermissionItem
              v-else-if="item.kind === 'permission'"
              :id="item.id"
              :ref="(el) => bindPermissionRef(item.id, el)"
              :variant="item.variant"
              :tool-id="item.toolId"
              :title="item.title"
              :input="item.input"
              :reason="item.reason"
              :scope="item.scope"
              :content="item.content"
              :raw-event="item.rawEvent"
            />
            <ChatCompactionItem v-else-if="item.kind === 'compaction'" :status="item.status" />
            <ChatMonitorItem
              v-else-if="item.kind === 'monitor'"
              :monitor-id="item.id"
              :description="item.description"
              :status="item.status"
              :last-line="item.lastLine"
              :command="item.command"
              :stop-reason="item.stopReason"
            />
          </div>
        </template>
        <div
          v-if="projectInstructionSummaryText"
          class="project-instruction-summary"
          data-test="project-instruction-summary"
        >
          {{ projectInstructionSummaryText }}
        </div>
        <div
          v-if="session.projection.token_stream"
          class="message message-assistant streaming"
          data-test="stream-indicator"
        >
          <span class="message-content"
            >{{ session.projection.token_stream }}<span class="cursor">▌</span></span
          >
        </div>
        <KxBadge
          v-if="session.projection.cancelled"
          class="cancelled-marker"
          tone="warning"
          data-test="cancelled-marker"
        >
          {{ t("chat.cancelled") }}
        </KxBadge>
      </div>
      <button
        v-if="showJumpPendingPermissionCta"
        type="button"
        class="jump-pending-permission-cta"
        data-test="jump-pending-permission-cta"
        :aria-label="t('chatStream.permission.jumpCta')"
        :title="t('chatStream.permission.jumpCta')"
        @click="jumpToPendingPermission"
      >
        <span class="jump-pending-permission-cta-count">
          {{
            t("chatStream.permission.jumpCtaCount", {
              count: pendingPermissionItems.length
            })
          }}
        </span>
        <span aria-hidden="true" class="jump-pending-permission-cta-arrow">↓</span>
      </button>
    </div>

    <div
      v-if="session.lastSendError"
      class="send-error-banner"
      data-test="error-banner"
      role="alert"
    >
      {{ session.lastSendError }}
    </div>

    <ChatComposer :workspace-path="workspacePath" :session-git-meta="sessionGitMeta" />
  </section>
</template>

<style scoped>
.chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: var(--app-card-color);
}
.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  min-height: 36px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-card-color);
}
.chat-header h2 {
  margin: 0;
  font-size: var(--app-text-lg);
  font-weight: 720;
}
.message-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  position: relative;
  background:
    radial-gradient(
      circle at 50% 0,
      color-mix(in srgb, var(--app-primary-color) 5%, transparent),
      transparent 260px
    ),
    var(--app-card-color);
}
.jump-pending-permission-cta {
  /* Float just above the chat composer, anchored to the bottom of the
     scroll viewport. `position: sticky` keeps the pill pinned to the
     bottom edge of the scrollable region as the user scrolls. */
  position: sticky;
  bottom: 12px;
  left: 50%;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin: 0 auto;
  padding: 6px 14px;
  border-radius: 999px;
  border: 1px solid var(--app-border-color);
  background: color-mix(in srgb, var(--app-elevated-color) 88%, transparent);
  backdrop-filter: blur(6px);
  color: var(--app-text-color);
  font-size: 12px;
  line-height: 1.4;
  cursor: pointer;
  box-shadow: var(--app-shadow-md);
  transform: translateX(-50%);
  /* `position: sticky` keeps the pill in flow; translateX centres it
     against the message-list width. */
}
@media (prefers-reduced-motion: no-preference) {
  .jump-pending-permission-cta {
    transition:
      opacity 120ms ease,
      color 120ms ease,
      background 120ms ease,
      border-color 120ms ease;
  }
}
.jump-pending-permission-cta:hover,
.jump-pending-permission-cta:focus-visible {
  outline: none;
  border-color: var(--app-primary-color);
  color: var(--app-primary-color);
}
.jump-pending-permission-cta-count {
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.jump-pending-permission-cta-arrow {
  font-size: 11px;
  opacity: 0.7;
}
.message-list-inner {
  padding: 14px 16px;
}
.message {
  display: flex;
  margin-bottom: 12px;
  line-height: 1.5;
}
.project-instruction-summary {
  margin-bottom: 12px;
  color: var(--app-muted-text-color, var(--app-text-color));
  font-size: 12px;
  line-height: 1.5;
}
.chat-empty-state {
  margin: 40px auto 28px;
  max-width: min(920px, 100%);
}
.message :deep(.message-content) {
  max-width: min(760px, 82%);
  border-radius: var(--app-radius-xl);
  padding: 10px 12px;
  white-space: pre-wrap;
  overflow-wrap: break-word;
}
.message-user {
  justify-content: flex-end;
}
.message-user :deep(.message-content) {
  color: var(--app-primary-contrast-color, #ffffff);
  background: var(--app-primary-color, #0077cc);
}
.message-assistant,
.message-planner,
.message-worker,
.message-reviewer,
.message-system {
  justify-content: flex-start;
}
.message-assistant :deep(.message-content),
.message-planner :deep(.message-content),
.message-worker :deep(.message-content),
.message-reviewer :deep(.message-content),
.message-system :deep(.message-content) {
  color: var(--app-muted-text-color, var(--app-text-color));
  background: var(--app-muted-surface-color, var(--app-panel-color));
}
.message-system :deep(.message-content) {
  opacity: 0.72;
  font-style: italic;
}
.streaming .cursor {
  animation: blink 1s step-end infinite;
}
.cancelled-marker {
  margin-top: 4px;
}
@keyframes blink {
  50% {
    opacity: 0;
  }
}
.send-error-banner {
  margin: 8px 16px 0;
  padding: 8px 10px;
  border: 1px solid var(--app-error-color);
  border-radius: var(--app-radius-md);
  background: color-mix(in srgb, var(--app-error-color) 10%, transparent);
  color: var(--app-error-color);
  font-size: 13px;
}
</style>
