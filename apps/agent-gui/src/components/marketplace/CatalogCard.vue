<script setup lang="ts">
import type { ServerEntryResponse, InstallRequestPayload } from "../../generated/commands";
import { useCatalogStore } from "@/stores/catalog";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const catalog = useCatalogStore();

const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ click: [] }>();

const trustTagClass = computed<string>(() => {
  if (props.entry.trust === "verified") return "tag-success";
  if (props.entry.trust === "community") return "tag-warning";
  return "";
});

const isInstalled = computed(() => catalog.installed.some((e) => e.catalog_id === props.entry.id));

const installDisabled = computed(
  () => catalog.currentInstallEntryId !== null && catalog.currentInstallEntryId !== props.entry.id
);

async function onInstall() {
  const req: InstallRequestPayload = {
    catalog_id: props.entry.id,
    source: props.entry.source,
    server_id_override: null,
    env_overrides: {},
    trust_grant: false,
    auto_start: true
  };
  catalog.requestInstallProgress(props.entry.id);
  await catalog.installEntry(req);
}
</script>

<template>
  <div class="card catalog-card">
    <button type="button" class="card-body-btn" data-test="catalog-card" @click="emit('click')">
      <div class="card-head">
        <span class="icon">{{ entry.icon || "🔌" }}</span>
        <span class="display-name">{{ entry.display_name }}</span>
        <span class="tag trust-tag" :class="trustTagClass">
          {{ entry.trust }}
        </span>
      </div>
      <span class="summary">{{ entry.summary }}</span>
      <div class="tags">
        <span v-for="t in entry.tags" :key="t" class="tag">
          {{ t }}
        </span>
      </div>
    </button>
    <div class="card-footer">
      <span v-if="isInstalled" class="installed-badge">
        {{ t("marketplace.install.installed") }}
      </span>
      <KxButton
        v-else
        variant="primary"
        size="xs"
        data-test="catalog-card-install"
        :disabled="installDisabled"
        @click.stop="onInstall"
      >
        {{ t("marketplace.install.buttonInstall") }}
      </KxButton>
    </div>
  </div>
</template>

<style scoped>
.card {
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  display: flex;
  flex-direction: column;
}

.card-body-btn {
  all: unset;
  display: block;
  box-sizing: border-box;
  width: 100%;
  cursor: pointer;
  text-align: left;
  padding: 12px;
  flex: 1;
}

.card-body-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  border-radius: 6px;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.display-name {
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon {
  flex-shrink: 0;
}

.trust-tag {
  flex-shrink: 0;
  margin-left: auto;
}

.summary {
  font-size: 13px;
  display: block;
  color: var(--app-text-color-2);
  margin-top: 4px;
}

.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-top: 6px;
}

.card-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 8px 12px;
  border-top: 1px solid var(--app-border-color);
}

.installed-badge {
  font-size: 12px;
  color: var(--app-text-color-3);
}
</style>
