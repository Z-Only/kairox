<script setup lang="ts">
/**
 * ChatPermissionItem — thin adapter from the chat-stream prop shape
 * to PermissionPrompt's TraceEntryData input.
 *
 * Wraps the existing PermissionPrompt so that tool / memory permission
 * prompts can be rendered inline inside the unified chat-stream feed
 * (Claude Code / Codex style) without duplicating allow/deny logic.
 * The local invoke + MCP trust flow live entirely in PermissionPrompt;
 * we only translate prop shapes here.
 *
 * The ChatPermissionStreamItem interface is intentionally re-declared
 * inline rather than imported from `@/types/chatStream`, keeping this
 * leaf component decoupled from the chat-stream value-shape (which is
 * still evolving across the v0.30.0 campaign).
 *
 * Keyboard shortcuts (R5-pkb): once the wrapping <article> has focus
 * (Tab-reachable via `tabindex="0"`), Y/Enter trigger Allow, N/Esc
 * trigger Deny, and D triggers Deny-once with a fallback to Deny if
 * no separate deny-once control is rendered. The handler delegates to
 * the underlying PermissionPrompt buttons by querying their stable
 * `data-test` anchors, so all permission logic (MCP trust opt-in,
 * `resolve_permission` invoke, error handling) stays in one place.
 */
import PermissionPrompt from "@/components/PermissionPrompt.vue";
import type { TraceEntryData } from "@/types/trace";

const { t } = useI18n();

const props = defineProps<{
  id: string;
  variant: "tool" | "memory";
  toolId?: string;
  title?: string;
  input?: string;
  reason?: string;
  scope?: string;
  content?: string;
  rawEvent?: string;
}>();

const rootEl = ref<HTMLElement | null>(null);

const adaptedEntry = computed<TraceEntryData>(() => ({
  id: props.id,
  kind: props.variant === "memory" ? "memory" : "permission",
  status: "pending",
  toolId: props.toolId,
  title: props.title ?? "",
  startedAt: 0,
  input: props.input,
  reason: props.reason,
  scope: props.scope,
  content: props.content,
  rawEvent: props.rawEvent,
  expanded: false
}));

/**
 * Locate the underlying PermissionPrompt control by stable test anchor
 * and dispatch a native click. Returns true when a matching button was
 * found and clicked, so callers can decide whether to swallow the key.
 */
function clickAnchor(selector: string): boolean {
  const anchor = rootEl.value?.querySelector<HTMLButtonElement>(selector);
  if (!anchor) return false;
  anchor.click();
  return true;
}

/**
 * Skip the shortcut handler when focus is inside an editable element
 * (e.g. a future input the prompt may render). Native shortcuts like
 * Y/N would otherwise hijack normal typing.
 */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

function onKeydown(event: KeyboardEvent) {
  // Modifier keys (Ctrl/Cmd/Alt) reserve combos for the host — don't intercept.
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (isEditableTarget(event.target)) return;

  const key = event.key;
  const lower = key.length === 1 ? key.toLowerCase() : key;
  let handled = false;

  if (lower === "y" || key === "Enter") {
    handled = clickAnchor('[data-test="permission-allow"]');
  } else if (lower === "n" || key === "Escape") {
    handled = clickAnchor('[data-test="permission-deny"]');
  } else if (lower === "d" && props.variant === "tool") {
    // Deny-once is not currently rendered by PermissionPrompt; fall back
    // to plain Deny so the hint stays meaningful when the dedicated
    // anchor is absent.
    handled =
      clickAnchor('[data-test="permission-deny-once"]') ||
      clickAnchor('[data-test="permission-deny"]');
  }

  if (handled) {
    event.preventDefault();
    // Prevent global hotkeys (e.g. composer shortcuts) from re-firing on the
    // same key press.
    event.stopPropagation();
  }
}
</script>

<template>
  <article
    ref="rootEl"
    class="chat-permission-item"
    data-test="chat-permission-item"
    :data-variant="variant"
    tabindex="0"
    @keydown="onKeydown"
  >
    <PermissionPrompt :entry="adaptedEntry" />
    <div
      v-if="variant === 'tool'"
      class="chat-permission-item__shortcuts"
      data-test="chat-permission-item-shortcuts"
      aria-hidden="true"
    >
      <span class="chat-permission-item__shortcut">
        {{ t("chatStream.permission.shortcut.allow") }}
      </span>
      <span class="chat-permission-item__shortcut">
        {{ t("chatStream.permission.shortcut.deny") }}
      </span>
      <span class="chat-permission-item__shortcut">
        {{ t("chatStream.permission.shortcut.denyOnce") }}
      </span>
    </div>
  </article>
</template>

<style scoped>
.chat-permission-item {
  display: block;
  width: 100%;
  max-width: 100%;
  /* Strip the default user-agent focus halo on the wrapping article and
     replace it with a subtle outline so keyboard focus is still obvious
     without clashing with the alert card border. */
  outline: none;
}

.chat-permission-item:focus-visible {
  outline: 2px solid var(--app-primary-color, #2080f0);
  outline-offset: 2px;
  border-radius: 4px;
}

.chat-permission-item__shortcuts {
  display: flex;
  gap: 8px;
  margin: 4px 8px 0;
  font-size: 10px;
  color: var(--app-text-color-3, #888);
  flex-wrap: wrap;
}

.chat-permission-item__shortcut {
  line-height: 1.4;
}
</style>
