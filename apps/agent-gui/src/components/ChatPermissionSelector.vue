<script setup lang="ts">
const props = defineProps<{
  permissionMode: string;
}>();

const emit = defineEmits<{
  selectPermission: [mode: string];
}>();

const open = defineModel<boolean>("open", { default: false });

const { t } = useI18n();

const permissionOptions = [
  { value: "read_only", label: "Read Only" },
  { value: "suggest", label: "Suggest" },
  { value: "agent", label: "Agent" },
  { value: "autonomous", label: "Autonomous" },
  { value: "interactive", label: "Interactive" }
];

const permissionDisplay = computed(() => {
  const opt = permissionOptions.find((o) => o.value === props.permissionMode);
  return opt ? opt.label : props.permissionMode;
});

function selectPermissionMode(mode: string) {
  emit("selectPermission", mode);
  open.value = false;
}
</script>

<template>
  <KxPopover
    v-model:open="open"
    content-data-test="chat-permission-popover"
    content-class="chat-permission-popover-panel"
    side="top"
    align="start"
  >
    <template #trigger>
      <button
        class="chat-permission-trigger"
        type="button"
        data-test="chat-permission-trigger"
        :aria-label="t('chat.selectPermissionAria', { mode: permissionDisplay })"
      >
        {{ permissionDisplay }}
      </button>
    </template>
    <template #content>
      <header class="kx-popover-panel__header chat-permission-popover-header">
        {{ t("chat.permission") }}
      </header>
      <ul class="kx-popover-list chat-permission-list">
        <li v-for="option in permissionOptions" :key="option.value">
          <button
            type="button"
            :class="[
              'kx-popover-option',
              'chat-permission-option',
              {
                selected: option.value === props.permissionMode,
                'kx-popover-option--selected': option.value === props.permissionMode
              }
            ]"
            :data-test="`chat-permission-option-${option.value}`"
            :aria-current="option.value === props.permissionMode ? 'true' : undefined"
            @click="selectPermissionMode(option.value)"
          >
            <span class="kx-popover-option__label chat-permission-option-label">
              {{ option.label }}
            </span>
          </button>
        </li>
      </ul>
    </template>
  </KxPopover>
</template>

<style scoped>
.chat-permission-trigger {
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
.chat-permission-trigger:hover {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 16%, var(--app-card-color));
}
.chat-permission-trigger:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-permission-trigger {
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }
}
.chat-permission-popover-panel {
  min-width: 180px;
}
.chat-permission-option {
  flex-direction: column;
  align-items: flex-start;
}
</style>
