import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { modelHealthAdvice, useModelProfilesStore } from "@/stores/modelProfiles";
import type { ProfileSettingsView, ProfileSettingsInput } from "@/generated/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@/generated/commands", () => ({
  commands: {
    listProfileSettings: vi.fn(),
    upsertProfileSettings: vi.fn(),
    setProfileEnabled: vi.fn(),
    deleteProfileSettings: vi.fn(),
    moveProfileInOrder: vi.fn(),
    testModelConnectivity: vi.fn(),
    testUrlConnectivity: vi.fn(),
    refreshConfig: vi.fn(),
    refreshConfigForProject: vi.fn()
  }
}));

import { commands } from "@/generated/commands";
import { invoke } from "@tauri-apps/api/core";
const mockedCommands = vi.mocked(commands);
const mockedInvoke = vi.mocked(invoke);

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function err(error: string): { status: "error"; error: string } {
  return { status: "error", error };
}

function makeProfile(overrides: Partial<ProfileSettingsView> = {}): ProfileSettingsView {
  return {
    alias: "default",
    provider: "anthropic",
    model_id: "claude-sonnet-4-6",
    enabled: true,
    context_window: 200000,
    output_limit: 16384,
    temperature: null,
    top_p: null,
    top_k: null,
    max_tokens: 16384,
    base_url: null,
    api_key: null,
    api_key_env: "ANTHROPIC_API_KEY",
    client_identity: null,
    has_api_key: true,
    writable: true,
    config_path: "/home/user/.kairox/config.toml",
    source: "user",
    ...overrides
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("useModelProfilesStore", () => {
  describe("loadProfiles", () => {
    it("sets loading true on initial load and stores profiles", async () => {
      const store = useModelProfilesStore();
      const profile = makeProfile();
      mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([profile]));

      await store.loadProfiles();

      expect(mockedCommands.listProfileSettings).toHaveBeenCalledWith(null, null);
      expect(store.profiles).toEqual([profile]);
      expect(store.loading).toBe(false);
      expect(store.error).toBeNull();
    });

    it("sets refreshing true on subsequent loads", async () => {
      const store = useModelProfilesStore();
      const profile = makeProfile();
      mockedCommands.listProfileSettings.mockResolvedValue(ok([profile]));

      await store.loadProfiles();
      await store.loadProfiles();

      expect(store.refreshing).toBe(false);
      expect(store.loading).toBe(false);
    });

    it("passes source filter and project root", async () => {
      const store = useModelProfilesStore();
      mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([]));

      await store.loadProfiles("user", "/my/project");

      expect(mockedCommands.listProfileSettings).toHaveBeenCalledWith("user", "/my/project");
    });

    it("stores error on failure", async () => {
      const store = useModelProfilesStore();
      mockedCommands.listProfileSettings.mockResolvedValueOnce(err("config not found"));

      await store.loadProfiles();

      expect(store.error).toBe("config not found");
      expect(store.profiles).toEqual([]);
      expect(store.loading).toBe(false);
    });
  });

  describe("refreshRuntime", () => {
    it("calls refreshConfigForProject when projectRoot provided", async () => {
      const store = useModelProfilesStore();
      mockedCommands.refreshConfigForProject.mockResolvedValueOnce(undefined as never);

      await store.refreshRuntime("/my/project");

      expect(mockedCommands.refreshConfigForProject).toHaveBeenCalledWith("/my/project");
      expect(mockedCommands.refreshConfig).not.toHaveBeenCalled();
    });

    it("calls refreshConfig when no projectRoot", async () => {
      const store = useModelProfilesStore();
      mockedCommands.refreshConfig.mockResolvedValueOnce(undefined as never);

      await store.refreshRuntime();

      expect(mockedCommands.refreshConfig).toHaveBeenCalled();
      expect(mockedCommands.refreshConfigForProject).not.toHaveBeenCalled();
    });

    it("swallows errors silently", async () => {
      const store = useModelProfilesStore();
      mockedCommands.refreshConfig.mockRejectedValueOnce(new Error("boom"));

      await expect(store.refreshRuntime()).resolves.toBeUndefined();
      expect(store.error).toBeNull();
    });
  });

  describe("upsertProfile", () => {
    it("upserts and reloads profiles", async () => {
      const store = useModelProfilesStore();
      const input: ProfileSettingsInput = {
        alias: "fast",
        provider: "anthropic",
        model_id: "claude-haiku-4-5",
        context_window: 200000,
        output_limit: 8192,
        temperature: null,
        top_p: null,
        top_k: null,
        max_tokens: 8192,
        base_url: null,
        api_key: null,
        api_key_env: "ANTHROPIC_API_KEY",
        client_identity: null
      };
      const created = makeProfile({ alias: "fast", model_id: "claude-haiku-4-5" });
      mockedCommands.upsertProfileSettings.mockResolvedValueOnce(ok(created));
      mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([created]));

      await store.upsertProfile(input);

      expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(input);
      expect(store.profiles).toEqual([created]);
      expect(store.loading).toBe(false);
    });

    it("stores error on failure", async () => {
      const store = useModelProfilesStore();
      mockedCommands.upsertProfileSettings.mockResolvedValueOnce(err("invalid provider"));

      await store.upsertProfile({
        alias: "bad",
        provider: "unknown",
        model_id: "x",
        context_window: 0,
        output_limit: 0,
        temperature: null,
        top_p: null,
        top_k: null,
        max_tokens: 0,
        base_url: null,
        api_key: null,
        api_key_env: null,
        client_identity: null
      });

      expect(store.error).toBe("invalid provider");
      expect(store.loading).toBe(false);
    });
  });

  describe("setProfileEnabled", () => {
    it("sets busyAlias during operation and reloads", async () => {
      const store = useModelProfilesStore();
      const profile = makeProfile({ alias: "live", enabled: false });
      mockedCommands.setProfileEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([{ ...profile, enabled: true }]));

      await store.setProfileEnabled("live", true);

      expect(mockedCommands.setProfileEnabled).toHaveBeenCalledWith("live", true);
      expect(store.busyAlias).toBeNull();
      expect(store.profiles[0]?.enabled).toBe(true);
    });

    it("stores error and clears busyAlias on failure", async () => {
      const store = useModelProfilesStore();
      mockedCommands.setProfileEnabled.mockResolvedValueOnce(err("not found"));

      await store.setProfileEnabled("missing", true);

      expect(store.error).toBe("not found");
      expect(store.busyAlias).toBeNull();
    });
  });

  describe("removeProfile", () => {
    it("deletes and reloads profiles", async () => {
      const store = useModelProfilesStore();
      mockedCommands.deleteProfileSettings.mockResolvedValueOnce(ok(null));
      mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([]));

      await store.removeProfile("old");

      expect(mockedCommands.deleteProfileSettings).toHaveBeenCalledWith("old");
      expect(store.profiles).toEqual([]);
      expect(store.busyAlias).toBeNull();
    });

    it("stores error on failure", async () => {
      const store = useModelProfilesStore();
      mockedCommands.deleteProfileSettings.mockResolvedValueOnce(err("cannot delete active"));

      await store.removeProfile("active");

      expect(store.error).toBe("cannot delete active");
      expect(store.busyAlias).toBeNull();
    });
  });

  describe("moveProfile", () => {
    it("moves and reloads profiles", async () => {
      const store = useModelProfilesStore();
      const p1 = makeProfile({ alias: "a" });
      const p2 = makeProfile({ alias: "b" });
      mockedCommands.moveProfileInOrder.mockResolvedValueOnce(ok(null));
      mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([p2, p1]));

      await store.moveProfile("a", 1);

      expect(mockedCommands.moveProfileInOrder).toHaveBeenCalledWith("a", 1);
      expect(store.profiles).toEqual([p2, p1]);
      expect(store.busyAlias).toBeNull();
    });
  });

  describe("testModelConnectivity", () => {
    it("normalizes actionable health advice for failed connectivity statuses", () => {
      const cases = [
        {
          status: "empty_response",
          expected: {
            tone: "warning",
            label: "Empty response",
            recommendation: "Check model availability, quota, or plan access."
          }
        },
        {
          status: "auth_failed",
          expected: {
            tone: "danger",
            label: "Authentication failed",
            recommendation: "Check the API key or configured API key environment variable."
          }
        },
        {
          status: "quota_or_plan_blocked",
          expected: {
            tone: "danger",
            label: "Quota or plan blocked",
            recommendation: "Check quota, billing, and model access for this account."
          }
        },
        {
          status: "rate_limited",
          expected: {
            tone: "warning",
            label: "Rate limited",
            recommendation: "Wait and retry, or reduce request rate."
          }
        },
        {
          status: "network_error",
          expected: {
            tone: "warning",
            label: "Network error",
            recommendation: "Check network connectivity and the endpoint URL."
          }
        },
        {
          status: "permission_denied",
          expected: {
            tone: "danger",
            label: "Permission denied",
            recommendation: "Use an API key with access to this model or endpoint."
          }
        },
        {
          status: "model_unavailable",
          expected: {
            tone: "danger",
            label: "Model unavailable",
            recommendation: "Check the model ID, provider, and account access."
          }
        },
        {
          status: "server_error",
          expected: {
            tone: "warning",
            label: "Server error",
            recommendation: "Retry later or check provider status."
          }
        },
        {
          status: "invalid_config",
          expected: {
            tone: "danger",
            label: "Invalid configuration",
            recommendation: "Check provider, base URL, API key settings, and model ID."
          }
        },
        {
          status: "request_failed",
          expected: {
            tone: "danger",
            label: "Request failed",
            recommendation: "Review the raw error and model configuration."
          }
        }
      ];

      for (const { status, expected } of cases) {
        expect(
          modelHealthAdvice({
            ok: false,
            status,
            error: "raw detail",
            message: "backend message",
            response_preview: "must not matter"
          })
        ).toEqual(expected);
      }
    });

    it("localizes health advice with the provided translator", () => {
      const translations: Record<string, string> = {
        "models.healthAdvice_invalid_config_label": "配置无效",
        "models.healthAdvice_invalid_config_recommendation":
          "检查提供商、Base URL、API Key 设置和模型 ID。"
      };

      expect(
        modelHealthAdvice(
          {
            ok: false,
            status: "invalid_config",
            error: "missing base URL",
            message: "configuration error",
            response_preview: null
          },
          (key) => translations[key] ?? key
        )
      ).toEqual({
        tone: "danger",
        label: "配置无效",
        recommendation: "检查提供商、Base URL、API Key 设置和模型 ID。"
      });
    });

    it("returns connectivity result", async () => {
      const store = useModelProfilesStore();
      const result = { success: true, latency_ms: 42, error: null };
      mockedCommands.testModelConnectivity.mockResolvedValueOnce(ok(result) as never);

      const res = await store.testModelConnectivity("default");

      expect(mockedCommands.testModelConnectivity).toHaveBeenCalledWith("default", null);
      expect(res).toEqual(ok(result));
      expect(store.busyAlias).toBeNull();
    });

    it("passes project root when provided", async () => {
      const store = useModelProfilesStore();
      mockedCommands.testModelConnectivity.mockResolvedValueOnce(
        ok({ success: true, latency_ms: 10, error: null }) as never
      );

      await store.testModelConnectivity("default", "/proj");

      expect(mockedCommands.testModelConnectivity).toHaveBeenCalledWith("default", "/proj");
    });
  });

  describe("testUrlConnectivity", () => {
    it("delegates to command", async () => {
      const store = useModelProfilesStore();
      const result = { success: true, latency_ms: 50, error: null };
      mockedCommands.testUrlConnectivity.mockResolvedValueOnce(ok(result) as never);

      await store.testUrlConnectivity("https://api.example.com");

      expect(mockedCommands.testUrlConnectivity).toHaveBeenCalledWith("https://api.example.com");
    });
  });

  describe("openConfigFile", () => {
    it("invokes open_config_file_for_scope with project scope", async () => {
      const store = useModelProfilesStore();
      mockedInvoke.mockResolvedValueOnce(undefined);

      await store.openConfigFile("project", "/my/project");

      expect(mockedInvoke).toHaveBeenCalledWith("open_config_file_for_scope", {
        scope: "project",
        projectRoot: "/my/project"
      });
    });

    it("invokes open_config_file_for_scope with user scope by default", async () => {
      const store = useModelProfilesStore();
      mockedInvoke.mockResolvedValueOnce(undefined);

      await store.openConfigFile();

      expect(mockedInvoke).toHaveBeenCalledWith("open_config_file_for_scope", {
        scope: "user",
        projectRoot: null
      });
    });

    it("swallows errors silently", async () => {
      const store = useModelProfilesStore();
      mockedInvoke.mockRejectedValueOnce(new Error("no editor"));

      await expect(store.openConfigFile()).resolves.toBeUndefined();
    });
  });
});
