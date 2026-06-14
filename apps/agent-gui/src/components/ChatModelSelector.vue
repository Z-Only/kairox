<script setup lang="ts">
import { DEFAULT_REASONING_EFFORTS, formatProfileDisplay } from "@/stores/session";
import type { ProfileInfo } from "@/types";

function formatContextWindow(tokens: number): string {
  if (tokens >= 1_000_000) {
    const m = tokens / 1_000_000;
    return m % 1 === 0 ? `${m}M` : `${m.toFixed(1)}M`;
  }
  const k = tokens / 1_000;
  return k % 1 === 0 ? `${k}K` : `${k.toFixed(1)}K`;
}

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
const modelCard = ref<HTMLElement | null>(null);
const reasoningPanel = ref<HTMLElement | null>(null);
const modelOptionEls = new Map<string, HTMLElement>();
const reasoningAnchorY = ref(52);

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

const activeReasoningEffort = computed(() => props.currentReasoningEffort);

const reasoningOptions = computed(() => {
  const options: string[] = [...DEFAULT_REASONING_EFFORTS];
  const current = activeReasoningEffort.value;
  if (current && !options.includes(current)) {
    options.push(current);
  }
  return options;
});

/** Display names that appear more than once across all model options. */
const duplicateDisplayNames = computed(() => {
  const counts = new Map<string, number>();
  for (const profile of props.modelOptions) {
    const display = formatProfileDisplay(profile);
    counts.set(display, (counts.get(display) ?? 0) + 1);
  }
  const duplicates = new Set<string>();
  for (const [display, count] of counts) {
    if (count > 1) duplicates.add(display);
  }
  return duplicates;
});

function getModelOptionDisplay(profile: ProfileInfo): string {
  const display = formatProfileDisplay(profile);
  if (duplicateDisplayNames.value.has(display)) {
    return `${display} (${profile.alias})`;
  }
  return display;
}

function onModelHover(profile: ProfileInfo) {
  hoveredModelAlias.value = profile.alias;
  if (!profile.supports_reasoning) {
    customReasoningEffort.value = "";
  }
  updateReasoningAnchor(profile.alias);
}

function onSelectReasoningEffort(effort: string) {
  const profile = reasoningModel.value;
  if (!profile) return;
  selectModelProfile(profile.alias, effort);
}

function onApplyCustomReasoning() {
  const effort = customReasoningEffort.value.trim();
  if (!effort) return;
  onSelectReasoningEffort(effort);
}

function selectModelProfile(alias: string, reasoningEffort?: string) {
  open.value = false;
  if (reasoningEffort === undefined) {
    emit("selectModel", alias);
    return;
  }
  emit("selectModel", alias, reasoningEffort);
}

function setModelOptionEl(alias: string, el: unknown): void {
  if (el instanceof HTMLElement) {
    modelOptionEls.set(alias, el);
    return;
  }
  modelOptionEls.delete(alias);
}

function updateReasoningAnchor(alias = reasoningModel.value?.alias): void {
  if (!alias) return;
  void nextTick(() => {
    const card = modelCard.value;
    const option = modelOptionEls.get(alias);
    if (!card || !option) return;
    const cardRect = card.getBoundingClientRect();
    const optionRect = option.getBoundingClientRect();
    const panelHeight = reasoningPanel.value?.getBoundingClientRect().height || 80;
    const panelHalfHeight = panelHeight / 2;
    const cardHeight = cardRect.height || card.clientHeight;
    const center = optionRect.top - cardRect.top + optionRect.height / 2;
    const min = panelHalfHeight;
    const max = Math.max(min, cardHeight - panelHalfHeight);
    reasoningAnchorY.value = Math.min(Math.max(center, min), max);
  });
}

watch(
  [() => open.value, () => reasoningModel.value?.alias],
  ([isOpen]) => {
    if (isOpen) updateReasoningAnchor();
  },
  { flush: "post" }
);
</script>

<template>
  <KxPopover
    v-model:open="open"
    content-data-test="chat-model-popover"
    content-class="chat-model-popover-panel"
    width="auto"
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
      <div class="chat-model-popover-layout">
        <div ref="modelCard" class="chat-model-card" data-test="chat-model-card">
          <header class="kx-popover-panel__header chat-model-popover-header">
            {{ t("chat.model") }}
          </header>
          <ul class="kx-popover-list chat-model-list" @scroll="updateReasoningAnchor()">
            <li v-for="profile in props.modelOptions" :key="profile.alias">
              <button
                :ref="(el) => setModelOptionEl(profile.alias, el)"
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
                <span
                  v-if="
                    profile.context_window ||
                    profile.supports_vision ||
                    profile.supports_tools ||
                    profile.supports_reasoning
                  "
                  class="chat-model-badges"
                  data-test="chat-model-badges"
                >
                  <span
                    v-if="profile.context_window"
                    class="chat-model-badge chat-model-badge--context"
                    data-test="badge-context"
                    >{{ formatContextWindow(profile.context_window) }}</span
                  >
                  <span
                    v-if="profile.supports_vision"
                    class="chat-model-badge chat-model-badge--vision"
                    data-test="badge-vision"
                    >Vision</span
                  >
                  <span
                    v-if="profile.supports_tools"
                    class="chat-model-badge chat-model-badge--tools"
                    data-test="badge-tools"
                    >Tools</span
                  >
                  <span
                    v-if="profile.supports_reasoning"
                    class="chat-model-badge chat-model-badge--reasoning"
                    data-test="badge-reasoning"
                    >Reasoning</span
                  >
                </span>
              </button>
            </li>
          </ul>
        </div>
        <div
          ref="reasoningPanel"
          v-if="reasoningModel"
          class="chat-reasoning-panel chat-reasoning-panel--anchored"
          data-test="chat-reasoning-panel"
          :style="{ '--chat-reasoning-anchor-y': `${reasoningAnchorY}px` }"
        >
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
                  selected: activeReasoningEffort && effort === activeReasoningEffort,
                  'kx-popover-option--selected':
                    activeReasoningEffort && effort === activeReasoningEffort
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
:global(.chat-model-popover-panel) {
  --chat-model-card-width: 360px;
  --chat-reasoning-card-width: 216px;

  width: calc(var(--chat-model-card-width) + 10px + var(--chat-reasoning-card-width));
  max-width: calc(100vw - 24px);
  max-height: none;
  overflow: visible;
  border: 0;
  background: transparent;
  box-shadow: none;
  padding: 0;
}
.chat-model-popover-layout {
  position: relative;
  display: flex;
  min-width: 0;
  gap: 10px;
  align-items: flex-start;
}
.chat-model-card,
.chat-reasoning-panel {
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-lg);
  background: var(--app-elevated-color);
  color: var(--app-text-color);
  box-shadow: var(--app-overlay-shadow);
  padding: 12px;
}
.chat-model-card {
  width: var(--chat-model-card-width);
  min-width: 0;
}
.chat-model-list {
  max-height: min(420px, calc(100vh - 180px));
  overflow-y: auto;
  padding-right: 4px;
}
.chat-model-option {
  flex-direction: column;
  align-items: flex-start;
}
.chat-reasoning-panel {
  width: var(--chat-reasoning-card-width);
  min-width: var(--chat-reasoning-card-width);
}
.chat-reasoning-panel--anchored {
  position: absolute;
  left: calc(var(--chat-model-card-width) + 10px);
  top: var(--chat-reasoning-anchor-y, 52px);
  transform: translateY(-50%);
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
@media (max-width: 720px) {
  :global(.chat-model-popover-panel) {
    width: min(92vw, 360px);
  }
  .chat-model-popover-layout {
    flex-direction: column;
  }
  .chat-model-card,
  .chat-reasoning-panel {
    width: 100%;
    min-width: 0;
  }
  .chat-reasoning-panel--anchored {
    position: static;
    transform: none;
  }
}
@media (prefers-reduced-motion: no-preference) {
  :global(.chat-model-popover-panel) {
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
.chat-model-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 3px;
}
.chat-model-badge {
  display: inline-block;
  border-radius: 999px;
  padding: 1px 7px;
  font-size: 10px;
  font-weight: 500;
  line-height: 16px;
  white-space: nowrap;
}
.chat-model-badge--context {
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
  color: var(--app-primary-color);
}
.chat-model-badge--vision {
  background: color-mix(in srgb, #8b5cf6 12%, transparent);
  color: #8b5cf6;
}
.chat-model-badge--tools {
  background: color-mix(in srgb, #0891b2 12%, transparent);
  color: #0891b2;
}
.chat-model-badge--reasoning {
  background: color-mix(in srgb, #d97706 12%, transparent);
  color: #d97706;
}
</style>
