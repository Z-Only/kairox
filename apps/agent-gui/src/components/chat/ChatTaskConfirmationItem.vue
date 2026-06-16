<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import type { TaskConfirmationOption } from "@/types/trace";

const { t } = useI18n();

const props = defineProps<{
  id: string;
  prompt: string;
  options: TaskConfirmationOption[];
  allowMultiple: boolean;
  allowCustom: boolean;
  rawEvent?: string;
}>();

const selectedOptionIds = ref<string[]>([]);
const customResponse = ref("");
const submitting = ref(false);
const error = ref<string | null>(null);

const trimmedCustomResponse = computed(() => customResponse.value.trim());
const canSubmit = computed(
  () => selectedOptionIds.value.length > 0 || trimmedCustomResponse.value.length > 0
);

function setSingleOption(optionId: string) {
  selectedOptionIds.value = [optionId];
}

function toggleMultipleOption(optionId: string, checked: boolean) {
  if (checked) {
    if (!selectedOptionIds.value.includes(optionId)) {
      selectedOptionIds.value = [...selectedOptionIds.value, optionId];
    }
  } else {
    selectedOptionIds.value = selectedOptionIds.value.filter((id) => id !== optionId);
  }
}

function onOptionChange(optionId: string, event: Event) {
  const checked = (event.target as HTMLInputElement).checked;
  if (props.allowMultiple) {
    toggleMultipleOption(optionId, checked);
  } else if (checked) {
    setSingleOption(optionId);
  }
}

async function submit() {
  if (!canSubmit.value || submitting.value) return;
  submitting.value = true;
  error.value = null;
  try {
    await invoke("resolve_task_confirmation", {
      decision: {
        request_id: props.id,
        selected_option_ids: selectedOptionIds.value,
        custom_response: trimmedCustomResponse.value.length > 0 ? trimmedCustomResponse.value : null
      }
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <article class="chat-task-confirmation-item" data-test="chat-task-confirmation-item" tabindex="0">
    <div class="chat-task-confirmation-item__header">
      <span class="chat-task-confirmation-item__icon" aria-hidden="true">?</span>
      <div class="chat-task-confirmation-item__title">
        <strong>{{ t("chatStream.taskConfirmation.title") }}</strong>
        <p>{{ prompt }}</p>
      </div>
    </div>

    <div class="chat-task-confirmation-item__options">
      <label v-for="option in options" :key="option.id" class="chat-task-confirmation-item__option">
        <input
          :type="allowMultiple ? 'checkbox' : 'radio'"
          :name="`task-confirmation-option-${id}`"
          :checked="selectedOptionIds.includes(option.id)"
          :data-test="`task-confirmation-option-${option.id}`"
          @change="onOptionChange(option.id, $event)"
        />
        <span class="chat-task-confirmation-item__option-text">
          <span class="chat-task-confirmation-item__option-label">{{ option.label }}</span>
          <span v-if="option.description" class="chat-task-confirmation-item__option-description">
            {{ option.description }}
          </span>
        </span>
      </label>
    </div>

    <label v-if="allowCustom" class="chat-task-confirmation-item__custom">
      <span>{{ t("chatStream.taskConfirmation.customLabel") }}</span>
      <textarea
        v-model="customResponse"
        data-test="task-confirmation-custom"
        rows="2"
        :placeholder="t('chatStream.taskConfirmation.customPlaceholder')"
      />
    </label>

    <p v-if="error" class="chat-task-confirmation-item__error" data-test="task-confirmation-error">
      {{ error }}
    </p>

    <div class="chat-task-confirmation-item__actions">
      <KxButton
        variant="primary"
        size="xs"
        data-test="task-confirmation-submit"
        :disabled="!canSubmit || submitting"
        @click="submit"
      >
        {{
          submitting
            ? t("chatStream.taskConfirmation.submitting")
            : t("chatStream.taskConfirmation.submit")
        }}
      </KxButton>
    </div>
  </article>
</template>

<style scoped>
.chat-task-confirmation-item {
  display: block;
  width: 100%;
  max-width: 100%;
  padding: 8px;
  border: 1px solid var(--app-warning-color, #e8b339);
  border-radius: 4px;
  background: var(--app-warning-color-suppl, #fff8e6);
  color: var(--app-text-color);
  outline: none;
  box-sizing: border-box;
}

.chat-task-confirmation-item:focus-visible {
  outline: 2px solid var(--app-primary-color, #2080f0);
  outline-offset: 2px;
}

.chat-task-confirmation-item__header {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  min-width: 0;
}

.chat-task-confirmation-item__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  border-radius: 50%;
  border: 1px solid color-mix(in srgb, var(--app-warning-color, #e8b339) 70%, transparent);
  font-size: 12px;
  font-weight: 700;
}

.chat-task-confirmation-item__title {
  min-width: 0;
}

.chat-task-confirmation-item__title strong {
  display: block;
  font-size: 12px;
  line-height: 1.35;
}

.chat-task-confirmation-item__title p {
  margin: 2px 0 0;
  font-size: 12px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.chat-task-confirmation-item__options {
  display: grid;
  gap: 6px;
  margin-top: 8px;
}

.chat-task-confirmation-item__option {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  min-width: 0;
  padding: 6px 8px;
  border: 1px solid var(--app-border-color, #e0e0e0);
  border-radius: 4px;
  background: color-mix(in srgb, var(--app-card-color, #fff) 82%, transparent);
  cursor: pointer;
}

.chat-task-confirmation-item__option input {
  margin-top: 2px;
  flex: 0 0 auto;
}

.chat-task-confirmation-item__option-text {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.chat-task-confirmation-item__option-label {
  font-size: 12px;
  font-weight: 600;
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.chat-task-confirmation-item__option-description {
  font-size: 11px;
  color: var(--app-text-color-3, #777);
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.chat-task-confirmation-item__custom {
  display: grid;
  gap: 4px;
  margin-top: 8px;
  font-size: 11px;
  color: var(--app-text-color-2, #666);
}

.chat-task-confirmation-item__custom textarea {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  resize: vertical;
  border: 1px solid var(--app-border-color, #d9d9d9);
  border-radius: 4px;
  background: var(--app-input-color, #fff);
  color: var(--app-text-color);
  font: inherit;
  font-size: 12px;
  line-height: 1.4;
  padding: 6px 8px;
}

.chat-task-confirmation-item__error {
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--app-error-color, #d03050);
}

.chat-task-confirmation-item__actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}
</style>
