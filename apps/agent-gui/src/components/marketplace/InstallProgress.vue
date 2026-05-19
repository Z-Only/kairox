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
  <!-- Modal overlay replaces NModal. data-test="install-progress" stays
       attached to the modal body for the existing selectors. -->
  <Teleport to="body">
    <div class="modal-overlay">
      <div class="modal-card">
        <header class="modal-header">
          <span class="modal-title">{{ modalTitle }}</span>
          <button class="btn modal-close-btn" aria-label="Close" @click="emit('close')">✕</button>
        </header>

        <div data-test="install-progress" class="install-progress">
          <div v-if="!inFlight" :class="['alert', `alert-${alertType}`]">
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
          </div>
          <div v-else class="spinner" />

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

        <footer class="modal-footer">
          <button class="btn btn-sm" data-test="install-close" @click="emit('close')">
            {{ t("common.close") }}
          </button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--app-z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--app-backdrop-color);
}
.modal-card {
  width: min(480px, 90vw);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  box-shadow: var(--app-shadow-2);
  color: var(--app-text-color);
}
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--app-border-color);
}
.modal-title {
  font-weight: 600;
  font-size: 15px;
}
.modal-close-btn {
  font-size: 16px;
  padding: 2px 6px;
  line-height: 1;
}
.install-progress {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
}
.modal-footer {
  padding: 10px 16px;
  border-top: 1px solid var(--app-border-color);
}
.alert {
  padding: 8px 12px;
  border-radius: 4px;
  font-size: 13px;
}
.alert-success {
  background: var(--app-success-bg);
  color: var(--app-success-color);
}
.alert-info {
  background: var(--app-bg-color);
  color: var(--app-info-color);
}
.alert-warning {
  background: var(--app-warning-bg);
  color: var(--app-warning-color);
}
.alert-error {
  background: var(--app-error-bg);
  color: var(--app-error-color);
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
.btn-sm {
  height: 28px;
}
.spinner {
  display: inline-block;
  width: 18px;
  height: 18px;
  border: 2px solid var(--app-border-color);
  border-top-color: var(--app-primary-color);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
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
  color: var(--app-text-color-3);
}
.steps li.ok::before {
  content: "✓ ";
  color: var(--app-success-color);
}
.steps li.fail::before {
  content: "✗ ";
  color: var(--app-error-color);
}
</style>
