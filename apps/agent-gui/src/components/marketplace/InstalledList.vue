<script setup lang="ts">
import { onMounted } from "vue";
import {
  catalogState,
  fetchInstalled,
  uninstallEntry
} from "../../stores/catalog";

onMounted(() => fetchInstalled());

async function onUninstall(serverId: string) {
  await uninstallEntry(serverId);
}
</script>

<template>
  <table class="installed" data-test="installed-list">
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
      <tr v-for="row in catalogState.installed" :key="row.server_id">
        <td>{{ row.display_name }}</td>
        <td>{{ row.source ?? "(manual)" }}</td>
        <td>
          <span :class="{ dot: true, running: row.running }" />
          {{ row.running ? "running" : "stopped" }}
        </td>
        <td>{{ row.installed_at }}</td>
        <td>
          <button
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
          </button>
        </td>
      </tr>
    </tbody>
  </table>
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
  border-bottom: 1px solid #eee;
}
.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #999;
  margin-right: 4px;
}
.dot.running {
  background: #2a2;
}
</style>
