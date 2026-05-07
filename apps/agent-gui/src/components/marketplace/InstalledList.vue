<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useCatalogStore } from "@/stores/catalog";

const { t } = useI18n();
const catalog = useCatalogStore();

onMounted(() => catalog.fetchInstalled());

async function onUninstall(serverId: string) {
  await catalog.uninstallEntry(serverId);
}
</script>

<template>
  <!-- A semantic <table> is retained because the existing test suite asserts
       layout via row/cell text. Per-cell controls use native HTML elements
       with theme-aware CSS variable colours. -->
  <div class="installed-wrap" data-test="installed-list">
    <table class="installed">
      <thead>
        <tr>
          <th>Server</th>
          <th>Source</th>
          <th>Status</th>
          <th>Installed at</th>
          <th />
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in catalog.installed" :key="row.server_id">
          <td>
            <span class="text-strong">{{ row.display_name }}</span>
          </td>
          <td>
            <span class="text-secondary">{{ row.source ?? "(manual)" }}</span>
          </td>
          <td>
            <span :class="['tag', 'tag-sm', row.running ? 'tag-success' : 'tag-default']">
              {{ row.running ? "running" : "stopped" }}
            </span>
          </td>
          <td>
            <span class="text-tertiary">{{ row.installed_at }}</span>
          </td>
          <td>
            <!-- The disabled button still needs to appear in the DOM for the
                 existing test (which checks the disabled attribute on a
                 hand-edited row). -->
            <button
              class="btn btn-xs"
              :disabled="!row.source"
              :title="row.source ? '' : 'Hand-edited entries are not removable from here'"
              :data-test="`uninstall-${row.server_id}`"
              @click="onUninstall(row.server_id)"
            >
              Uninstall
            </button>
          </td>
        </tr>
      </tbody>
    </table>
    <div v-if="catalog.installed.length === 0" class="empty-state empty">
      {{ t("marketplace.installedEmpty") }}
    </div>
  </div>
</template>

<style scoped>
.installed {
  width: 100%;
  border-collapse: collapse;
}
.installed th,
.installed td {
  text-align: left;
  padding: 6px 8px;
  border-bottom: 1px solid var(--app-border-color);
}
.text-strong {
  font-weight: 600;
  color: var(--app-text-color);
}
.text-secondary {
  color: var(--app-text-color-2);
}
.text-tertiary {
  color: var(--app-text-color-3);
}
.tag {
  display: inline-flex;
  align-items: center;
  padding: 0 6px;
  height: 22px;
  font-size: 12px;
  border-radius: 3px;
  line-height: 1;
}
.tag-default {
  background: var(--app-hover-color);
  color: var(--app-text-color);
}
.tag-success {
  background: var(--app-success-bg);
  color: var(--app-success-color);
}
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}
.btn:hover:not(:disabled) {
  background: var(--app-hover-color);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-xs {
  height: 24px;
}
.empty-state {
  text-align: center;
  color: var(--app-text-color-3);
  padding: 24px 0;
}
.empty {
  margin-top: 24px;
}
</style>
