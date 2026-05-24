/**
 * Per-session expand/collapse persistence for chat tool-call rows.
 *
 * The chat stream renders each tool invocation as a collapsible row
 * (`ChatToolCallItem.vue`). Without persistence the row resets to
 * collapsed on every reload and on every session switch, which is
 * frustrating when the user has manually opened several invocations to
 * inspect their input/output.
 *
 * This composable backs the expand flag with `localStorage` keyed by
 * `kairox.chatToolExpand.${sessionId}.${toolCallId}`, so the choice
 * survives reloads and session switches. The default state remains
 * `false` (collapsed) — we only persist explicit user toggles.
 *
 * Safety properties:
 * - SSR-safe: every `localStorage` access is guarded with a
 *   `typeof window !== "undefined"` check and a `try/catch`.
 * - Failure-tolerant: corrupt JSON, missing values, throwing storage
 *   (Safari private mode, quota errors) all silently fall back to the
 *   default. The composable never throws.
 * - Session-scoped: when `sessionId` is `null` we skip persistence
 *   entirely — there is no meaningful key to write under yet, and we
 *   don't want a transient null-keyed entry to outlive the page.
 * - Reactive: when `sessionId` changes (user switches sessions) we
 *   re-read the stored value for the new key so the row matches the
 *   newly-mounted session's saved state.
 */
import { ref, watch, type Ref } from "vue";

const STORAGE_PREFIX = "kairox.chatToolExpand.";

interface UseChatToolExpandResult {
  isExpanded: Ref<boolean>;
  toggle: () => void;
}

function isStorageAvailable(): boolean {
  return typeof window !== "undefined" && typeof window.localStorage !== "undefined";
}

function storageKey(sessionId: string, toolCallId: string): string {
  return `${STORAGE_PREFIX}${sessionId}.${toolCallId}`;
}

function readPersisted(sessionId: string | null, toolCallId: string): boolean {
  if (!sessionId || !isStorageAvailable()) return false;
  try {
    const raw = window.localStorage.getItem(storageKey(sessionId, toolCallId));
    if (raw == null) return false;
    const parsed = JSON.parse(raw);
    return typeof parsed === "boolean" ? parsed : false;
  } catch {
    return false;
  }
}

function writePersisted(sessionId: string | null, toolCallId: string, value: boolean): void {
  if (!sessionId || !isStorageAvailable()) return;
  try {
    window.localStorage.setItem(storageKey(sessionId, toolCallId), JSON.stringify(value));
  } catch {
    // Best-effort: ignore quota / private-mode failures.
  }
}

export function useChatToolExpand(
  sessionId: Ref<string | null>,
  toolCallId: string
): UseChatToolExpandResult {
  const isExpanded = ref<boolean>(readPersisted(sessionId.value, toolCallId));

  // When the active session changes (user navigates between sessions and
  // the same component instance is reused), reload the persisted value
  // for the new key so the row mirrors the new session's saved state.
  watch(sessionId, (next) => {
    isExpanded.value = readPersisted(next, toolCallId);
  });

  function toggle(): void {
    const next = !isExpanded.value;
    isExpanded.value = next;
    writePersisted(sessionId.value, toolCallId, next);
  }

  return { isExpanded, toggle };
}
