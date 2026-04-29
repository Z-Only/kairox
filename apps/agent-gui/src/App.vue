<script setup lang="ts">
import PermissionCenter from "./components/PermissionCenter.vue";
import TraceTimeline from "./components/TraceTimeline.vue";

const events = [{ event_type: "WorkspaceOpened" }, { event_type: "UserMessageAdded" }];
</script>

<template>
  <main class="workbench">
    <aside class="sidebar">
      <h1>Kairox</h1>
      <p>Local workbench</p>
    </aside>
    <section class="session">
      <h2>Session</h2>
      <p>Shared core session projection will render here.</p>
    </section>
    <TraceTimeline :events="events" />
    <PermissionCenter />
  </main>
</template>

<style scoped>
.workbench {
  display: grid;
  grid-template-columns: 220px 1fr 320px;
  min-height: 100vh;
  font-family: system-ui, sans-serif;
}
.sidebar,
.session,
.trace,
.permission-center {
  padding: 16px;
  border-right: 1px solid #d7d7d7;
}
</style>
