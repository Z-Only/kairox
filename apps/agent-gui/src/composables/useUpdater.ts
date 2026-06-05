import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useStorage } from "@vueuse/core";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { useUiStore } from "@/stores/ui";
import { i18n } from "@/locales";

interface UpdateInfo {
  version: string;
  body?: string;
  date?: string;
}

export type UpdateCheckInterval = "30m" | "1h" | "6h" | "12h" | "24h";

const INTERVAL_MS: Record<UpdateCheckInterval, number> = {
  "30m": 30 * 60_000,
  "1h": 60 * 60_000,
  "6h": 6 * 60 * 60_000,
  "12h": 12 * 60 * 60_000,
  "24h": 24 * 60 * 60_000
};

export const updateAvailable = ref(false);
export const updateInfo = ref<UpdateInfo | null>(null);
export const checkingForUpdate = ref(false);
export const downloadingUpdate = ref(false);
export const currentVersion = ref<string | null>(null);
export const lastCheckTime = ref<number | null>(null);
export const lastCheckError = ref<string | null>(null);

export async function checkForUpdate(silent = true): Promise<void> {
  if (checkingForUpdate.value) return;

  const ui = useUiStore();
  const t = i18n.global.t;
  checkingForUpdate.value = true;
  lastCheckError.value = null;
  try {
    const update = await check();
    lastCheckTime.value = Date.now();
    if (update) {
      updateAvailable.value = true;
      updateInfo.value = {
        version: update.version,
        body: update.body ?? undefined
      };
      ui.pushNotification("info", t("notifications.updateNewVersion", { version: update.version }));
    } else if (!silent) {
      ui.pushNotification("info", t("notifications.updateLatestVersion"));
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    lastCheckError.value = msg;
    if (!silent) {
      ui.pushNotification("error", t("notifications.updateCheckError", { error: msg }));
    }
    console.debug("Update check failed:", e);
  } finally {
    checkingForUpdate.value = false;
  }
}

export async function downloadAndInstallUpdate(): Promise<void> {
  if (downloadingUpdate.value) return;

  const ui = useUiStore();
  const t = i18n.global.t;
  downloadingUpdate.value = true;
  try {
    const update = await check();
    if (!update) {
      ui.pushNotification("info", t("notifications.updateNoUpdate"));
      return;
    }

    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          console.debug(`Download started: ${event.data.contentLength ?? 0} bytes`);
          break;
        case "Progress":
          break;
        case "Finished":
          console.debug("Download finished");
          break;
      }
    });

    ui.pushNotification("info", t("notifications.updateInstalled"));
    await relaunch();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    ui.pushNotification("error", t("notifications.updateFailed", { error: msg }));
    console.error("Update download/install failed:", e);
  } finally {
    downloadingUpdate.value = false;
    updateAvailable.value = false;
  }
}

export function useUpdater() {
  const autoCheckEnabled = useStorage("kairox.update-auto-check", true, undefined, {
    flush: "sync"
  });
  const autoDownloadEnabled = useStorage("kairox.update-auto-download", false, undefined, {
    flush: "sync"
  });
  const checkInterval = useStorage<UpdateCheckInterval>(
    "kairox.update-check-interval",
    "6h",
    undefined,
    {
      flush: "sync",
      serializer: {
        read: (v) => (v && v in INTERVAL_MS ? (v as UpdateCheckInterval) : "6h"),
        write: (v) => v
      }
    }
  );

  const checkIntervalMs = computed(() => INTERVAL_MS[checkInterval.value]);

  let intervalHandle: ReturnType<typeof setInterval> | null = null;

  function clearSchedule() {
    if (intervalHandle !== null) {
      clearInterval(intervalHandle);
      intervalHandle = null;
    }
  }

  function startSchedule() {
    clearSchedule();
    if (!autoCheckEnabled.value) return;
    intervalHandle = setInterval(() => {
      void checkForUpdate();
    }, checkIntervalMs.value);
  }

  async function loadVersion() {
    try {
      currentVersion.value = await getVersion();
    } catch {
      currentVersion.value = null;
    }
  }

  watch([autoCheckEnabled, checkInterval], () => {
    startSchedule();
  });

  watch([updateAvailable, autoDownloadEnabled], ([available, autoDownload]) => {
    if (available && autoDownload) {
      void downloadAndInstallUpdate();
    }
  });

  onMounted(async () => {
    await loadVersion();
    if (autoCheckEnabled.value) {
      await checkForUpdate();
      startSchedule();
    }
  });

  onUnmounted(() => {
    clearSchedule();
  });

  return {
    autoCheckEnabled,
    autoDownloadEnabled,
    checkInterval,
    checkIntervalMs,
    currentVersion,
    lastCheckTime,
    lastCheckError,
    updateAvailable,
    updateInfo,
    checkingForUpdate,
    downloadingUpdate,
    checkForUpdate,
    downloadAndInstallUpdate
  };
}
