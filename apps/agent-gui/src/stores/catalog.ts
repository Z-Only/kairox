import { reactive, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  ServerEntryResponse,
  InstalledEntryResponse,
  InstallOutcomeResponse,
  InstallRequestPayload,
  CatalogQueryRequest
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
  }
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
