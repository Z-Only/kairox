import { ref, onMounted } from "vue";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useUiStore } from "@/stores/ui";

interface UpdateInfo {
  version: string;
  body?: string;
  date?: string;
}

export const updateAvailable = ref(false);
export const updateInfo = ref<UpdateInfo | null>(null);
export const checkingForUpdate = ref(false);
export const downloadingUpdate = ref(false);

/**
 * Check for updates using the Tauri updater plugin.
 * On startup, silently checks and notifies the user if an update is available.
 */
export async function checkForUpdate(): Promise<void> {
  if (checkingForUpdate.value) return;

  const ui = useUiStore();
  checkingForUpdate.value = true;
  try {
    const update = await check();
    if (update) {
      updateAvailable.value = true;
      updateInfo.value = {
        version: update.version,
        body: update.body ?? undefined
      };
      ui.pushNotification(
        "info",
        `Kairox ${update.version} is available. Click to update.`
      );
    }
  } catch (e) {
    // Silently ignore update check failures (offline, etc.)
    console.debug("Update check failed:", e);
  } finally {
    checkingForUpdate.value = false;
  }
}

/**
 * Download and install the update, then relaunch the app.
 */
export async function downloadAndInstallUpdate(): Promise<void> {
  if (downloadingUpdate.value) return;

  const ui = useUiStore();
  downloadingUpdate.value = true;
  try {
    const update = await check();
    if (!update) {
      ui.pushNotification("info", "No update available.");
      return;
    }

    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          console.debug(
            `Download started: ${event.data.contentLength ?? 0} bytes`
          );
          break;
        case "Progress":
          break;
        case "Finished":
          console.debug("Download finished");
          break;
      }
    });

    ui.pushNotification("info", "Update installed. Relaunching...");
    await relaunch();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    ui.pushNotification("error", `Update failed: ${msg}`);
    console.error("Update download/install failed:", e);
  } finally {
    downloadingUpdate.value = false;
    updateAvailable.value = false;
  }
}

/**
 * Vue composable that checks for updates on mount.
 * Call in App.vue or a top-level layout component.
 */
export function useUpdater() {
  onMounted(() => {
    checkForUpdate();
  });

  return {
    updateAvailable,
    updateInfo,
    checkingForUpdate,
    downloadingUpdate,
    checkForUpdate,
    downloadAndInstallUpdate
  };
}
