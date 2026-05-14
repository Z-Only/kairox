import { invoke } from "@tauri-apps/api/core";

export function useDraftStore() {
  async function loadDraft(sessionId: string): Promise<string> {
    try {
      return await invoke<string>("get_draft", { sessionId });
    } catch {
      return "";
    }
  }

  async function saveDraft(sessionId: string, text: string): Promise<void> {
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
