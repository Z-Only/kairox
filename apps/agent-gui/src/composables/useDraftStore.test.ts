import { describe, it, expect, vi, beforeEach } from "vitest";
import { useDraftStore } from "./useDraftStore";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args)
}));

describe("useDraftStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("loadDraft returns saved text", async () => {
    mockInvoke.mockResolvedValueOnce("saved draft text");
    const { loadDraft } = useDraftStore();
    const result = await loadDraft("ses_1");
    expect(result).toBe("saved draft text");
    expect(mockInvoke).toHaveBeenCalledWith("get_draft", { sessionId: "ses_1" });
  });

  it("loadDraft returns empty string on error", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("fail"));
    const { loadDraft } = useDraftStore();
    const result = await loadDraft("ses_1");
    expect(result).toBe("");
  });

  it("saveDraft invokes backend command", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    const { saveDraft } = useDraftStore();
    await saveDraft("ses_1", "hello");
    expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
      request: { session_id: "ses_1", draft_text: "hello" }
    });
  });

  it("clearDraft saves empty text", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    const { clearDraft } = useDraftStore();
    await clearDraft("ses_1");
    expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
      request: { session_id: "ses_1", draft_text: "" }
    });
  });

  it("saveDraft silently ignores errors", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("disk full"));
    const { saveDraft } = useDraftStore();
    // Should not throw
    await expect(saveDraft("ses_1", "data")).resolves.toBeUndefined();
  });
});
