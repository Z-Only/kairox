import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../session";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockedInvoke = vi.mocked(invoke);

describe("session profile display", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("maps the active profile alias to provider and model id", async () => {
    mockedInvoke.mockResolvedValueOnce([
      {
        alias: "deep",
        provider: "anthropic",
        model_id: "claude-3-5-sonnet",
        local: false,
        has_api_key: true
      }
    ]);
    const session = useSessionStore();
    session.currentProfile = "deep";

    await session.loadProfileInfo();

    expect(mockedInvoke).toHaveBeenCalledWith("get_profile_info");
    expect(session.activeProfileDisplay).toBe("anthropic / claude-3-5-sonnet");
  });

  it("falls back to the alias when profile details are unavailable", () => {
    const session = useSessionStore();
    session.currentProfile = "deep";

    expect(session.activeProfileDisplay).toBe("deep");
  });

  it("keeps alias fallback and allows retry when profile loading fails", async () => {
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    try {
      mockedInvoke
        .mockRejectedValueOnce(new Error("profile info unavailable"))
        .mockResolvedValueOnce([
          {
            alias: "deep",
            provider: "openai",
            model_id: "gpt-4o",
            local: false,
            has_api_key: true
          }
        ]);
      const session = useSessionStore();
      session.currentProfile = "deep";

      await expect(session.loadProfileInfo()).resolves.toBeUndefined();

      expect(session.activeProfileDisplay).toBe("deep");
      expect(session.loadingProfileInfo).toBe(false);

      await session.loadProfileInfo();

      expect(mockedInvoke).toHaveBeenCalledTimes(2);
      expect(session.activeProfileDisplay).toBe("openai / gpt-4o");
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });
});
