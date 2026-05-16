// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Plain `.ts` composables must import the
// lifecycle + pinia helpers they use.
import { onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useCatalogStore } from "@/stores/catalog";
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

/** Extract headers from install_spec_json (Sse/StreamableHttp variants). */
export function parseInstallHeaders(entry: ServerEntryResponse): EnvVarSpecParsed[] {
  try {
    const spec = JSON.parse(entry.install_spec_json);
    if (spec.transport !== "sse" && spec.transport !== "streamable_http") return [];
    const headers = spec.headers as Record<string, string> | undefined;
    if (!headers) return [];
    return Object.entries(headers).map(([key]) => {
      // Match against default_env to get description/required/secret metadata.
      const envMeta = parseDefaultEnv(entry).find((e) => e.key === key);
      return {
        key,
        label: key,
        description: envMeta?.description ?? "",
        required: envMeta?.required ?? false,
        secret: envMeta?.secret ?? false,
        default: ""
      };
    });
  } catch {
    return [];
  }
}

export function useMarketplace() {
  const catalog = useCatalogStore();
  const { filteredEntries } = storeToRefs(catalog);

  onMounted(async () => {
    if (catalog.entries.length === 0) await catalog.fetchCatalog();
    if (catalog.installed.length === 0) await catalog.fetchInstalled();
  });

  return {
    state: catalog,
    filteredEntries,
    fetchCatalog: catalog.fetchCatalog,
    fetchInstalled: catalog.fetchInstalled,
    installEntry: catalog.installEntry,
    uninstallEntry: catalog.uninstallEntry,
    refreshCatalogSource: catalog.refreshCatalogSource,
    parseRequirements,
    parseDefaultEnv
  };
}
