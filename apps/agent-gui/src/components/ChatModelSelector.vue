<script setup lang="ts">
import {
  DEFAULT_REASONING_EFFORT,
  DEFAULT_REASONING_EFFORTS,
  formatProfileDisplay
} from "@/stores/session";
import type { ProfileInfo } from "@/types";

const props = defineProps<{
  modelOptions: ProfileInfo[];
  currentProfile: string;
  switchingModel: boolean;
  activeProfileDisplay: string;
  currentReasoningEffort: string | null;
}>();

const emit = defineEmits<{
  selectModel: [alias: string, reasoningEffort?: string];
}>();

const open = defineModel<boolean>("open", { default: false });

const { t } = useI18n();

const hoveredModelAlias = ref<string | null>(null);
const customReasoningEffort = ref("");

const hoveredModel = computed(() =>
  props.modelOptions.find((profile) => profile.alias === hoveredModelAlias.value)
);

const selectedModelAlias = computed(() => {
  if (props.modelOptions.some((profile) => profile.alias === props.currentProfile)) {
    return props.currentProfile;
  }
  return props.modelOptions[0]?.alias ?? props.currentProfile;
});

const reasoningModel = computed(() => {
  if (hoveredModelAlias.value) {
    return hoveredModel.value?.supports_reasoning ? hoveredModel.value : null;
  }
  const current = props.modelOptions.find((profile) => profile.alias === selectedModelAlias.value);
  return current?.supports_reasoning ? current : null;
});

const activeReasoningEffort = computed(
  () => props.currentReasoningEffort ?? DEFAULT_REASONING_EFFORT
);

const reasoningOptions = computed(() => {
  const options: string[] = [...DEFAULT_REASONING_EFFORTS];
  const current = activeReasoningEffort.value;
  if (current && !options.includes(current)) {
    options.push(current);
  }
  return options;
});

function getModelOptionDisplay(profile: ProfileInfo): string {
  return formatProfileDisplay(profile);
}

function onModelHover(profile: ProfileInfo) {
  hoveredModelAlias.value = profile.alias;
  if (!profile.supports_reasoning) {
    customReasoningEffort.value = "";
  }
}

function onSelectReasoningEffort(effort: string) {
  const profile = reasoningModel.value;
  if (!profile) return;
  emit("selectModel", profile.alias, effort);
}

function onApplyCustomReasoning() {
  const effort = customReasoningEffort.value.trim();
  if (!effort) return;
  onSelectReasoningEffort(effort);
}

function selectModelProfile(alias: string) {
  emit("selectModel", alias);
}
</script>

<template>
  <KxPopover
    v-model:open="open"
    content-data-test="chat-model-popover"
    content-class="chat-model-popover-panel"
    width="min(92vw, 520px)"
    side="top"
    align="start"
  >
    <template #trigger>
      <button
        class="chat-model-trigger"
        type="button"
        data-test="chat-model-trigger"
        :aria-label="t('chat.selectModelAria', { model: props.activeProfileDisplay })"
      >
        {{ props.activeProfileDisplay }}
      </button>
    </template>
    <template #content>
      <div class="chat-model-column">
        <header class="kx-popover-panel__header chat-model-popover-header">
          {{ t("chat.model") }}
        </header>
        <ul class="kx-popover-list chat-model-list">
          <li v-for="profile in props.modelOptions" :key="profile.alias">
            <button
              type="button"
              :class="[
                'kx-popover-option',
                'chat-model-option',
                {
                  selected: profile.alias === selectedModelAlias,
                  hovered: profile.alias === hoveredModelAlias,
                  'kx-popover-option--selected': profile.alias === selectedModelAlias
                }
              ]"
              :data-test="`chat-model-option-${profile.alias}`"
              :aria-current="profile.alias === selectedModelAlias ? 'true' : undefined"
              :disabled="props.switchingModel"
              @mouseenter="onModelHover(profile)"
              @focus="onModelHover(profile)"
              @click="selectModelProfile(profile.alias)"
            >
              <span class="kx-popover-option__label chat-model-option-label">
                {{ getModelOptionDisplay(profile) }}
              </span>
              <span class="kx-popover-option__meta chat-model-option-meta">
                {{ profile.alias }}
                <span v-if="profile.alias === selectedModelAlias">
                  · {{ t("chat.currentModel") }}</span
                >
              </span>
            </button>
          </li>
        </ul>
      </div>
      <div v-if="reasoningModel" class="chat-reasoning-panel" data-test="chat-reasoning-panel">
        <header class="kx-popover-panel__header chat-model-popover-header">
          {{ t("chat.reasoning") }}
        </header>
        <div class="chat-reasoning-list">
          <button
            v-for="effort in reasoningOptions"
            :key="effort"
            type="button"
            :class="[
              'kx-popover-option',
              'chat-reasoning-option',
              {
                selected: effort === activeReasoningEffort,
                'kx-popover-option--selected': effort === activeReasoningEffort
              }
            ]"
            :data-test="`chat-reasoning-option-${effort}`"
            :disabled="props.switchingModel"
            @click="onSelectReasoningEffort(effort)"
          >
            {{ effort }}
          </button>
        </div>
        <form class="chat-reasoning-custom" @submit.prevent="onApplyCustomReasoning">
          <KxInput
            v-model="customReasoningEffort"
            class="chat-reasoning-custom-input"
            data-test="chat-reasoning-custom-input"
            :placeholder="t('chat.customReasoningPlaceholder')"
            :disabled="props.switchingModel"
            size="compact"
          />
          <button
            class="chat-reasoning-custom-apply"
            data-test="chat-reasoning-custom-apply"
            type="button"
            :disabled="props.switchingModel || !customReasoningEffort.trim()"
            @click="onApplyCustomReasoning"
          >
            {{ t("chat.applyReasoning") }}
          </button>
        </form>
      </div>
    </template>
  </KxPopover>
</template>

<style scoped>
.chat-model-trigger {
  max-width: min(100%, 280px);
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
.chat-model-trigger:hover {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 16%, var(--app-card-color));
}
.chat-model-trigger:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-model-trigger {
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }
}
.chat-model-popover-panel {
  display: flex;
  min-width: 240px;
  gap: 8px;
  align-items: stretch;
}
.chat-model-column {
  min-width: 240px;
}
.chat-model-option {
  flex-direction: column;
  align-items: flex-start;
}
.chat-reasoning-panel {
  width: 184px;
  min-width: 184px;
  border-left: 1px solid var(--app-border-color);
  padding-left: 8px;
}
.chat-reasoning-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px;
}
.chat-reasoning-option {
  overflow: hidden;
  padding: 7px 8px;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chat-reasoning-custom {
  display: flex;
  gap: 4px;
  margin-top: 8px;
}
.chat-reasoning-custom-input {
  width: 0;
  min-width: 0;
  flex: 1;
  font-size: 12px;
}
.chat-reasoning-custom-apply {
  flex: 0 0 auto;
  border: 1px solid var(--app-primary-color);
  border-radius: 6px;
  padding: 6px 8px;
  cursor: pointer;
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color, #fff);
  font: inherit;
  font-size: 12px;
}
.chat-reasoning-option:disabled,
.chat-reasoning-custom-apply:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
@media (max-width: 560px) {
  .chat-model-popover-panel {
    flex-direction: column;
  }
  .chat-reasoning-panel {
    width: auto;
    min-width: 0;
    border-top: 1px solid var(--app-border-color);
    border-left: 0;
    padding-top: 8px;
    padding-left: 0;
  }
}
@media (prefers-reduced-motion: no-preference) {
  .chat-model-popover-panel {
    animation: popover-in 0.15s ease;
  }
}
@keyframes popover-in {
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
