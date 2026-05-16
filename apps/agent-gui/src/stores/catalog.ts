// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore`, `ref`, and `computed` explicitly.
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  ServerEntryResponse,
  InstalledEntryResponse,
  InstallOutcomeResponse,
  InstallRequestPayload,
  CatalogQueryRequest,
  CatalogSourceViewResponse,
  AddCatalogSourceRequestPayload
} from "@/generated/commands";
import { useUiStore } from "@/stores/ui";

export type CatalogTab = "browse" | "installed";
export type TrustLevel = "unverified" | "community" | "verified";

export interface CatalogFilters {
  keyword: string;
  category: string | null;
  trustMin: TrustLevel | null;
}

const TRUST_ORDER: Record<TrustLevel, number> = {
  unverified: 0,
  community: 1,
  verified: 2
};

export const useCatalogStore = defineStore("catalog", () => {
  // ── state ────────────────────────────────────────────────────────
  const entries = ref<ServerEntryResponse[]>([]);
  const installed = ref<InstalledEntryResponse[]>([]);
  const installState = ref<Record<string, InstallOutcomeResponse>>({});
  const loading = ref(false);
  const error = ref<string | null>(null);
  const tab = ref<CatalogTab>("browse");
  const filters = ref<CatalogFilters>({
    keyword: "",
    category: null,
    trustMin: null
  });
  const sources = ref<CatalogSourceViewResponse[]>([]);
  const sourceFailures = ref<Record<string, string>>({});
  // Set of server_ids that are currently installed. Populated by
  // `fetchInstalled` and `checkInstalledStatus` for quick O(1) lookups
  // from CatalogDetail.vue without repeating the async fetch per entry.
  const installedServerNames = ref<Set<string>>(new Set());

  // Catalog id whose install-progress modal is currently visible. Hoisted out
  // of CatalogDetail.vue (which is unmounted whenever its NDrawer closes) so
  // the progress modal survives drawer dismissal mid-install. `null` = hidden.
  const currentInstallEntryId = ref<string | null>(null);

  // ── helpers ──────────────────────────────────────────────────────
  function reset(): void {
    entries.value = [];
    installed.value = [];
    installedServerNames.value = new Set();
    installState.value = {};
    loading.value = false;
    error.value = null;
    tab.value = "browse";
    filters.value = { keyword: "", category: null, trustMin: null };
    sources.value = [];
    sourceFailures.value = {};
    currentInstallEntryId.value = null;
  }

  function requestInstallProgress(entryId: string): void {
    // Clear any stale outcome from a previous install of the same entry
    // BEFORE flipping the visible-modal flag — otherwise InstallProgress
    // briefly renders the previous result alert (`inFlight = !outcome` is
    // false for one frame) before the new outcome lands.
    delete installState.value[entryId];
    currentInstallEntryId.value = entryId;
  }

  function dismissInstallProgress(): void {
    currentInstallEntryId.value = null;
  }

  // ── computeds ────────────────────────────────────────────────────
  const filteredEntries = computed<ServerEntryResponse[]>(() => {
    const kw = filters.value.keyword.trim().toLowerCase();
    const minOrder = filters.value.trustMin ? TRUST_ORDER[filters.value.trustMin] : -1;
    return entries.value.filter((e) => {
      if (kw) {
        const hay = `${e.display_name} ${e.summary} ${e.tags.join(" ")}`.toLowerCase();
        if (!hay.includes(kw)) return false;
      }
      if (filters.value.category && !e.categories.includes(filters.value.category)) {
        return false;
      }
      if (filters.value.trustMin) {
        const t = TRUST_ORDER[e.trust as TrustLevel] ?? 0;
        if (t < minOrder) return false;
      }
      return true;
    });
  });

  const hasEntries = computed(() => entries.value.length > 0);
  const installedCount = computed(() => installed.value.length);
  const allSourceIds = computed<string[]>(() => {
    const ids = sources.value.map((s) => s.id);
    if (!ids.includes("builtin")) {
      ids.unshift("builtin");
    }
    return ids;
  });

  function isSourceEnabled(id: string): boolean {
    const src = sources.value.find((s) => s.id === id);
    return src != null ? src.enabled : id === "builtin";
  }

  async function toggleSource(id: string): Promise<void> {
    const currentlyEnabled = isSourceEnabled(id);
    await setSourceEnabled(id, !currentlyEnabled);
    if (!currentlyEnabled && id !== "builtin") {
      await refreshCatalogSource(id);
    } else {
      await fetchCatalog();
    }
  }

  const visibleEntries = computed<ServerEntryResponse[]>(() =>
    filteredEntries.value.filter((e) => isSourceEnabled(e.source))
  );

  // ── actions ──────────────────────────────────────────────────────
  async function fetchCatalog(query: CatalogQueryRequest = {}): Promise<void> {
    const ui = useUiStore();
    loading.value = true;
    error.value = null;
    try {
      entries.value = await invoke<ServerEntryResponse[]>("list_catalog", {
        query
      });
    } catch (e) {
      error.value = String(e);
      ui.pushNotification("error", `Failed to load catalog: ${e}`);
    } finally {
      loading.value = false;
    }
  }

  async function fetchInstalled(): Promise<void> {
    const ui = useUiStore();
    try {
      const result = await invoke<InstalledEntryResponse[]>("list_installed_entries");
      // Replace array contents in-place via splice so Vue's reactive proxy
      // correctly notifies all watchers and computed properties. A plain
      // `installed.value = result` assignment can silently detach the proxy
      // in Pinia setup-stores when called from deeply-nested async flows.
      installed.value.splice(0, installed.value.length, ...result);
      installedServerNames.value = new Set(result.map((e) => e.server_id));
    } catch (e) {
      error.value = String(e);
      ui.pushNotification("error", `Failed to load installed entries: ${e}`);
    }
  }

  /** Lightweight installed-status refresh. Keeps both the installed array
   *  and the installedServerNames lookup set in sync so CatalogDetail can
   *  detect installed state after the drawer opens. */
  async function checkInstalledStatus(): Promise<void> {
    try {
      const result = await invoke<InstalledEntryResponse[]>("list_installed_entries");
      installed.value.splice(0, installed.value.length, ...result);
      installedServerNames.value = new Set(result.map((e) => e.server_id));
    } catch (e) {
      console.error("Failed to check installed status:", e);
    }
  }

  /** O(1) check whether a server name appears in the installed set. */
  function isServerInstalled(name: string): boolean {
    return installedServerNames.value.has(name);
  }

  async function getCatalogEntry(
    id: string,
    source?: string | null
  ): Promise<ServerEntryResponse | null> {
    const ui = useUiStore();
    try {
      return await invoke<ServerEntryResponse | null>("get_catalog_entry", {
        id,
        source: source ?? null
      });
    } catch (e) {
      console.error("Failed to get catalog entry:", e);
      ui.pushNotification("error", `Failed to load catalog entry ${id}: ${e}`);
      return null;
    }
  }

  async function installEntry(
    request: InstallRequestPayload
  ): Promise<InstallOutcomeResponse | null> {
    const ui = useUiStore();
    try {
      const outcome = await invoke<InstallOutcomeResponse>("install_catalog_entry", { request });
      installState.value[request.catalog_id] = outcome;
      if (outcome.kind === "installed") {
        await fetchInstalled();
      }
      return outcome;
    } catch (e) {
      console.error("Failed to install catalog entry:", e);
      ui.pushNotification("error", `Failed to install ${request.catalog_id}: ${e}`);
      return null;
    }
  }

  async function uninstallEntry(serverId: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("uninstall_catalog_entry", { serverId });
      delete installState.value[serverId];
      await fetchInstalled();
    } catch (e) {
      console.error("Failed to uninstall catalog entry:", e);
      ui.pushNotification("error", `Failed to uninstall ${serverId}: ${e}`);
    }
  }

  async function refreshCatalogSource(source: string | null = null): Promise<void> {
    const ui = useUiStore();
    if (source) {
      delete sourceFailures.value[source];
    } else {
      sourceFailures.value = {};
    }
    try {
      await invoke("refresh_catalog", { source });
      await fetchCatalog();
    } catch (e) {
      console.error("Failed to refresh catalog source:", e);
      ui.pushNotification("error", `Failed to refresh catalog: ${e}`);
    }
  }

  async function fetchSources(): Promise<void> {
    const ui = useUiStore();
    try {
      sources.value = await invoke<CatalogSourceViewResponse[]>("list_catalog_sources");
    } catch (e) {
      error.value = String(e);
      ui.pushNotification("error", `Failed to load catalog sources: ${e}`);
    }
  }

  async function addSource(request: AddCatalogSourceRequestPayload): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("add_catalog_source", { request });
      await fetchSources();
    } catch (e) {
      console.error("Failed to add catalog source:", e);
      ui.pushNotification("error", `Failed to add source ${request.id}: ${e}`);
    }
  }

  async function removeSource(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("remove_catalog_source", { id });
      delete sourceFailures.value[id];
      await fetchSources();
    } catch (e) {
      console.error("Failed to remove catalog source:", e);
      ui.pushNotification("error", `Failed to remove source ${id}: ${e}`);
    }
  }

  async function setSourceEnabled(id: string, enabled: boolean): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("set_catalog_source_enabled", { id, enabled });
      await fetchSources();
    } catch (e) {
      console.error("Failed to toggle catalog source:", e);
      ui.pushNotification("error", `Failed to toggle source ${id}: ${e}`);
    }
  }

  function handleSourceFailed(source: string, errorMsg: string): void {
    sourceFailures.value[source] = errorMsg;
  }

  /** Merge incremental results from one source into the combined list.
   *  Called as each catalog source completes, so the UI updates without
   *  waiting for the slowest source. */
  function mergeSourceResults(source: string, incoming: ServerEntryResponse[]): void {
    delete sourceFailures.value[source];
    // Build a lookup of existing (source, id) keys.
    const seen = new Map<string, ServerEntryResponse>();
    for (const e of entries.value) {
      seen.set(`${e.source}:${e.id}`, e);
    }
    for (const e of incoming) {
      seen.set(`${e.source}:${e.id}`, e);
    }
    // Re-sort to match backend ordering (trust desc, source asc, name asc).
    const TRUST_ORDER: Record<string, number> = {
      unverified: 0,
      community: 1,
      verified: 2
    };
    entries.value = Array.from(seen.values()).sort((a, b) => {
      const ta = TRUST_ORDER[a.trust] ?? 0;
      const tb = TRUST_ORDER[b.trust] ?? 0;
      if (tb !== ta) return tb - ta;
      if (a.source !== b.source) return a.source.localeCompare(b.source);
      return a.display_name.localeCompare(b.display_name);
    });
  }

  return {
    // state
    entries,
    installed,
    installedServerNames,
    installState,
    loading,
    error,
    tab,
    filters,
    sources,
    sourceFailures,
    currentInstallEntryId,
    // computeds
    filteredEntries,
    hasEntries,
    installedCount,
    allSourceIds,
    visibleEntries,
    // helpers
    reset,
    isSourceEnabled,
    toggleSource,
    handleSourceFailed,
    requestInstallProgress,
    dismissInstallProgress,
    checkInstalledStatus,
    isServerInstalled,
    // incremental merge (called by useTauriEvents on CatalogSourceResultsArrived)
    mergeSourceResults,
    // actions
    fetchCatalog,
    fetchInstalled,
    getCatalogEntry,
    installEntry,
    uninstallEntry,
    refreshCatalogSource,
    fetchSources,
    addSource,
    removeSource,
    setSourceEnabled
  };
});
