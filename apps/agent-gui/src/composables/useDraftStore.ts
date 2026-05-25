import { invoke } from "@tauri-apps/api/core";

const PLACEHOLDER_DRAFT_PREFIX = "new-session:";
const LOCAL_DRAFT_STORAGE_PREFIX = "kairox.composer-draft:";

function isPlaceholderDraftKey(sessionId: string): boolean {
  return sessionId.startsWith(PLACEHOLDER_DRAFT_PREFIX);
}

function localDraftStorageKey(sessionId: string): string {
  return `${LOCAL_DRAFT_STORAGE_PREFIX}${sessionId}`;
}

export function useDraftStore() {
  async function loadDraft(sessionId: string): Promise<string> {
    if (isPlaceholderDraftKey(sessionId)) {
      try {
        return globalThis.localStorage?.getItem(localDraftStorageKey(sessionId)) ?? "";
      } catch {
        return "";
      }
    }

    try {
      return await invoke<string>("get_draft", { sessionId });
    } catch {
      return "";
    }
  }

  async function saveDraft(sessionId: string, text: string): Promise<void> {
    if (isPlaceholderDraftKey(sessionId)) {
      try {
        const key = localDraftStorageKey(sessionId);
        if (text) {
          globalThis.localStorage?.setItem(key, text);
        } else {
          globalThis.localStorage?.removeItem(key);
        }
      } catch {
        // Best-effort: silently ignore save failures
      }
      return;
    }

    try {
      await invoke("save_draft", {
        request: { session_id: sessionId, draft_text: text }
      });
    } catch {
      // Best-effort: silently ignore save failures
    }
  }

  async function clearDraft(sessionId: string): Promise<void> {
    await saveDraft(sessionId, "");
  }

  return { loadDraft, saveDraft, clearDraft };
}
