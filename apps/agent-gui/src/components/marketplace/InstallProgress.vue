<script setup lang="ts">
import { computed } from "vue";
import { catalogState } from "../../stores/catalog";

const props = defineProps<{ catalogId: string }>();
defineEmits<{ close: [] }>();

const outcome = computed(() => catalogState.installState[props.catalogId]);

// Backend installer order: runtime probe → env validate → toml write → start.
// "Detect runtime" only ticks when we either succeeded fully or failed *after*
// runtime probe (i.e. invalid_env / already_installed / installed). It does
// NOT tick on runtime_missing — that's the explicit failure path.
const runtimeOk = computed(() => !!outcome.value && outcome.value.kind !== "runtime_missing");
const writeOk = computed(
  () => outcome.value?.kind === "installed" || outcome.value?.kind === "already_installed"
);
// `outcome.started` mirrors the install request's auto_start flag rather than
// confirmed liveness. UI shows it as "Start server requested" when ticked.
const startOk = computed(
  () => outcome.value?.kind === "installed" && outcome.value.started === true
);
</script>

<template>
  <div class="modal" role="dialog" data-test="install-progress">
    <h3>Installing…</h3>
    <ul>
      <li :class="{ ok: runtimeOk, fail: outcome?.kind === 'runtime_missing' }">Detect runtime</li>
      <li :class="{ ok: writeOk, fail: outcome?.kind === 'invalid_env' }">Write config</li>
      <li :class="{ ok: startOk }">Start server</li>
    </ul>
    <p v-if="outcome?.kind === 'runtime_missing'">
      Missing runtimes: {{ outcome.missing_runtimes.join(", ") }}
    </p>
    <p v-if="outcome?.kind === 'invalid_env'">
      Required env: {{ outcome.missing_env_keys.join(", ") }}
    </p>
    <p v-if="outcome?.kind === 'already_installed'">Already installed.</p>
    <button data-test="install-close" @click="$emit('close')">Close</button>
  </div>
</template>

<style scoped>
.modal {
  position: fixed;
  inset: 20% 25%;
  background: var(--surface, #fff);
  border: 1px solid #ccc;
  padding: 16px;
  z-index: 60;
}
li.ok::before {
  content: "✓ ";
  color: green;
}
li.fail::before {
  content: "✗ ";
  color: #c33;
}
</style>
