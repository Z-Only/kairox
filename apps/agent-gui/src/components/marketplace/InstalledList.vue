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
       layout via row/cell text. We swap the per-cell controls to NaiveUI
       primitives so coloring + button affordance follow the active theme. -->
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
            <NText strong>{{ row.display_name }}</NText>
          </td>
          <td>
            <NText depth="2">{{ row.source ?? "(manual)" }}</NText>
          </td>
          <td>
            <NTag
              size="small"
              :type="row.running ? 'success' : 'default'"
              :bordered="false"
            >
              {{ row.running ? "running" : "stopped" }}
            </NTag>
          </td>
          <td>
            <NText depth="3">{{ row.installed_at }}</NText>
          </td>
          <td>
            <!-- The disabled button still needs to appear in the DOM for the
                 existing test (which checks the disabled attribute on a
                 hand-edited row). NButton renders a real <button>, so the
                 attribute round-trips. -->
            <NButton
              size="tiny"
              :disabled="!row.source"
              :title="
                row.source
                  ? ''
                  : 'Hand-edited entries are not removable from here'
              "
              :data-test="`uninstall-${row.server_id}`"
              @click="onUninstall(row.server_id)"
            >
              Uninstall
            </NButton>
          </td>
        </tr>
      </tbody>
    </table>
    <NEmpty
      v-if="catalog.installed.length === 0"
      :description="t('marketplace.installedEmpty')"
      class="empty"
    />
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
  border-bottom: 1px solid var(--app-border-color, #eee);
}
.empty {
  margin-top: 24px;
}
</style>
