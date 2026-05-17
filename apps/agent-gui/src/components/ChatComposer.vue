<script setup lang="ts">
import { formatProfileDisplay, useSessionStore } from "@/stores/session";
import { useSkillsStore } from "@/stores/skills";
import { useNotifications } from "@/composables/useNotifications";
import { useChatComposer } from "@/composables/useChatComposer";
import type { ProfileInfo } from "@/types";
import type { CommandDef } from "@/composables/useCommandRegistry";
import CommandPalette from "@/components/CommandPalette.vue";
import FileMentionPalette from "@/components/FileMentionPalette.vue";
import AttachmentTray from "@/components/AttachmentTray.vue";

const props = defineProps<{
  workspacePath: string;
  sessionGitMeta: string[];
}>();

const { t } = useI18n();
const session = useSessionStore();
const skillsStore = useSkillsStore();
const { notify } = useNotifications();
const modelPopoverOpen = ref(false);
const permissionPopoverOpen = ref(false);
const commandPaletteRef = ref<InstanceType<typeof CommandPalette> | null>(null);
const fileMentionPaletteRef = ref<InstanceType<typeof FileMentionPalette> | null>(null);

const composer = useChatComposer({ session, notify, t });
const {
  inputText,
  showCommandPalette,
  showMentionPalette,
  paletteFilter,
  attachments,
  switchingModel,
  sendDisabled,
  handleInput,
  closePalettes,
  onSelectFile,
  pickFiles,
  removeAttachment,
  sendMessage,
  cancelSession
} = composer;

const modelOptions = computed<ProfileInfo[]>(() =>
  [...session.profileInfos].sort((a, b) => a.alias.localeCompare(b.alias))
);

function getModelOptionDisplay(profile: ProfileInfo): string {
  return formatProfileDisplay(profile);
}

function onSelectCommand(cmd: CommandDef) {
  composer.onSelectCommand(cmd);
}

async function selectModelProfile(alias: string) {
  await composer.selectModelProfile(alias, modelPopoverOpen);
}

function onSelectSkill(skillId: string) {
  void skillsStore.activateSkill(skillId);
  closePalettes();
}

function onSelectModelProfile(alias: string) {
  void selectModelProfile(alias);
}

const permissionOptions = [
  { value: "read_only", label: "Read Only" },
  { value: "suggest", label: "Suggest" },
  { value: "agent", label: "Agent" },
  { value: "autonomous", label: "Autonomous" },
  { value: "interactive", label: "Interactive" }
];

const permissionDisplay = computed(() => {
  const opt = permissionOptions.find((o) => o.value === session.permissionMode);
  return opt ? opt.label : session.permissionMode;
});

async function selectPermissionMode(mode: string) {
  await session.setPermissionMode(mode);
  permissionPopoverOpen.value = false;
}

function handleKeydown(e: KeyboardEvent) {
  if (showCommandPalette.value && commandPaletteRef.value) {
    if (["ArrowDown", "ArrowUp", "Enter", "Escape"].includes(e.key)) {
      commandPaletteRef.value.handleKeydown(e);
      return;
    }
  }
  if (showMentionPalette.value && fileMentionPaletteRef.value) {
    if (["ArrowDown", "ArrowUp", "Enter", "Escape"].includes(e.key)) {
      fileMentionPaletteRef.value.handleKeydown(e);
      return;
    }
  }

  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    if (showCommandPalette.value || showMentionPalette.value) {
      return;
    }
    void sendMessage();
  }
}

onMounted(() => {
  void session.loadProfileInfo();
});
</script>

<template>
  <div class="input-area">
    <div class="palette-container">
      <CommandPalette
        ref="commandPaletteRef"
        :visible="showCommandPalette"
        :filter-text="paletteFilter"
        @select-command="onSelectCommand"
        @select-skill="onSelectSkill"
        @select-model-profile="onSelectModelProfile"
        @close="closePalettes"
      />
      <FileMentionPalette
        ref="fileMentionPaletteRef"
        :visible="showMentionPalette"
        :filter-text="paletteFilter"
        :workspace-path="props.workspacePath"
        @select-file="(path: string) => onSelectFile(path, props.workspacePath)"
        @close="closePalettes"
      />
    </div>
    <div class="composer-meta">
      <KxPopover
        v-model:open="modelPopoverOpen"
        content-data-test="chat-model-popover"
        side="top"
        align="start"
      >
        <template #trigger>
          <button
            class="chat-model-trigger"
            type="button"
            data-test="chat-model-trigger"
            :aria-label="t('chat.selectModelAria', { model: session.activeProfileDisplay })"
          >
            {{ session.activeProfileDisplay }}
          </button>
        </template>
        <template #content>
          <div class="chat-model-popover-panel">
            <header class="chat-model-popover-header">{{ t("chat.model") }}</header>
            <ul class="chat-model-list">
              <li v-for="profile in modelOptions" :key="profile.alias">
                <button
                  type="button"
                  :class="[
                    'chat-model-option',
                    { selected: profile.alias === session.currentProfile }
                  ]"
                  :data-test="`chat-model-option-${profile.alias}`"
                  :aria-current="profile.alias === session.currentProfile ? 'true' : undefined"
                  :disabled="switchingModel"
                  @click="selectModelProfile(profile.alias)"
                >
                  <span class="chat-model-option-label">
                    {{ getModelOptionDisplay(profile) }}
                  </span>
                  <span class="chat-model-option-meta">
                    {{ profile.alias }}
                    <span v-if="profile.alias === session.currentProfile">
                      · {{ t("chat.currentModel") }}</span
                    >
                  </span>
                </button>
              </li>
            </ul>
          </div>
        </template>
      </KxPopover>
      <KxPopover
        v-model:open="permissionPopoverOpen"
        content-data-test="chat-permission-popover"
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
          <div class="chat-permission-popover-panel">
            <header class="chat-permission-popover-header">{{ t("chat.permission") }}</header>
            <ul class="chat-permission-list">
              <li v-for="option in permissionOptions" :key="option.value">
                <button
                  type="button"
                  :class="[
                    'chat-permission-option',
                    { selected: option.value === session.permissionMode }
                  ]"
                  :data-test="`chat-permission-option-${option.value}`"
                  :aria-current="option.value === session.permissionMode ? 'true' : undefined"
                  @click="selectPermissionMode(option.value)"
                >
                  <span class="chat-permission-option-label">
                    {{ option.label }}
                  </span>
                </button>
              </li>
            </ul>
          </div>
        </template>
      </KxPopover>
      <span v-if="props.sessionGitMeta.length" class="git-meta" data-test="session-git-meta">
        {{ props.sessionGitMeta.join(" · ") }}
      </span>
    </div>
    <AttachmentTray
      :attachments="attachments"
      :disabled="session.isStreaming"
      @pick-files="pickFiles"
      @remove-attachment="removeAttachment"
    />
    <div class="input-row">
      <button
        v-if="attachments.length === 0"
        class="attach-btn"
        type="button"
        data-test="attach-file-btn"
        :aria-label="t('chat.attachFileAria')"
        :disabled="session.isStreaming"
        @click="pickFiles"
      >
        +
      </button>
      <textarea
        v-model="inputText"
        class="message-input"
        data-test="message-input"
        :disabled="session.isStreaming"
        rows="1"
        :placeholder="t('chat.placeholder')"
        @keydown="handleKeydown"
        @input="handleInput"
      />
      <ContextMeter variant="ring" />
      <button
        v-if="session.isStreaming"
        class="btn btn-error"
        data-test="cancel-button"
        @click="cancelSession"
      >
        {{ t("common.cancel") }}
      </button>
      <button
        v-else
        class="btn btn-primary"
        data-test="send-button"
        :disabled="sendDisabled"
        @click="sendMessage"
      >
        {{ t("common.send") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.input-area {
  position: relative;
  padding: 8px 16px;
  border-top: 1px solid var(--app-border-color, #d7d7d7);
}
.palette-container {
  position: relative;
}
.composer-meta {
  display: flex;
  min-width: 0;
  overflow: hidden;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  margin-bottom: 6px;
  color: var(--app-muted-text-color, var(--app-text-color));
  font-size: 12px;
}
.git-meta {
  min-width: 0;
  max-width: min(100%, 420px);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.72;
}
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
  min-width: 240px;
}
.chat-model-popover-header {
  margin-bottom: 8px;
  color: var(--app-text-color-2, var(--app-muted-text-color));
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.chat-model-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 0;
  margin: 0;
  list-style: none;
}
.chat-model-option {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font: inherit;
  text-align: left;
}
.chat-model-option:hover:not(:disabled) {
  border-color: var(--app-border-color);
  background: var(--app-hover-color, color-mix(in srgb, var(--app-primary-color) 8%, transparent));
}
.chat-model-option.selected {
  border-color: color-mix(in srgb, var(--app-primary-color) 45%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
}
.chat-model-option:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.chat-model-option-label {
  max-width: 100%;
  overflow: hidden;
  font-size: 13px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chat-model-option-meta {
  color: var(--app-text-color-3, var(--app-muted-text-color));
  font-size: 11px;
}
@media (prefers-reduced-motion: no-preference) {
  .chat-model-popover-panel {
    animation: popover-in 0.15s ease;
  }
}
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
.chat-permission-popover-header {
  margin-bottom: 8px;
  color: var(--app-text-color-2, var(--app-muted-text-color));
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.chat-permission-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 0;
  margin: 0;
  list-style: none;
}
.chat-permission-option {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font: inherit;
  text-align: left;
}
.chat-permission-option:hover {
  border-color: var(--app-border-color);
  background: var(--app-hover-color, color-mix(in srgb, var(--app-primary-color) 8%, transparent));
}
.chat-permission-option.selected {
  border-color: color-mix(in srgb, var(--app-primary-color) 45%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
}
.chat-permission-option-label {
  max-width: 100%;
  overflow: hidden;
  font-size: 13px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
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
.input-row {
  display: flex;
  gap: 8px;
  align-items: flex-end;
}
.message-input {
  flex: 1;
  min-width: 0;
  resize: vertical;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 4px;
  padding: 6px 10px;
  font-size: 13px;
  font-family: inherit;
  background: var(--app-card-color);
  color: var(--app-text-color);
  outline: none;
  width: 100%;
  box-sizing: border-box;
}
.message-input:focus {
  border-color: var(--app-primary-color);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--app-primary-color) 25%, transparent);
}
.message-input:disabled {
  opacity: 0.5;
}
.btn {
  padding: 6px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-primary {
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-primary-color);
}
.btn-error {
  background: var(--app-error-color, #d03050);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-error-color, #d03050);
}
.attach-btn {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border: 1px dashed var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: transparent;
  color: var(--app-muted-text-color, var(--app-text-color));
  cursor: pointer;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  line-height: 1;
}
.attach-btn:hover:not(:disabled) {
  border-color: var(--app-primary-color);
  color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
.attach-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
