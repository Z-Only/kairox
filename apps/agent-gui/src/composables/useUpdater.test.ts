import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// Mock Tauri updater plugin
const mockDownloadAndInstall = vi.fn();
const mockCheck = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => mockCheck(...args)
}));

const mockRelaunch = vi.fn();
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: (...args: unknown[]) => mockRelaunch(...args)
}));

const mockGetVersion = vi.fn();
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: (...args: unknown[]) => mockGetVersion(...args)
}));

const mockT = vi.fn((key: string, ...args: unknown[]) => {
  const params =
    args.length > 0 && typeof args[0] === "object" && args[0] !== null
      ? (args[0] as Record<string, unknown>)
      : undefined;
  if (params) {
    return Object.entries(params).reduce((s, [k, v]) => s.replace(`{${k}}`, String(v)), key);
  }
  return key;
});

vi.mock("@/locales", () => ({
  i18n: {
    global: {
      get t() {
        return mockT;
      }
    }
  }
}));

import {
  updateAvailable,
  updateInfo,
  checkingForUpdate,
  downloadingUpdate,
  lastCheckTime,
  lastCheckError,
  checkForUpdate,
  downloadAndInstallUpdate
} from "./useUpdater";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
  setActivePinia(createPinia());
  updateAvailable.value = false;
  updateInfo.value = null;
  checkingForUpdate.value = false;
  downloadingUpdate.value = false;
  lastCheckTime.value = null;
  lastCheckError.value = null;
  mockCheck.mockReset();
  mockRelaunch.mockReset();
  mockDownloadAndInstall.mockReset();
  mockGetVersion.mockReset();
});

describe("checkForUpdate", () => {
  it("sets updateAvailable and updateInfo when an update exists", async () => {
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: "New features",
      downloadAndInstall: mockDownloadAndInstall
    });

    await checkForUpdate();

    expect(updateAvailable.value).toBe(true);
    expect(updateInfo.value).toEqual({ version: "2.0.0", body: "New features" });
  });

  it("pushes info notification when update is available", async () => {
    mockCheck.mockResolvedValue({
      version: "3.1.0",
      body: null,
      downloadAndInstall: mockDownloadAndInstall
    });

    await checkForUpdate();

    const ui = useUiStore();
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("info");
    expect(mockT).toHaveBeenCalledWith("notifications.updateNewVersion", { version: "3.1.0" });
  });

  it("does nothing when no update is available (check returns null)", async () => {
    mockCheck.mockResolvedValue(null);

    await checkForUpdate();

    expect(updateAvailable.value).toBe(false);
    expect(updateInfo.value).toBeNull();
  });

  it("silently handles errors without crashing", async () => {
    mockCheck.mockRejectedValue(new Error("Network error"));

    await checkForUpdate();

    expect(updateAvailable.value).toBe(false);
    expect(checkingForUpdate.value).toBe(false);
  });

  it("prevents concurrent checks (early return when already checking)", async () => {
    checkingForUpdate.value = true;
    mockCheck.mockResolvedValue({ version: "2.0.0", body: null });

    await checkForUpdate();

    expect(mockCheck).not.toHaveBeenCalled();
  });

  it("resets checkingForUpdate flag after completion", async () => {
    mockCheck.mockResolvedValue(null);

    await checkForUpdate();

    expect(checkingForUpdate.value).toBe(false);
  });

  it("handles update with undefined body gracefully", async () => {
    mockCheck.mockResolvedValue({
      version: "1.5.0",
      body: undefined,
      downloadAndInstall: mockDownloadAndInstall
    });

    await checkForUpdate();

    expect(updateInfo.value).toEqual({ version: "1.5.0", body: undefined });
  });

  it("records lastCheckTime on success", async () => {
    mockCheck.mockResolvedValue(null);

    await checkForUpdate();

    expect(lastCheckTime.value).toBeGreaterThan(0);
  });

  it("records lastCheckError on failure", async () => {
    mockCheck.mockRejectedValue(new Error("Offline"));

    await checkForUpdate();

    expect(lastCheckError.value).toBe("Offline");
  });

  it("clears lastCheckError on successful check", async () => {
    lastCheckError.value = "previous error";
    mockCheck.mockResolvedValue(null);

    await checkForUpdate();

    expect(lastCheckError.value).toBeNull();
  });

  it("pushes up-to-date notification for non-silent check with no update", async () => {
    mockCheck.mockResolvedValue(null);

    await checkForUpdate(false);

    const ui = useUiStore();
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].message).toContain("updateLatestVersion");
  });

  it("does not push notification for silent check with no update", async () => {
    mockCheck.mockResolvedValue(null);

    await checkForUpdate(true);

    const ui = useUiStore();
    expect(ui.notifications).toHaveLength(0);
  });

  it("pushes error notification for non-silent check failure", async () => {
    mockCheck.mockRejectedValue(new Error("DNS fail"));

    await checkForUpdate(false);

    const ui = useUiStore();
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("error");
    expect(mockT).toHaveBeenCalledWith("notifications.updateCheckError", { error: "DNS fail" });
  });
});

describe("downloadAndInstallUpdate", () => {
  it("downloads, installs, and relaunches the app", async () => {
    mockDownloadAndInstall.mockResolvedValue(undefined);
    mockRelaunch.mockResolvedValue(undefined);
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: "changelog",
      downloadAndInstall: mockDownloadAndInstall
    });

    await downloadAndInstallUpdate();

    expect(mockDownloadAndInstall).toHaveBeenCalledWith(expect.any(Function));
    expect(mockRelaunch).toHaveBeenCalled();
  });

  it("pushes success notification before relaunch", async () => {
    mockDownloadAndInstall.mockResolvedValue(undefined);
    mockRelaunch.mockResolvedValue(undefined);
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: null,
      downloadAndInstall: mockDownloadAndInstall
    });

    await downloadAndInstallUpdate();

    const ui = useUiStore();
    const installNotice = ui.notifications.find((n) => n.message.includes("updateInstalled"));
    expect(installNotice).toBeDefined();
  });

  it("pushes info notification when no update is available", async () => {
    mockCheck.mockResolvedValue(null);

    await downloadAndInstallUpdate();

    const ui = useUiStore();
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].message).toContain("updateNoUpdate");
    expect(mockRelaunch).not.toHaveBeenCalled();
  });

  it("pushes error notification on download failure", async () => {
    mockDownloadAndInstall.mockRejectedValue(new Error("Disk full"));
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: null,
      downloadAndInstall: mockDownloadAndInstall
    });

    await downloadAndInstallUpdate();

    const ui = useUiStore();
    const errorNotice = ui.notifications.find((n) => n.level === "error");
    expect(errorNotice).toBeDefined();
    expect(mockT).toHaveBeenCalledWith("notifications.updateFailed", { error: "Disk full" });
    expect(mockRelaunch).not.toHaveBeenCalled();
  });

  it("prevents concurrent downloads", async () => {
    downloadingUpdate.value = true;
    mockCheck.mockResolvedValue({ version: "2.0.0", downloadAndInstall: mockDownloadAndInstall });

    await downloadAndInstallUpdate();

    expect(mockCheck).not.toHaveBeenCalled();
  });

  it("resets downloadingUpdate and updateAvailable after completion", async () => {
    mockDownloadAndInstall.mockResolvedValue(undefined);
    mockRelaunch.mockResolvedValue(undefined);
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: null,
      downloadAndInstall: mockDownloadAndInstall
    });
    updateAvailable.value = true;

    await downloadAndInstallUpdate();

    expect(downloadingUpdate.value).toBe(false);
    expect(updateAvailable.value).toBe(false);
  });

  it("invokes download callback with event progress", async () => {
    mockDownloadAndInstall.mockImplementation(async (cb: (event: unknown) => void) => {
      cb({ event: "Started", data: { contentLength: 1024 } });
      cb({ event: "Progress", data: { chunkLength: 512 } });
      cb({ event: "Finished", data: {} });
    });
    mockRelaunch.mockResolvedValue(undefined);
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: null,
      downloadAndInstall: mockDownloadAndInstall
    });

    await downloadAndInstallUpdate();

    expect(mockDownloadAndInstall).toHaveBeenCalled();
    expect(mockRelaunch).toHaveBeenCalled();
  });
});
