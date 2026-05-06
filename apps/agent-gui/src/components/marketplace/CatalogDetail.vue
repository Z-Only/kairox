<script setup lang="ts">
import { ref, computed, watch } from "vue";
import type {
  ServerEntryResponse,
  InstallRequestPayload
} from "../../generated/commands";
import { useCatalogStore } from "@/stores/catalog";
import {
  parseRequirements,
  parseDefaultEnv
} from "../../composables/useMarketplace";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";
import InstallProgress from "./InstallProgress.vue";

const catalog = useCatalogStore();
const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ close: [] }>();

const requirements = computed(() => parseRequirements(props.entry));
const envSpec = computed(() => parseDefaultEnv(props.entry));
const overrides = ref<Record<string, string>>({});
// Trust grant must be opt-in: catalog "verified" means the *distribution
// channel* is trusted, not that runtime tool calls should bypass the
// PermissionCenter. Default to false and let the user opt in explicitly.
const trustGrant = ref(false);
const autoStart = ref(true);
const showProgress = ref(false);

// Re-initialise local form state whenever the selected entry changes.
// Using `watch(..., immediate: true)` instead of `onMounted` so that switching
// entries without unmounting the drawer (e.g. future keep-alive use) still
// resets overrides + checkboxes cleanly.
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
    showProgress.value = false;
  },
  { immediate: true }
);

async function onInstall() {
  const req: InstallRequestPayload = {
    catalog_id: props.entry.id,
    source: props.entry.source,
    server_id_override: null,
    env_overrides: overrides.value,
    trust_grant: trustGrant.value,
    auto_start: autoStart.value
  };
  showProgress.value = true;
  await catalog.installEntry(req);
}
</script>

<template>
  <aside
    class="drawer"
    role="dialog"
    aria-modal="true"
    data-test="catalog-detail"
  >
    <header>
      <h2>{{ entry.display_name }}</h2>
      <button aria-label="Close" @click="emit('close')">×</button>
    </header>
    <p>{{ entry.description }}</p>
    <a
      v-if="entry.homepage"
      :href="entry.homepage"
      target="_blank"
      rel="noopener"
    >
      Homepage
    </a>

    <section>
      <h3>Requirements</h3>
      <RuntimeMissingHint :requirements="requirements" />
    </section>

    <section>
      <h3>Configure</h3>
      <div v-for="spec in envSpec" :key="spec.key" class="field">
        <label> {{ spec.label }}<span v-if="spec.required">*</span> </label>
        <input
          v-model="overrides[spec.key]"
          :type="spec.secret ? 'password' : 'text'"
          :placeholder="spec.default ?? ''"
          :data-test="`env-${spec.key}`"
        />
        <small>{{ spec.description }}</small>
      </div>
    </section>

    <section class="options">
      <label>
        <input v-model="trustGrant" type="checkbox" />
        Trust this server (skip per-tool permission prompts)
      </label>
      <p v-if="entry.trust === 'verified'" class="hint-verified">
        This entry comes from a verified source. You can grant runtime trust to
        skip permission prompts, but it remains opt-in.
      </p>
      <label>
        <input v-model="autoStart" type="checkbox" /> Start after install
      </label>
    </section>

    <footer>
      <button data-test="catalog-install" @click="onInstall">Install</button>
    </footer>

    <InstallProgress
      v-if="showProgress"
      :catalog-id="entry.id"
      @close="showProgress = false"
    />
  </aside>
</template>

<style scoped>
.drawer {
  position: fixed;
  right: 0;
  top: 0;
  bottom: 0;
  width: min(480px, 90vw);
  background: var(--surface, #fff);
  border-left: 1px solid #ddd;
  padding: 16px;
  overflow-y: auto;
  z-index: 50;
}
.drawer header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.field {
  display: flex;
  flex-direction: column;
  margin-bottom: 8px;
}
.options {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.hint-verified {
  margin: 0;
  font-size: 12px;
  color: var(--muted, #555);
}
</style>
