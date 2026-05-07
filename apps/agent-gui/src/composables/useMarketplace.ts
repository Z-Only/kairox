import { onMounted } from "vue";
import {
  catalogState,
  filteredEntries,
  fetchCatalog,
  fetchInstalled,
  installEntry,
  uninstallEntry,
  refreshCatalogSource
} from "../stores/catalog";
import type { ServerEntryResponse } from "../generated/commands";

export interface RuntimeRequirementParsed {
  kind: string;
  min_version: string | null;
  install_hint: string | null;
}

export interface EnvVarSpecParsed {
  key: string;
  label: string;
  description: string;
  required: boolean;
  secret: boolean;
  default: string | null;
}

export function parseRequirements(entry: ServerEntryResponse): RuntimeRequirementParsed[] {
  try {
    const v = JSON.parse(entry.requirements_json);
    return Array.isArray(v) ? (v as RuntimeRequirementParsed[]) : [];
  } catch {
    return [];
  }
}

export function parseDefaultEnv(entry: ServerEntryResponse): EnvVarSpecParsed[] {
  try {
    const v = JSON.parse(entry.default_env_json);
    return Array.isArray(v) ? (v as EnvVarSpecParsed[]) : [];
  } catch {
    return [];
  }
}

export function useMarketplace() {
  onMounted(async () => {
    if (catalogState.entries.length === 0) await fetchCatalog();
    if (catalogState.installed.length === 0) await fetchInstalled();
  });

  return {
    state: catalogState,
    filteredEntries,
    fetchCatalog,
    fetchInstalled,
    installEntry,
    uninstallEntry,
    refreshCatalogSource,
    parseRequirements,
    parseDefaultEnv
  };
}
