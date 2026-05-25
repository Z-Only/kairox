<script setup lang="ts">
// Standalone inline chat-stream row for a single tool/command execution.
// Pattern mirrors TraceEntry.vue, but this component owns its expand/collapse
// state locally (via prop + emit) and does NOT mutate any Pinia store. A
// later PR will mount it from the chat-stream dispatcher.
//
// NOTE: We intentionally define the props interface inline rather than
// importing from `@/types/chatStream` — that file is owned by a sibling
// lane and does not yet exist on this branch.
import { useToolIcon } from "@/composables/useToolIcon";
import { isDiffShaped } from "@/composables/useDiffDetect";
import { useChatToolExpand } from "@/composables/useChatToolExpand";
import { useSessionStore } from "@/stores/session";
import { storeToRefs } from "pinia";
import DiffPreview from "@/components/chat/DiffPreview.vue";

interface ChatToolCallItemProps {
  toolId: string;
  /**
   * Unique-per-invocation identifier used to scope the persisted
   * expand/collapse state in localStorage. Falls back to `toolId` when
   * not provided; callers rendering multiple invocations of the same
   * tool should pass a stable per-invocation id (e.g., the trace entry
   * id) to avoid sharing state across rows.
   */
  toolCallId?: string;
  title?: string;
  status: "running" | "completed" | "failed" | "pending";
  durationMs?: number;
  /**
   * Epoch milliseconds when the tool call started. When provided and the
   * row is expanded, we render a "started X ago" line below the duration
   * so users can see how long ago the call kicked off without hovering.
   */
  startedAt?: number;
  input?: string;
  outputPreview?: string;
  scope?: string;
  /** Controlled mode: when provided, the parent owns the expanded state. */
  expanded?: boolean;
  /** Uncontrolled initial value; ignored when `expanded` is provided. */
  defaultExpanded?: boolean;
}

const props = withDefaults(defineProps<ChatToolCallItemProps>(), {
  toolCallId: undefined,
  title: undefined,
  durationMs: undefined,
  startedAt: undefined,
  input: undefined,
  outputPreview: undefined,
  scope: undefined,
  expanded: undefined,
  defaultExpanded: false
});

const emit = defineEmits<{
  (event: "update:expanded", value: boolean): void;
}>();

const { t } = useI18n();

const isControlled = computed(() => props.expanded !== undefined);

// Uncontrolled-mode backing state is persisted per-session in
// localStorage via `useChatToolExpand`. The persistence key is
// `kairox.chatToolExpand.${currentSessionId}.${toolCallId || toolId}`,
// so a row remembers its expand state across reloads and session
// switches. Controlled-mode rendering bypasses this — the parent owns
// the visible state in that case.
const session = useSessionStore();
const { currentSessionId } = storeToRefs(session);
const persistenceKey = computed(() => props.toolCallId ?? props.toolId);
// `useChatToolExpand` expects a stable string key. We read it once at
// setup; if a caller changes `toolCallId` mid-flight (not expected for
// the chat stream — trace entry ids are stable) the persistence key
// would not update, which matches our scoping intent.
const { isExpanded: persistedExpanded, toggle: togglePersisted } = useChatToolExpand(
  currentSessionId,
  persistenceKey.value
);

// Seed persisted state from `defaultExpanded` only when we have no
// previously saved value AND no live override. This preserves the
// "expand by default" affordance for callers that opt in without
// clobbering a user's prior persisted choice.
if (!isControlled.value && props.defaultExpanded && !persistedExpanded.value) {
  persistedExpanded.value = true;
}

const isExpanded = computed<boolean>(() =>
  isControlled.value ? Boolean(props.expanded) : persistedExpanded.value
);

function toggle() {
  const next = !isExpanded.value;
  // Always notify parent so controlled callers can react.
  emit("update:expanded", next);
  // In uncontrolled mode we own the state locally and persist the
  // change; in controlled mode the rendered state is driven entirely
  // by the prop and we must not flip locally (the parent will update
  // the prop, or not).
  if (!isControlled.value) {
    togglePersisted();
  }
}

const statusIcon: Record<ChatToolCallItemProps["status"], string> = {
  running: "⏳",
  completed: "✅",
  failed: "❌",
  pending: "🔑"
};

const { iconFor } = useToolIcon();
const toolIcon = computed(() => iconFor(props.toolId));

const statusLabel = computed(() => t(`chatStream.toolCall.status.${props.status}`));

const durationLabel = computed(() => {
  if (props.durationMs == null) return null;
  const formatted = `${(props.durationMs / 1000).toFixed(1)}s`;
  if (props.status === "failed") {
    return t("chatStream.toolCall.timing.failedAfter", { duration: formatted });
  }
  return formatted;
});

/**
 * Build a compact human-friendly relative-time string ("3s", "1m",
 * "3m 20s", "2h 15m") from an elapsed millisecond count. We deliberately
 * stringify in JS rather than adding ICU pluralization to the locale
 * files — the chunking is identical across locales and the "just now"
 * threshold is handled by `startedAgoLabel` below using a separate key.
 */
function formatRelativeChunked(elapsedMs: number): string {
  const seconds = Math.max(0, Math.floor(elapsedMs / 1000));
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return s === 0 ? `${m}m` : `${m}m ${s}s`;
  }
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

const startedAgoLabel = computed<string | null>(() => {
  if (props.startedAt == null) return null;
  // `Date.now()` is mockable via `vi.setSystemTime`. We intentionally do
  // NOT poll on a timer — the chat stream re-renders frequently enough
  // (props change as `durationMs` lands) that the relative label stays
  // fresh, and a ticking clock would force unnecessary repaints across
  // many rows.
  const elapsedMs = Date.now() - props.startedAt;
  // Under ~3s the "Xs ago" granularity is noise — collapse to "just
  // now". Tests rely on the boundary: 2s → "just now", 3s → "3s ago".
  if (elapsedMs < 3000) return t("chatStream.toolCall.timing.startedJustNow");
  return t("chatStream.toolCall.timing.startedAgo", {
    relative: formatRelativeChunked(elapsedMs)
  });
});

const toggleLabel = computed(() =>
  isExpanded.value ? t("chatStream.toolCall.collapse") : t("chatStream.toolCall.expand")
);

const outputIsDiff = computed(() =>
  props.outputPreview ? isDiffShaped(props.outputPreview) : false
);
</script>

<template>
  <div
    :class="['chat-tool-call', `chat-tool-call--${props.status}`]"
    data-test="chat-tool-call-item"
  >
    <div class="chat-tool-call__row" @click="toggle">
      <span
        class="chat-tool-call__status"
        role="img"
        :aria-label="statusLabel"
        :title="statusLabel"
      >
        {{ statusIcon[props.status] }}
      </span>
      <span
        class="chat-tool-call__tool-icon"
        data-test="chat-tool-call-tool-icon"
        role="img"
        aria-hidden="true"
        :title="props.toolId"
      >
        {{ toolIcon }}
      </span>
      <span class="chat-tool-call__tool">
        <span class="chat-tool-call__tool-text" :title="props.toolId">
          {{ props.title || props.toolId }}
        </span>
      </span>
      <KxTag v-if="props.scope" class="chat-tool-call__scope" tone="info" size="sm">
        {{ props.scope }}
      </KxTag>
      <span
        v-if="durationLabel"
        class="chat-tool-call__duration"
        :title="t('chatStream.toolCall.duration')"
      >
        {{ durationLabel }}
      </span>
      <KxBadge v-if="props.status === 'pending'" class="chat-tool-call__badge" tone="warning">
        {{ statusLabel }}
      </KxBadge>
      <KxIconButton
        :label="toggleLabel"
        :title="toggleLabel"
        size="sm"
        variant="ghost"
        data-test="chat-tool-call-toggle"
        @click.stop="toggle"
      >
        <span aria-hidden="true">{{ isExpanded ? "▾" : "▸" }}</span>
      </KxIconButton>
    </div>
    <div v-if="isExpanded" class="chat-tool-call__detail">
      <div
        v-if="startedAgoLabel"
        class="chat-tool-call__started-ago"
        data-test="chat-tool-call-started-ago"
      >
        {{ startedAgoLabel }}
      </div>
      <div v-if="props.input" class="chat-tool-call__section">
        <span class="chat-tool-call__label">{{ t("chatStream.toolCall.input") }}:</span>
        <pre class="chat-tool-code">{{ props.input }}</pre>
      </div>
      <div v-if="props.outputPreview" class="chat-tool-call__section">
        <span class="chat-tool-call__label">{{ t("chatStream.toolCall.output") }}:</span>
        <DiffPreview v-if="outputIsDiff" :text="props.outputPreview" />
        <pre v-else class="chat-tool-code">{{ props.outputPreview }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.chat-tool-call {
  font-size: 12px;
  box-sizing: border-box;
  min-width: 0;
  max-width: 100%;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  overflow: hidden;
}
.chat-tool-call--failed {
  border-color: color-mix(in srgb, var(--app-error-color) 45%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-error-color) 6%, var(--app-card-color));
}
.chat-tool-call--pending {
  background: color-mix(in srgb, var(--app-warning-color) 8%, transparent);
}
.chat-tool-call__row {
  display: flex;
  min-width: 0;
  max-width: 100%;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  cursor: pointer;
}
.chat-tool-call__row:hover {
  background: var(--app-hover-color);
}
.chat-tool-call__status {
  font-size: 12px;
  line-height: 1;
}
.chat-tool-call__tool-icon {
  font-size: 12px;
  line-height: 1;
}
.chat-tool-call__tool {
  flex: 1;
  min-width: 0;
  font-weight: 500;
  overflow: hidden;
}
.chat-tool-call__tool-text {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}
.chat-tool-call__scope {
  font-size: 10px;
}
.chat-tool-call__duration {
  color: var(--app-text-color-3);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.chat-tool-call__badge {
  font-size: 10px;
}
.chat-tool-call__detail {
  padding: 6px 8px 8px;
  border-top: 1px solid var(--app-border-color);
  background: var(--app-card-color);
}
.chat-tool-call__started-ago {
  color: var(--app-text-color-3);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  margin-bottom: 4px;
}
.chat-tool-call__section + .chat-tool-call__section {
  margin-top: 6px;
}
.chat-tool-call__label {
  font-weight: 600;
  font-size: 11px;
  color: var(--app-text-color-2);
}
.chat-tool-code {
  margin: 2px 0 0;
  padding: 6px 8px;
  background: var(--app-code-bg);
  color: var(--app-text-color);
  border-radius: 4px;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
