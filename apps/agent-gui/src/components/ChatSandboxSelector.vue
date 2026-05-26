<script setup lang="ts">
const props = defineProps<{
  sandboxPolicy: string;
}>();

const emit = defineEmits<{
  selectSandbox: [sandboxJson: string];
}>();

const open = defineModel<boolean>("open", { default: false });

const { t } = useI18n();

const sandboxOptions = [
  {
    value: "read_only",
    json: '{"kind":"read_only"}',
    labelKey: "chat.sandbox.readOnly"
  },
  {
    value: "workspace_write",
    json: '{"kind":"workspace_write","network_access":false,"writable_roots":[]}',
    labelKey: "chat.sandbox.workspaceWrite"
  },
  {
    value: "danger_full_access",
    json: '{"kind":"danger_full_access"}',
    labelKey: "chat.sandbox.dangerFullAccess"
  }
];

function parseKind(jsonStr: string): string {
  try {
    const parsed = JSON.parse(jsonStr);
    return typeof parsed?.kind === "string" ? parsed.kind : "";
  } catch {
    return "";
  }
}

const currentKind = computed(() => parseKind(props.sandboxPolicy));

const sandboxDisplay = computed(() => {
  const opt = sandboxOptions.find((o) => o.value === currentKind.value);
  return opt ? t(opt.labelKey) : currentKind.value || props.sandboxPolicy;
});

function selectSandbox(jsonStr: string) {
  emit("selectSandbox", jsonStr);
  open.value = false;
}
</script>

<template>
  <KxPopover
    v-model:open="open"
    content-data-test="chat-sandbox-popover"
    content-class="chat-sandbox-popover-panel"
    side="top"
    align="start"
  >
    <template #trigger>
      <button
        class="chat-sandbox-trigger"
        type="button"
        data-test="chat-sandbox-trigger"
        :aria-label="t('chat.selectSandboxAria', { sandbox: sandboxDisplay })"
      >
        {{ sandboxDisplay }}
      </button>
    </template>
    <template #content>
      <header class="kx-popover-panel__header chat-sandbox-popover-header">
        {{ t("chat.sandbox.label") }}
      </header>
      <ul class="kx-popover-list chat-sandbox-list">
        <li v-for="option in sandboxOptions" :key="option.value">
          <button
            type="button"
            :class="[
              'kx-popover-option',
              'chat-sandbox-option',
              {
                selected: option.value === currentKind,
                'kx-popover-option--selected': option.value === currentKind
              }
            ]"
            :data-test="`chat-sandbox-option-${option.value}`"
            :aria-current="option.value === currentKind ? 'true' : undefined"
            @click="selectSandbox(option.json)"
          >
            <span class="kx-popover-option__label chat-sandbox-option-label">
              {{ t(option.labelKey) }}
            </span>
          </button>
        </li>
      </ul>
    </template>
  </KxPopover>
</template>

<style scoped>
.chat-sandbox-trigger {
  max-width: min(100%, 160px);
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--app-primary-color) 22%, var(--app-border-color));
  border-radius: 999px;
  padding: 3px 10px;
  cursor: pointer;
  background: color-mix(in srgb, var(--app-primary-color) 10%, var(--app-card-color));
  color: var(--app-text-color);
  font: inherit;
  font-size: 12px;
  line-height: 18px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chat-sandbox-trigger:hover {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 16%, var(--app-card-color));
}
.chat-sandbox-trigger:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-sandbox-trigger {
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }
}
.chat-sandbox-popover-panel {
  min-width: 200px;
}
.chat-sandbox-option {
  flex-direction: column;
  align-items: flex-start;
}
</style>
