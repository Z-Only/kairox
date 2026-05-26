<script setup lang="ts">
const props = defineProps<{
  approvalPolicy: string;
}>();

const emit = defineEmits<{
  selectApproval: [approval: string];
}>();

const open = defineModel<boolean>("open", { default: false });

const { t } = useI18n();

const approvalOptions = [
  { value: "never", labelKey: "chat.approval.never" },
  { value: "on_request", labelKey: "chat.approval.onRequest" },
  { value: "always", labelKey: "chat.approval.always" }
];

const approvalDisplay = computed(() => {
  const opt = approvalOptions.find((o) => o.value === props.approvalPolicy);
  return opt ? t(opt.labelKey) : props.approvalPolicy;
});

function selectApproval(approval: string) {
  emit("selectApproval", approval);
  open.value = false;
}
</script>

<template>
  <KxPopover
    v-model:open="open"
    content-data-test="chat-approval-popover"
    content-class="chat-approval-popover-panel"
    side="top"
    align="start"
  >
    <template #trigger>
      <button
        class="chat-approval-trigger"
        type="button"
        data-test="chat-approval-trigger"
        :aria-label="t('chat.selectApprovalAria', { approval: approvalDisplay })"
      >
        {{ approvalDisplay }}
      </button>
    </template>
    <template #content>
      <header class="kx-popover-panel__header chat-approval-popover-header">
        {{ t("chat.approval.label") }}
      </header>
      <ul class="kx-popover-list chat-approval-list">
        <li v-for="option in approvalOptions" :key="option.value">
          <button
            type="button"
            :class="[
              'kx-popover-option',
              'chat-approval-option',
              {
                selected: option.value === props.approvalPolicy,
                'kx-popover-option--selected': option.value === props.approvalPolicy
              }
            ]"
            :data-test="`chat-approval-option-${option.value}`"
            :aria-current="option.value === props.approvalPolicy ? 'true' : undefined"
            @click="selectApproval(option.value)"
          >
            <span class="kx-popover-option__label chat-approval-option-label">
              {{ t(option.labelKey) }}
            </span>
          </button>
        </li>
      </ul>
    </template>
  </KxPopover>
</template>

<style scoped>
.chat-approval-trigger {
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
.chat-approval-trigger:hover {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 16%, var(--app-card-color));
}
.chat-approval-trigger:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-approval-trigger {
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }
}
.chat-approval-popover-panel {
  min-width: 180px;
}
.chat-approval-option {
  flex-direction: column;
  align-items: flex-start;
}
</style>
