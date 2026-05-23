import { computed, ref } from "vue";
import type {
  CheckMcpHealthResponse,
  ConnectivityResult,
  EffectiveMcpServerView,
  McpContentBlockResponse,
  McpPromptDefResponse,
  McpResourceDefResponse,
  McpServerSettingsView
} from "@/generated/commands";
import type { McpServerEntry } from "./types";

export function createMcpState() {
  const servers = ref<McpServerEntry[]>([]);
  const trustedServerIds = ref<string[]>([]);
  const loading = ref(false);
  const settingsServers = ref<McpServerSettingsView[]>([]);
  const settingsLoading = ref(false);
  const configFileOpening = ref(false);
  const settingsError = ref<string | null>(null);
  const effectiveServers = ref<EffectiveMcpServerView[]>([]);
  const connectivityResults = ref<Record<string, ConnectivityResult>>({});
  const testingConnectivity = ref<Set<string>>(new Set());

  // Health check + tool management (P5)
  const serverHealth = ref<Record<string, CheckMcpHealthResponse>>({});
  const checkingHealth = ref<Set<string>>(new Set());
  const expandedServers = ref<Set<string>>(new Set());
  const disabledTools = ref<Record<string, Set<string>>>({});

  // Resource & prompt browsing
  const serverResources = ref<Record<string, McpResourceDefResponse[]>>({});
  const serverPrompts = ref<Record<string, McpPromptDefResponse[]>>({});
  const loadingResources = ref<Set<string>>(new Set());
  const loadingPrompts = ref<Set<string>>(new Set());
  const expandedResourceUri = ref<Record<string, string | null>>({});
  const resourcesError = ref<Record<string, string | null>>({});
  const promptsError = ref<Record<string, string | null>>({});
  const resourceContentCache = ref<Record<string, McpContentBlockResponse[]>>({});

  const runningServers = computed(() => servers.value.filter((s) => s.status === "running"));
  const failedServers = computed(() => servers.value.filter((s) => s.status === "failed"));
  const runningCount = computed(() => runningServers.value.length);
  const hasServers = computed(() => servers.value.length > 0);

  return {
    servers,
    trustedServerIds,
    loading,
    settingsServers,
    settingsLoading,
    configFileOpening,
    settingsError,
    effectiveServers,
    connectivityResults,
    testingConnectivity,
    serverHealth,
    checkingHealth,
    expandedServers,
    disabledTools,
    serverResources,
    serverPrompts,
    loadingResources,
    loadingPrompts,
    expandedResourceUri,
    resourcesError,
    promptsError,
    resourceContentCache,
    runningServers,
    failedServers,
    runningCount,
    hasServers
  };
}

export type McpState = ReturnType<typeof createMcpState>;
