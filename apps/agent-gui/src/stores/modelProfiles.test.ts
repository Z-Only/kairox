import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { formatError, useModelProfilesStore } from "@/stores/modelProfiles";
import type { ProfileSettingsInput, ProfileSettingsView } from "@/generated/commands";

vi.mock("@/generated/commands", () => ({
  commands: {
    listProfileSettings: vi.fn(),
    upsertProfileSettings: vi.fn(),
    setProfileEnabled: vi.fn(),
    deleteProfileSettings: vi.fn(),
    moveProfileInOrder: vi.fn(),
    testModelConnectivity: vi.fn(),
    testUrlConnectivity: vi.fn(),
    openProfilesConfigFile: vi.fn()
  }
}));

import { commands } from "@/generated/commands";
const mockedCommands = vi.mocked(commands);

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function makeProfile(overrides: Partial<ProfileSettingsView> = {}): ProfileSettingsView {
  return {
    alias: "fast",
    display_name: "Fast",
    enabled: true,
    scope: "User",
    path: "/mock/profile.toml",
    effective: true,
    ...overrides
  } as ProfileSettingsView;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("formatError", () => {
  it("returns Error.message when given an Error", () => {
    expect(formatError(new Error("boom"))).toBe("boom");
  });

  it("falls back to String() for non-Error values", () => {
    expect(formatError("plain string")).toBe("plain string");
    expect(formatError(42)).toBe("42");
    expect(formatError(null)).toBe("null");
  });
});

describe("useModelProfilesStore", () => {
  it("loadProfiles passes the source filter and stores returned profiles", async () => {
    const store = useModelProfilesStore();
    const profile = makeProfile();
    mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([profile]));

    await store.loadProfiles("user");

    expect(mockedCommands.listProfileSettings).toHaveBeenCalledWith("user", null);
    expect(store.profiles).toEqual([profile]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeNull();
  });

  it("loadProfiles defaults the source filter to null when omitted", async () => {
    const store = useModelProfilesStore();
    mockedCommands.listProfileSettings.mockResolvedValueOnce(ok([]));

    await store.loadProfiles();

    expect(mockedCommands.listProfileSettings).toHaveBeenCalledWith(null, null);
  });

  it("loadProfiles records the error and clears loading when the command rejects", async () => {
    const store = useModelProfilesStore();
    mockedCommands.listProfileSettings.mockRejectedValueOnce(new Error("backend offline"));

    await store.loadProfiles();

    expect(store.error).toBe("backend offline");
    expect(store.loading).toBe(false);
    expect(store.profiles).toEqual([]);
  });

  it("upsertProfile records the error and keeps loading false on rejection", async () => {
    const store = useModelProfilesStore();
    mockedCommands.upsertProfileSettings.mockRejectedValueOnce(new Error("invalid"));

    await store.upsertProfile({} as ProfileSettingsInput);

    expect(store.error).toBe("invalid");
    expect(store.loading).toBe(false);
  });

  it("setProfileEnabled clears busyAlias and records error on rejection", async () => {
    const store = useModelProfilesStore();
    mockedCommands.setProfileEnabled.mockRejectedValueOnce(new Error("forbidden"));

    await store.setProfileEnabled("fast", false);

    expect(store.error).toBe("forbidden");
    expect(store.busyAlias).toBeNull();
  });

  it("removeProfile clears busyAlias and records error on rejection", async () => {
    const store = useModelProfilesStore();
    mockedCommands.deleteProfileSettings.mockRejectedValueOnce(new Error("missing"));

    await store.removeProfile("fast");

    expect(store.error).toBe("missing");
    expect(store.busyAlias).toBeNull();
  });

  it("moveProfile clears busyAlias and records error on rejection", async () => {
    const store = useModelProfilesStore();
    mockedCommands.moveProfileInOrder.mockRejectedValueOnce(new Error("locked"));

    await store.moveProfile("fast", 1);

    expect(store.error).toBe("locked");
    expect(store.busyAlias).toBeNull();
  });

  it("testModelConnectivity clears busyAlias even when the command throws", async () => {
    const store = useModelProfilesStore();
    mockedCommands.testModelConnectivity.mockRejectedValueOnce(new Error("timeout"));

    await expect(store.testModelConnectivity("fast")).rejects.toThrow("timeout");
    expect(store.busyAlias).toBeNull();
  });

  it("testModelConnectivity forwards the project root scope", async () => {
    const store = useModelProfilesStore();
    mockedCommands.testModelConnectivity.mockResolvedValueOnce(ok({}));

    await store.testModelConnectivity("project-model", "/tmp/project");

    expect(mockedCommands.testModelConnectivity).toHaveBeenCalledWith(
      "project-model",
      "/tmp/project"
    );
  });

  it("openConfigFile swallows opener errors silently", async () => {
    const store = useModelProfilesStore();
    mockedCommands.openProfilesConfigFile.mockRejectedValueOnce(new Error("opener missing"));

    await expect(store.openConfigFile()).resolves.toBeUndefined();
    expect(store.error).toBeNull();
  });
});
