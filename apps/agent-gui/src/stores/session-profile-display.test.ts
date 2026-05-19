import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "./session";

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
    expect(session.activeProfileDisplay).toBe("Anthropic · Claude 3.5 Sonnet");
  });

  it("falls back to the alias when profile details are unavailable", () => {
    const session = useSessionStore();
    session.currentProfile = "deep";

    expect(session.activeProfileDisplay).toBe("deep");
  });

  it("maps default profile aliases to provider and model display labels", () => {
    const session = useSessionStore();
    session.currentProfile = "default";
    session.profileInfos = [
      {
        alias: "default",
        provider: "openai",
        model_id: "gpt-4o",
        local: false,
        has_api_key: true
      }
    ];

    expect(session.activeProfileDisplay).toBe("OpenAI · GPT-4o");
    expect(session.activeProfileDisplay).not.toBe("default");
  });

  it("shows reasoning effort for profiles that support reasoning controls", () => {
    const session = useSessionStore();
    session.currentProfile = "smart";
    session.currentReasoningEffort = "high";
    session.profileInfos = [
      {
        alias: "smart",
        provider: "openai",
        model_id: "gpt-5.2",
        local: false,
        has_api_key: true,
        supports_reasoning: true
      }
    ];

    expect(session.activeProfileDisplay).toBe("OpenAI · GPT-5.2 · high");
  });

  it("omits reasoning effort for profiles without reasoning controls", () => {
    const session = useSessionStore();
    session.currentProfile = "fast";
    session.currentReasoningEffort = "high";
    session.profileInfos = [
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o-mini",
        local: false,
        has_api_key: true,
        supports_reasoning: false
      }
    ];

    expect(session.activeProfileDisplay).toBe("OpenAI · GPT-4o Mini");
  });

  it("defaults reasoning-capable profile display to low when no effort is set", () => {
    const session = useSessionStore();
    session.currentProfile = "smart";
    session.currentReasoningEffort = null;
    session.profileInfos = [
      {
        alias: "smart",
        provider: "openai",
        model_id: "gpt-5.2",
        local: false,
        has_api_key: true,
        supports_reasoning: true
      }
    ];

    expect(session.activeProfileDisplay).toBe("OpenAI · GPT-5.2 · low");
  });

  it("updates current reasoning effort from ModelProfileSwitched events", () => {
    const session = useSessionStore();

    session.applyEvent({
      schema_version: 1,
      workspace_id: "wrk_test",
      session_id: "ses_test",
      timestamp: new Date().toISOString(),
      source_agent_id: "agent_system",
      privacy: "minimal_trace",
      event_type: "ModelProfileSwitched",
      payload: {
        type: "ModelProfileSwitched",
        from_profile: "fast",
        to_profile: "smart",
        reasoning_effort: "xhigh",
        effective_at: "2026-05-18T10:00:00Z",
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry"
      }
    });

    expect(session.currentProfile).toBe("smart");
    expect(session.currentReasoningEffort).toBe("xhigh");
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
      expect(session.activeProfileDisplay).toBe("OpenAI · GPT-4o");
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });
});
