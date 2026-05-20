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
          <th>{{ t("marketplace.installedList.server") }}</th>
          <th>{{ t("marketplace.installedList.source") }}</th>
          <th>{{ t("marketplace.installedList.status") }}</th>
          <th>{{ t("marketplace.installedList.installedAt") }}</th>
          <th />
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in catalog.installed" :key="row.server_id">
          <td>
            <span class="text-strong">{{ row.display_name }}</span>
          </td>
          <td>
            <span class="text-secondary">{{
              row.source ?? t("marketplace.installedList.manualSource")
            }}</span>
          </td>
          <td>
            <KxBadge :tone="row.running ? 'success' : 'neutral'">
              {{
                row.running
                  ? t("marketplace.installedList.running")
                  : t("marketplace.installedList.stopped")
              }}
            </KxBadge>
          </td>
          <td>
            <span class="text-tertiary">{{ row.installed_at }}</span>
          </td>
          <td>
            <!-- The disabled button still needs to appear in the DOM for the
                 existing test (which checks the disabled attribute on a
                 hand-edited row). -->
            <KxButton
              size="xs"
              :disabled="!row.source"
              :title="row.source ? '' : t('marketplace.installedList.manualSourceNotRemovable')"
              :data-test="`uninstall-${row.server_id}`"
              @click="onUninstall(row.server_id)"
            >
              {{ t("marketplace.installedList.uninstall") }}
            </KxButton>
          </td>
        </tr>
      </tbody>
    </table>
    <KxEmptyState
      v-if="catalog.installed.length === 0"
      class="installed-empty"
      data-test="installed-empty-state"
    >
      {{ t("marketplace.installedEmpty") }}
    </KxEmptyState>
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
.installed-empty {
  margin-top: 24px;
}
</style>
