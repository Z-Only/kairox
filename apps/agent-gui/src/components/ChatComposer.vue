<script setup lang="ts">
import { useSessionStore } from "@/stores/session";
import { useSkillsStore } from "@/stores/skills";
import { useNotifications } from "@/composables/useNotifications";
import { useChatComposer } from "@/composables/useChatComposer";
import type { ProfileInfo } from "@/types";
import type { CommandDef } from "@/composables/useCommandRegistry";
import CommandPalette from "@/components/CommandPalette.vue";
import FileMentionPalette from "@/components/FileMentionPalette.vue";
import AttachmentTray from "@/components/AttachmentTray.vue";
import ChatModelSelector from "@/components/ChatModelSelector.vue";
import ChatPermissionSelector from "@/components/ChatPermissionSelector.vue";

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

function onSelectCommand(cmd: CommandDef) {
  composer.onSelectCommand(cmd);
}

async function handleModelSelect(alias: string, reasoningEffort?: string) {
  await composer.selectModelProfile(alias, modelPopoverOpen, reasoningEffort);
}

function onSelectModelProfile(alias: string) {
  void handleModelSelect(alias);
}

function onSelectSkill(skillId: string) {
  void skillsStore.activateSkill(skillId);
  closePalettes();
}

async function handlePermissionSelect(mode: string) {
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
      <ChatModelSelector
        v-model:open="modelPopoverOpen"
        :model-options="modelOptions"
        :current-profile="session.currentProfile"
        :switching-model="switchingModel"
        :active-profile-display="session.activeProfileDisplay"
        :current-reasoning-effort="session.currentReasoningEffort"
        @select-model="handleModelSelect"
      />
      <ChatPermissionSelector
        v-model:open="permissionPopoverOpen"
        :permission-mode="session.permissionMode"
        @select-permission="handlePermissionSelect"
      />
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
