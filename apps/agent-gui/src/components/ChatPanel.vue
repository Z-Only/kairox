<script setup lang="ts">
import { useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import { renderMarkdown } from "../utils/markdown";
import type { ProjectedRole } from "../types";
import ChatComposer from "@/components/ChatComposer.vue";

const { t } = useI18n();
const router = useRouter();
const session = useSessionStore();
const projectStore = useProjectStore();
const scrollbar = ref<HTMLElement | null>(null);
const worktreeBranchInput = ref("");
const worktreeBranchFormOpen = ref(false);

/** Map role to CSS class suffix. */
const roleClass: Record<ProjectedRole, string> = {
  user: "user",
  assistant: "assistant",
  planner: "planner",
  worker: "worker",
  reviewer: "reviewer",
  system: "system"
};

const currentSession = computed(() => session.currentSessionInfo);
const currentProjectId = computed(() => currentSession.value?.project_id ?? null);
const currentProject = computed(() => {
  const projectId = currentProjectId.value;
  if (!projectId) return null;
  return projectStore.projects.find((p) => p.projectId === projectId) ?? null;
});
const currentProjectName = computed(
  () => currentProject.value?.displayName ?? currentProjectId.value ?? ""
);

function isWorktreeSession(sessionInfo: typeof currentSession.value): boolean {
  if (!sessionInfo?.worktree_path) return false;
  const projectRoot = currentProject.value?.rootPath;
  if (projectRoot) return sessionInfo.worktree_path !== projectRoot;
  return sessionInfo.worktree_path.includes("/.worktrees/") || Boolean(sessionInfo.branch);
}

const sessionGitMeta = computed(() => {
  const sessionInfo = currentSession.value;
  if (!sessionInfo?.project_id && !sessionInfo?.worktree_path) return [];

  const gitMetaParts = [];
  if (isWorktreeSession(sessionInfo) && sessionInfo.branch) gitMetaParts.push("worktree");
  if (sessionInfo.branch) gitMetaParts.push(sessionInfo.branch);
  else if (sessionInfo.worktree_path) gitMetaParts.push(sessionInfo.worktree_path);
  if (!gitMetaParts.length && sessionInfo.project_id) gitMetaParts.push(sessionInfo.project_id);
  return gitMetaParts;
});

const workspacePath = computed(() => {
  const sessionInfo = currentSession.value;
  if (sessionInfo?.worktree_path) return sessionInfo.worktree_path;
  const projectId = currentProjectId.value;
  if (!projectId) return "";
  const project = projectStore.projects.find((p) => p.projectId === projectId);
  return project?.rootPath ?? "";
});
const isEmptyProjectChat = computed(
  () =>
    Boolean(currentProjectId.value) &&
    session.projection.messages.length === 0 &&
    !session.projection.token_stream
);
const projectInstructionSummaryText = computed(() => {
  const projectId = currentProjectId.value;
  if (!projectId || !isEmptyProjectChat.value) return null;

  const instructionSummary = projectStore.instructionSummariesByProject.get(projectId);
  const sourceFileNames =
    instructionSummary?.sourcePaths
      .map((sourcePath) => sourcePath.split(/[\\/]/).filter(Boolean).at(-1))
      .filter((fileName): fileName is string => Boolean(fileName)) ?? [];
  if (!sourceFileNames.length) return null;

  return `Loaded ${sourceFileNames.join(", ")}`;
});

function startProjectWorktreeSession() {
  worktreeBranchFormOpen.value = true;
  worktreeBranchInput.value = "";
}

function cancelProjectWorktreeSession() {
  worktreeBranchFormOpen.value = false;
  worktreeBranchInput.value = "";
}

async function confirmProjectWorktreeSession() {
  const projectId = currentProjectId.value;
  const branchName = worktreeBranchInput.value.trim();
  if (!projectId || !branchName) return;

  try {
    const projectSession = await projectStore.createProjectWorktreeSession(projectId, branchName);
    await session.switchProjectSession(projectSession);
    await router.push({ name: "workbench", params: { sessionId: projectSession.sessionId } });
  } catch (error) {
    console.error("Failed to start project worktree session:", error);
  } finally {
    cancelProjectWorktreeSession();
  }
}

watch(
  () => currentProjectId.value,
  async (projectId) => {
    if (!projectId || projectStore.instructionSummariesByProject.has(projectId)) return;
    await projectStore.getProjectInstructionSummary(projectId);
  },
  { immediate: true }
);

watch(
  () => [session.projection.messages.length, session.projection.token_stream],
  async () => {
    await nextTick();
    if (scrollbar.value) {
      scrollbar.value.scrollTo({ top: scrollbar.value.scrollHeight, behavior: "smooth" });
    }
  }
);
</script>

<template>
  <section class="chat-panel" data-test="chat-panel">
    <header class="chat-header">
      <h2>{{ t("chat.header") }}</h2>
      <div v-if="currentProjectId" class="chat-header-actions">
        <form
          v-if="worktreeBranchFormOpen"
          class="project-worktree-form"
          data-test="project-worktree-branch-form"
          @submit.prevent="confirmProjectWorktreeSession"
        >
          <KxInput
            v-model="worktreeBranchInput"
            class="project-worktree-input"
            :placeholder="t('sessions.worktreeBranchPlaceholder')"
            data-test="project-worktree-branch-input"
            size="compact"
            @keydown.escape="cancelProjectWorktreeSession"
          />
          <KxTooltip :text="t('common.confirm')">
            <KxIconButton
              :label="t('common.confirm')"
              data-test="project-worktree-branch-confirm"
              size="sm"
              @click="confirmProjectWorktreeSession"
            >
              <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
              </svg>
            </KxIconButton>
          </KxTooltip>
        </form>
        <KxTooltip :text="t('sessions.newWorktreeSessionInProject', { name: currentProjectName })">
          <KxIconButton
            :label="t('sessions.newWorktreeSessionInProject', { name: currentProjectName })"
            data-test="project-worktree-session-trigger"
            size="sm"
            @click="startProjectWorktreeSession"
          >
            <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
              <path
                d="M3 4.5A1.5 1.5 0 0 1 4.5 3h5.75v1.5H4.5v11h11v-5.75H17v7.25H3V4.5Zm6.25 9.75h1.5V11H14V9.5h-3.25V6.25h-1.5V9.5H6V11h3.25v3.25Z"
              />
            </svg>
          </KxIconButton>
        </KxTooltip>
      </div>
    </header>

    <div ref="scrollbar" class="message-list" data-test="message-list">
      <div class="message-list-inner">
        <KxEmptyState
          v-if="session.projection.messages.length === 0 && !session.projection.token_stream"
          class="chat-empty-state"
          data-test="chat-empty-state"
        >
          <template #icon>
            <svg
              width="40"
              height="40"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              aria-hidden="true"
            >
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
          </template>
          {{ t("chat.emptyState") }}
        </KxEmptyState>
        <div
          v-for="(msg, i) in session.projection.messages"
          :key="i"
          :class="['message', `message-${roleClass[msg.role] || 'assistant'}`]"
          data-test="chat-message"
          :data-role="roleClass[msg.role] || 'assistant'"
          :data-error="msg.content.startsWith('[error]') ? 'true' : undefined"
        >
          <!-- eslint-disable vue/no-v-html -->
          <span
            v-if="
              msg.role === 'assistant' ||
              msg.role === 'planner' ||
              msg.role === 'worker' ||
              msg.role === 'reviewer'
            "
            class="message-content markdown-body"
            :data-test="msg.content.startsWith('[error]') ? 'error-banner' : undefined"
            v-html="renderMarkdown(msg.content)"
          ></span>
          <!-- eslint-enable vue/no-v-html -->
          <span v-else class="message-content">{{ msg.content }}</span>
        </div>
        <div
          v-if="projectInstructionSummaryText"
          class="project-instruction-summary"
          data-test="project-instruction-summary"
        >
          {{ projectInstructionSummaryText }}
        </div>
        <div
          v-if="session.projection.token_stream"
          class="message message-assistant streaming"
          data-test="stream-indicator"
        >
          <span class="message-content"
            >{{ session.projection.token_stream }}<span class="cursor">▌</span></span
          >
        </div>
        <KxBadge
          v-if="session.projection.cancelled"
          class="cancelled-marker"
          tone="warning"
          data-test="cancelled-marker"
        >
          {{ t("chat.cancelled") }}
        </KxBadge>
      </div>
    </div>

    <div
      v-if="session.lastSendError"
      class="send-error-banner"
      data-test="error-banner"
      role="alert"
    >
      {{ session.lastSendError }}
    </div>

    <ChatComposer :workspace-path="workspacePath" :session-git-meta="sessionGitMeta" />
  </section>
</template>

<style scoped>
.chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
}
.chat-header h2 {
  margin: 0;
  font-size: 14px;
}
.chat-header-actions {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
}
.project-worktree-form {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 4px;
}
.project-worktree-input {
  width: min(32vw, 180px);
  min-width: 96px;
}
.chat-header-actions svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.message-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.message-list-inner {
  padding: 12px 16px;
}
.message {
  display: flex;
  margin-bottom: 12px;
  line-height: 1.5;
}
.project-instruction-summary {
  margin-bottom: 12px;
  color: var(--app-muted-text-color, var(--app-text-color));
  font-size: 12px;
  line-height: 1.5;
}
.chat-empty-state {
  margin: 28px 0;
}
.message-content {
  max-width: min(760px, 82%);
  border-radius: var(--app-radius-xl);
  padding: 10px 12px;
  white-space: pre-wrap;
  overflow-wrap: break-word;
}
.message-user {
  justify-content: flex-end;
}
.message-user .message-content {
  color: var(--app-primary-contrast, #ffffff);
  background: var(--app-primary-color, #0077cc);
}
.message-assistant,
.message-planner,
.message-worker,
.message-reviewer,
.message-system {
  justify-content: flex-start;
}
.message-assistant .message-content,
.message-planner .message-content,
.message-worker .message-content,
.message-reviewer .message-content,
.message-system .message-content {
  color: var(--app-muted-text-color, var(--app-text-color));
  background: var(--app-muted-surface-color, var(--app-card-color));
}
.message-system .message-content {
  opacity: 0.72;
  font-style: italic;
}
.streaming .cursor {
  animation: blink 1s step-end infinite;
}
.cancelled-marker {
  margin-top: 4px;
}
@keyframes blink {
  50% {
    opacity: 0;
  }
}
.send-error-banner {
  margin: 8px 16px 0;
  padding: 8px 10px;
  border: 1px solid var(--app-error-color, #d03050);
  border-radius: 4px;
  background: color-mix(in srgb, var(--app-error-color, #d03050) 10%, transparent);
  color: var(--app-error-color, #d03050);
  font-size: 13px;
}
.markdown-body :deep(pre.hljs) {
  margin: 8px 0;
  border-radius: 6px;
  padding: 12px;
  overflow-x: auto;
  font-size: 13px;
  line-height: 1.5;
}
.markdown-body :deep(code) {
  font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
}
.markdown-body :deep(:not(pre) > code) {
  background: var(--app-card-color);
  padding: 2px 4px;
  border-radius: 3px;
  font-size: 12px;
}
.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  padding-left: 20px;
}
.markdown-body :deep(p) {
  margin: 6px 0;
}
</style>
