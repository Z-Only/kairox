import { reactive, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  ServerEntryResponse,
  InstalledEntryResponse,
  InstallOutcomeResponse,
  InstallRequestPayload,
  CatalogQueryRequest,
  CatalogSourceViewResponse,
  AddCatalogSourceRequestPayload
} from "../generated/commands";
import { addNotification } from "../composables/useNotifications";

export type CatalogTab = "browse" | "installed";

export type TrustLevel = "unverified" | "community" | "verified";

export interface CatalogFilters {
  keyword: string;
  category: string | null;
  trustMin: TrustLevel | null;
}

export interface CatalogState {
  entries: ServerEntryResponse[];
  installed: InstalledEntryResponse[];
  installState: Record<string, InstallOutcomeResponse>;
  loading: boolean;
  error: string | null;
  tab: CatalogTab;
  filters: CatalogFilters;
  // Phase 2: catalog sources
  sources: CatalogSourceViewResponse[];
  sourceFailures: Record<string, string>;
  /** Source ids currently selected via chip filter. null = all selected. */
  selectedSources: string[] | null;
}

const initial = (): CatalogState => ({
  entries: [],
  installed: [],
  installState: {},
  loading: false,
  error: null,
  tab: "browse",
  filters: {
    keyword: "",
    category: null,
    trustMin: null
  },
  sources: [],
  sourceFailures: {},
  selectedSources: null
});

export const catalogState = reactive<CatalogState>(initial());

/** Reset all catalog state. Used in tests. */
export function resetCatalogState(): void {
  Object.assign(catalogState, initial());
}

const TRUST_ORDER: Record<TrustLevel, number> = {
  unverified: 0,
  community: 1,
  verified: 2
};

export const filteredEntries = computed<ServerEntryResponse[]>(() => {
  const kw = catalogState.filters.keyword.trim().toLowerCase();
  const minOrder = catalogState.filters.trustMin
    ? TRUST_ORDER[catalogState.filters.trustMin]
    : -1;
  return catalogState.entries.filter((e) => {
    if (kw) {
      const hay =
        `${e.display_name} ${e.summary} ${e.tags.join(" ")}`.toLowerCase();
      if (!hay.includes(kw)) return false;
    }
    if (
      catalogState.filters.category &&
      !e.categories.includes(catalogState.filters.category)
    ) {
      return false;
    }
    if (catalogState.filters.trustMin) {
      const t = TRUST_ORDER[e.trust as TrustLevel] ?? 0;
      if (t < minOrder) return false;
    }
    return true;
  });
});

export const hasEntries = computed(() => catalogState.entries.length > 0);

export const installedCount = computed(() => catalogState.installed.length);

export async function fetchCatalog(
  query: CatalogQueryRequest = {}
): Promise<void> {
  catalogState.loading = true;
  catalogState.error = null;
  try {
    catalogState.entries = await invoke<ServerEntryResponse[]>("list_catalog", {
      query
    });
  } catch (e) {
    catalogState.error = String(e);
    addNotification("error", `Failed to load catalog: ${e}`);
  } finally {
    catalogState.loading = false;
  }
}

export async function fetchInstalled(): Promise<void> {
  try {
    catalogState.installed = await invoke<InstalledEntryResponse[]>(
      "list_installed_entries"
    );
  } catch (e) {
    catalogState.error = String(e);
    addNotification("error", `Failed to load installed entries: ${e}`);
  }
}

export async function getCatalogEntry(
  id: string,
  source?: string | null
): Promise<ServerEntryResponse | null> {
  try {
    return await invoke<ServerEntryResponse | null>("get_catalog_entry", {
      id,
      source: source ?? null
    });
  } catch (e) {
    console.error("Failed to get catalog entry:", e);
    addNotification("error", `Failed to load catalog entry ${id}: ${e}`);
    return null;
  }
}

export async function installEntry(
  request: InstallRequestPayload
): Promise<InstallOutcomeResponse | null> {
  try {
    const outcome = await invoke<InstallOutcomeResponse>(
      "install_catalog_entry",
      { request }
    );
    catalogState.installState[request.catalog_id] = outcome;
    if (outcome.kind === "installed") {
      await fetchInstalled();
    }
    return outcome;
  } catch (e) {
    console.error("Failed to install catalog entry:", e);
    addNotification("error", `Failed to install ${request.catalog_id}: ${e}`);
    return null;
  }
}

export async function uninstallEntry(serverId: string): Promise<void> {
  try {
    await invoke("uninstall_catalog_entry", { serverId });
    delete catalogState.installState[serverId];
    await fetchInstalled();
  } catch (e) {
    console.error("Failed to uninstall catalog entry:", e);
    addNotification("error", `Failed to uninstall ${serverId}: ${e}`);
  }
}

export async function refreshCatalogSource(
  source: string | null = null
): Promise<void> {
  try {
    await invoke("refresh_catalog", { source });
    await fetchCatalog();
  } catch (e) {
    console.error("Failed to refresh catalog source:", e);
    addNotification("error", `Failed to refresh catalog: ${e}`);
  }
}

// ---------------------------------------------------------------------------
// Phase 2: catalog source CRUD + failure tracking
// ---------------------------------------------------------------------------

export async function fetchSources(): Promise<void> {
  try {
    catalogState.sources = await invoke<CatalogSourceViewResponse[]>(
      "list_catalog_sources"
    );
  } catch (e) {
    catalogState.error = String(e);
    addNotification("error", `Failed to load catalog sources: ${e}`);
  }
}

export async function addSource(
  request: AddCatalogSourceRequestPayload
): Promise<void> {
  try {
    await invoke("add_catalog_source", { request });
    await fetchSources();
  } catch (e) {
    console.error("Failed to add catalog source:", e);
    addNotification("error", `Failed to add source ${request.id}: ${e}`);
  }
}

export async function removeSource(id: string): Promise<void> {
  try {
    await invoke("remove_catalog_source", { id });
    delete catalogState.sourceFailures[id];
    await fetchSources();
  } catch (e) {
    console.error("Failed to remove catalog source:", e);
    addNotification("error", `Failed to remove source ${id}: ${e}`);
  }
}

export async function setSourceEnabled(
  id: string,
  enabled: boolean
): Promise<void> {
  try {
    await invoke("set_catalog_source_enabled", { id, enabled });
    await fetchSources();
  } catch (e) {
    console.error("Failed to toggle catalog source:", e);
    addNotification("error", `Failed to toggle source ${id}: ${e}`);
  }
}

/** Record a CatalogSourceFailed event payload onto sourceFailures[source]. */
export function handleSourceFailed(source: string, error: string): void {
  catalogState.sourceFailures[source] = error;
}

// ---------------------------------------------------------------------------
// Phase 2: source chip filter
// ---------------------------------------------------------------------------

/** All available source ids (builtin + remote). */
export const allSourceIds = computed<string[]>(() => [
  "builtin",
  ...catalogState.sources.map((s) => s.id)
]);

/** Returns true when the given source id is currently active (or all are). */
export function isSourceSelected(id: string): boolean {
  if (catalogState.selectedSources === null) return true;
  return catalogState.selectedSources.includes(id);
}

/** Toggle a source on/off in the chip filter. */
export function toggleSource(id: string): void {
  // Materialise the "all selected" sentinel into an explicit array on first toggle.
  const current = catalogState.selectedSources ?? allSourceIds.value.slice();
  const next = current.includes(id)
    ? current.filter((x) => x !== id)
    : [...current, id];
  catalogState.selectedSources = next;
}

/**
 * Entries filtered by both client-side filters AND chip selection.
 * Use this in marketplace components instead of `filteredEntries`.
 */
export const visibleEntries = computed<ServerEntryResponse[]>(() =>
  filteredEntries.value.filter((e) => isSourceSelected(e.source))
);
