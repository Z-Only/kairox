<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type {
  ServerEntryResponse,
  InstallRequestPayload,
  InstalledEntryResponse,
  ConfigScope
} from "../../generated/commands";
import { commands } from "../../generated/commands";
import { useCatalogStore } from "@/stores/catalog";
import { useMcpStore } from "@/stores/mcp";
import {
  parseRequirements,
  parseDefaultEnv,
  parseInstallHeaders
} from "../../composables/useMarketplace";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";
import ScopeSelector from "@/components/ScopeSelector.vue";

const { t } = useI18n();
const catalog = useCatalogStore();
const mcp = useMcpStore();
const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ close: [] }>();

// Disable Install when *another* entry's install is currently in flight.
const installDisabled = computed(
  () => catalog.currentInstallEntryId !== null && catalog.currentInstallEntryId !== props.entry.id
);

// ── installed-state detection ──────────────────────────────────────
// Match catalog entry → installed entry via catalog_id.
const installedEntry = computed<InstalledEntryResponse | null>(
  () => catalog.installed.find((e) => e.catalog_id === props.entry.id) ?? null
);

const isInstalled = computed(() => installedEntry.value !== null);

// Resolve the scope at which this server is installed. We cross-reference
// the installed entry's server_id against the effective MCP server list
// (which carries ConfigScope metadata per item).
const installScope = computed<ConfigScope | null>(() => {
  if (!installedEntry.value) return null;
  const effective = mcp.effectiveServers.find(
    (es) =>
      es.value.id === installedEntry.value!.server_id ||
      es.value.name === installedEntry.value!.server_id
  );
  return (effective?.source as ConfigScope) ?? null;
});

// Derive a short human-readable label for the scope badge.
const scopeLabel = computed(() => {
  if (!installScope.value) return "";
  switch (installScope.value) {
    case "Builtin":
      return t("marketplace.scopeBuiltin");
    case "User":
      return t("marketplace.scopeUser");
    case "Project":
      return t("marketplace.scopeProject");
    case "Local":
      return t("marketplace.scopeLocal");
    default:
      return installScope.value;
  }
});

const requirements = computed(() => parseRequirements(props.entry));
const envSpec = computed(() => parseDefaultEnv(props.entry));
const headerSpec = computed(() => parseInstallHeaders(props.entry));
const headerKeys = computed(() => new Set(headerSpec.value.map((h) => h.key)));

// Env vars excluding those that serve as header values.
const nonHeaderEnvSpec = computed(() => envSpec.value.filter((s) => !headerKeys.value.has(s.key)));
const configItems = computed(() => [
  ...headerSpec.value.map((spec) => ({
    ...spec,
    kind: spec.key.toLowerCase() === "authorization" ? "Authentication header" : "HTTP header"
  })),
  ...nonHeaderEnvSpec.value.map((spec) => ({
    ...spec,
    kind: "Environment variable"
  }))
]);
const requiredConfigCount = computed(
  () => configItems.value.filter((spec) => spec.required).length
);
const hasConfig = computed(() => configItems.value.length > 0);

const overrides = ref<Record<string, string>>({});
// Trust grant must be opt-in: catalog "verified" means the *distribution
// channel* is trusted, not that runtime tool calls should bypass the
// PermissionCenter. Default to false and let the user opt in explicitly.
const trustGrant = ref(false);
const autoStart = ref(true);
const installTarget = ref<ConfigScope>("User");

const testingCatalogConnectivity = ref(false);
const catalogConnectivityResult = ref<
  { status: "connected"; tool_count: number } | { status: "failed"; reason: string } | null
>(null);

// Re-initialise local form state whenever the selected entry changes.
watch(
  () => props.entry.id,
  () => {
    const next: Record<string, string> = {};
    for (const spec of envSpec.value) {
      next[spec.key] = spec.default ?? "";
    }
    overrides.value = next;
    trustGrant.value = false;
    autoStart.value = true;
    catalogConnectivityResult.value = null;
  },
  { immediate: true }
);

function testConnectivityLabel(): string {
  if (testingCatalogConnectivity.value) return t("mcp.testChecking");
  const result = catalogConnectivityResult.value;
  if (!result) return t("mcp.testConnectivity");
  if (result.status === "connected") {
    return t("mcp.testConnected", { count: result.tool_count });
  }
  return t("mcp.testFailed", { reason: result.reason });
}

function configPlaceholder(spec: (typeof configItems.value)[number]): string {
  if (spec.default) return spec.default;
  if (spec.kind === "Authentication header") return "Bearer <token>";
  return "";
}

async function testCatalogConnectivity(): Promise<void> {
  if (!installedEntry.value) return;
  const serverId = installedEntry.value.server_id;
  testingCatalogConnectivity.value = true;
  try {
    const result = await commands.testMcpConnectivity(serverId);
    if (result.status === "ok") {
      catalogConnectivityResult.value = result.data;
    } else {
      catalogConnectivityResult.value = { status: "failed", reason: String(result.error) };
    }
  } catch (e) {
    catalogConnectivityResult.value = { status: "failed", reason: String(e) };
  } finally {
    testingCatalogConnectivity.value = false;
  }
}

async function onInstall() {
  const req: InstallRequestPayload = {
    catalog_id: props.entry.id,
    source: props.entry.source,
    server_id_override: null,
    env_overrides: overrides.value,
    trust_grant: trustGrant.value,
    auto_start: autoStart.value
  };
  catalog.requestInstallProgress(props.entry.id);
  await catalog.installEntry(req);
}

function onOverlayClick(event: MouseEvent) {
  if (event.target === event.currentTarget) emit("close");
}
</script>

<template>
  <!-- Drawer slide-out panel replaces NDrawer. data-test="catalog-detail"
       stays on the inner container for existing e2e selectors. -->
  <Teleport to="body">
    <div class="drawer-overlay" @click="onOverlayClick">
      <aside class="drawer">
        <header class="drawer-header">
          <span class="drawer-title">{{ entry.display_name }}</span>
          <button class="btn drawer-close-btn" aria-label="Close" @click="emit('close')">✕</button>
        </header>

        <div class="drawer-body" data-test="catalog-detail">
          <div class="catalog-detail">
            <span class="text-secondary">{{ entry.description }}</span>
            <a
              v-if="entry.homepage"
              :href="entry.homepage"
              target="_blank"
              rel="noopener"
              class="homepage-link"
            >
              Homepage
            </a>

            <div class="card card-sm">
              <div class="card-title">Requirements</div>
              <RuntimeMissingHint :requirements="requirements" />
            </div>

            <div class="card card-sm">
              <div class="config-head">
                <div>
                  <div class="card-title">Configuration</div>
                  <span v-if="hasConfig" class="config-summary text-tertiary">
                    {{
                      requiredConfigCount > 0
                        ? `${requiredConfigCount} required value${requiredConfigCount === 1 ? "" : "s"} before install.`
                        : "Optional values can be provided before install."
                    }}
                  </span>
                  <span v-else class="config-summary text-tertiary">
                    No configuration required.
                  </span>
                </div>
                <span v-if="requiredConfigCount > 0" class="config-status required">
                  Required configuration
                </span>
              </div>

              <div v-if="hasConfig" class="config-list">
                <div
                  v-for="spec in configItems"
                  :key="`${spec.kind}:${spec.key}`"
                  class="config-item"
                >
                  <div class="config-item-head">
                    <div class="config-title-row">
                      <span class="config-label">{{ spec.label }}</span>
                      <span class="config-key">{{ spec.key }}</span>
                    </div>
                    <div class="config-badges">
                      <span class="config-kind">{{ spec.kind }}</span>
                      <span class="config-required" :class="{ optional: !spec.required }">
                        {{ spec.required ? "Required" : "Optional" }}
                      </span>
                    </div>
                  </div>
                  <span v-if="spec.description" class="config-description text-secondary">
                    {{ spec.description }}
                  </span>
                  <span v-else class="config-description text-tertiary">
                    No description provided by the catalog.
                  </span>
                  <input
                    :value="overrides[spec.key]"
                    :type="spec.secret ? 'password' : 'text'"
                    :placeholder="configPlaceholder(spec)"
                    class="input input-sm"
                    :data-test="`config-${spec.key}`"
                    @input="overrides[spec.key] = ($event.target as HTMLInputElement).value"
                  />
                </div>
              </div>
            </div>

            <div class="card card-sm">
              <div class="card-title">Options</div>
              <div class="options-group">
                <label class="checkbox-label">
                  <input v-model="trustGrant" type="checkbox" />
                  Trust this server (skip per-tool permission prompts)
                </label>
                <span v-if="entry.trust === 'verified'" class="hint-verified text-tertiary">
                  This entry comes from a verified source. You can grant runtime trust to skip
                  permission prompts, but it remains opt-in.
                </span>
                <label class="checkbox-label">
                  <input v-model="autoStart" type="checkbox" />
                  Start after install
                </label>
              </div>
            </div>
          </div>
        </div>

        <footer class="drawer-footer">
          <button
            class="btn btn-primary btn-sm"
            data-test="catalog-install"
            :disabled="installDisabled"
            @click="onInstall"
          >
            {{ t("marketplace.install.buttonInstall") }}
          </button>
          <button class="btn btn-sm" @click="emit('close')">{{ t("common.close") }}</button>
        </footer>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.drawer-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(0, 0, 0, 0.3);
}
.drawer {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: 480px;
  max-width: 90vw;
  display: flex;
  flex-direction: column;
  background: var(--app-body-color);
  box-shadow: var(--app-shadow-2);
}
.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--app-border-color);
}
.drawer-title {
  font-weight: 600;
  font-size: 16px;
  color: var(--app-text-color);
}
.drawer-close-btn {
  font-size: 16px;
  padding: 2px 6px;
  line-height: 1;
}
.drawer-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}
.drawer-footer {
  display: flex;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--app-border-color);
}

.catalog-detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.text-secondary {
  color: var(--app-text-color-2);
}
.text-tertiary {
  color: var(--app-text-color-3);
}
.homepage-link {
  font-size: 13px;
}
.card {
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 12px;
  background: var(--app-card-color);
}
.card-sm {
  padding: 10px 12px;
}
.card-title {
  font-weight: 600;
  font-size: 13px;
  color: var(--app-text-color);
  margin-bottom: 8px;
}
.config-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
  margin-bottom: 10px;
}
.config-head .card-title {
  margin-bottom: 2px;
}
.config-summary {
  display: block;
  font-size: 12px;
  line-height: 1.35;
}
.config-status,
.config-kind,
.config-required,
.config-key {
  display: inline-flex;
  align-items: center;
  min-height: 20px;
  padding: 0 6px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  font-size: 11px;
  line-height: 1;
  white-space: nowrap;
}
.config-status.required {
  border-color: color-mix(in srgb, var(--app-warning-color, #d97706) 45%, var(--app-border-color));
  color: var(--app-warning-color, #b45309);
  background: color-mix(in srgb, var(--app-warning-color, #d97706) 10%, transparent);
}
.config-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.config-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 10px;
  border-top: 1px solid var(--app-border-color);
}
.config-item:first-child {
  padding-top: 0;
  border-top: none;
}
.config-item-head {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  align-items: flex-start;
}
.config-title-row {
  min-width: 0;
  display: flex;
  gap: 6px;
  align-items: center;
  flex-wrap: wrap;
}
.config-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--app-text-color);
}
.config-key {
  color: var(--app-text-color-3);
  font-family: var(--app-mono-font, ui-monospace, SFMono-Regular, Menlo, monospace);
}
.config-badges {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 4px;
}
.config-kind {
  color: var(--app-text-color-2);
  background: var(--app-bg-color);
}
.config-required {
  border-color: color-mix(in srgb, var(--app-error-color, #dc2626) 40%, var(--app-border-color));
  color: var(--app-error-color, #dc2626);
  background: color-mix(in srgb, var(--app-error-color, #dc2626) 8%, transparent);
}
.config-required.optional {
  border-color: var(--app-border-color);
  color: var(--app-text-color-3);
  background: transparent;
}
.config-description {
  display: block;
  font-size: 12px;
  line-height: 1.4;
}
.env-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.env-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.env-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-color);
}
.env-help {
  display: block;
  margin-top: 4px;
  font-size: 11px;
}
.input {
  width: 100%;
  padding: 4px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-bg-color);
  color: var(--app-text-color);
  font-size: 13px;
}
.input:focus {
  outline: 2px solid var(--app-primary-color);
  outline-offset: -1px;
}
.input-sm {
  height: 28px;
}
.options-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.checkbox-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--app-text-color);
  cursor: pointer;
}
.hint-verified {
  font-size: 12px;
}
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
}
.btn:hover {
  background: var(--app-hover-color);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-primary {
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color);
  border-color: var(--app-primary-color);
}
.btn-primary:hover:not(:disabled) {
  filter: brightness(1.1);
}
.btn-sm {
  height: 28px;
  font-size: 13px;
}
.tooltip-wrap {
  position: relative;
}
.tooltip-active::after {
  content: attr(data-tooltip);
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  padding: 4px 8px;
  border-radius: 4px;
  background: var(--app-popover-color);
  color: var(--app-text-color);
  font-size: 12px;
  white-space: nowrap;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.15s;
  box-shadow: var(--app-shadow-1);
}
.tooltip-active:hover::after {
  opacity: 1;
}
.scope-badge {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 8px;
  border-radius: 4px;
  background: var(--app-tag-color, color-mix(in srgb, var(--app-primary-color) 10%, transparent));
  color: var(--app-text-color-2);
  font-size: 12px;
  white-space: nowrap;
}
</style>
