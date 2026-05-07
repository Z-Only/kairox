<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useCatalogStore } from "@/stores/catalog";

const { t } = useI18n();
const catalog = useCatalogStore();
const props = defineProps<{ catalogId: string }>();
const emit = defineEmits<{ close: [] }>();

const outcome = computed(() => catalog.installState[props.catalogId]);

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

// Show a spinner while we are still mid-install (no outcome yet); once an
// outcome lands we render the per-step status list.
const inFlight = computed(() => !outcome.value);

// Top-level alert shown above the step list. Mirrors the failure-vs-success
// shape of the outcome union so users see one obvious banner.
const alertType = computed<"info" | "success" | "warning" | "error">(() => {
  if (!outcome.value) return "info";
  switch (outcome.value.kind) {
    case "installed":
      return "success";
    case "already_installed":
      return "info";
    case "runtime_missing":
    case "invalid_env":
      return "error";
    default:
      return "warning";
  }
});

// Modal title tracks the outcome.kind so users see the result state in the
// header rather than a static "Installing…". `inFlight` (no outcome yet)
// keeps the original copy; success-shaped kinds collapse to one label and
// failure-shaped kinds to another.
const modalTitle = computed<string>(() => {
  if (!outcome.value) return t("marketplace.install.titleInstalling");
  switch (outcome.value.kind) {
    case "installed":
    case "already_installed":
      return t("marketplace.install.titleComplete");
    case "runtime_missing":
    case "invalid_env":
      return t("marketplace.install.titleFailed");
    default:
      return t("marketplace.install.titleInstalling");
  }
});
</script>

<template>
  <!-- NModal with show=true mirrors the previous always-on `position: fixed`
       behaviour while picking up theming, focus trap and overlay handling.
       data-test="install-progress" stays attached to the modal body for
       the existing selectors. -->
  <NModal
    :show="true"
    preset="card"
    :mask-closable="false"
    :bordered="true"
    size="small"
    style="width: min(480px, 90vw)"
    :title="modalTitle"
    @close="emit('close')"
  >
    <div data-test="install-progress" class="install-progress">
      <NAlert v-if="!inFlight" :type="alertType" :show-icon="true" :bordered="false">
        <span v-if="outcome?.kind === 'installed'">
          {{ t("marketplace.install.alertInstalled") }}
        </span>
        <span v-else-if="outcome?.kind === 'already_installed'">
          {{ t("marketplace.install.alertAlreadyInstalled") }}
        </span>
        <span v-else-if="outcome?.kind === 'runtime_missing'">
          {{
            t("marketplace.install.alertMissingRuntimes", {
              runtimes: outcome.missing_runtimes.join(", ")
            })
          }}
        </span>
        <span v-else-if="outcome?.kind === 'invalid_env'">
          {{
            t("marketplace.install.alertMissingEnv", {
              keys: outcome.missing_env_keys.join(", ")
            })
          }}
        </span>
        <span v-else>{{ t("marketplace.install.alertUnexpected") }}</span>
      </NAlert>
      <NSpin v-else size="small" />

      <ul class="steps">
        <li :class="{ ok: runtimeOk, fail: outcome?.kind === 'runtime_missing' }">
          {{ t("marketplace.install.stepDetectRuntime") }}
        </li>
        <li :class="{ ok: writeOk, fail: outcome?.kind === 'invalid_env' }">
          {{ t("marketplace.install.stepWriteConfig") }}
        </li>
        <li :class="{ ok: startOk }">
          {{ t("marketplace.install.stepStartServer") }}
        </li>
      </ul>
    </div>

    <template #footer>
      <NButton size="small" data-test="install-close" @click="emit('close')">
        {{ t("common.close") }}
      </NButton>
    </template>
  </NModal>
</template>

<style scoped>
.install-progress {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.steps {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.steps li::before {
  content: "○ ";
  color: var(--app-text-color-3, #999);
}
.steps li.ok::before {
  content: "✓ ";
  color: var(--app-success-color, #18a058);
}
.steps li.fail::before {
  content: "✗ ";
  color: var(--app-error-color, #d03050);
}
</style>
