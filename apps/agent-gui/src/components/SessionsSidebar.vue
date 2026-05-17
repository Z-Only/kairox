<script setup lang="ts">
import ProjectSection from "@/components/sidebar/ProjectSection.vue";
import SessionSection from "@/components/sidebar/SessionSection.vue";
import { useSidebarActions } from "@/composables/sidebar/useSidebarActions";
import { useSidebarRename } from "@/composables/sidebar/useSidebarRename";
import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";
import { useWorkspaceUiStore, type SidebarSection } from "@/stores/workspaceUi";

const { t } = useI18n();

const session = useSessionStore();
const projects = useProjectStore();
const workspaceUi = useWorkspaceUiStore();
const sidebarActions = useSidebarActions();

const {
  activeSessionId,
  projectCreateMenuOpen,
  pendingDeleteSessionId,
  pendingDeleteProjectId,
  pendingArchiveProjectSessionId,
  importingProject,
  resetDeleteConfirmation,
  switchToSession,
  createSession,
  requestDeleteSession,
  getProjectSessions,
  switchToProjectSession,
  createProjectSession,
  worktreeBranchInput,
  worktreeBranchProjectId,
  startWorktreeSession,
  cancelWorktreeSession,
  confirmWorktreeSession,
  createBlankProject,
  importExistingProject,
  requestArchiveProjectSession,
  toggleProjectExpanded,
  requestDeleteProject,
  loadProjectsForSidebar
} = sidebarActions;

const sessionRename = useSidebarRename({
  onStart: resetDeleteConfirmation,
  onConfirm: session.renameSession
});
const projectRename = useSidebarRename({
  onStart: resetDeleteConfirmation,
  onConfirm: projects.renameProject
});
const projectSessionRename = useSidebarRename({
  onStart: resetDeleteConfirmation,
  onConfirm: projects.renameProjectSession
});

const orderedSidebarSections = computed<SidebarSection[]>(() => {
  const configuredSections = workspaceUi.sectionOrder.filter(
    (section, index, sections) => sections.indexOf(section) === index
  );
  const requiredSections: SidebarSection[] = ["projects", "sessions"];
  return [
    ...configuredSections,
    ...requiredSections.filter((section) => !configuredSections.includes(section))
  ];
});

onMounted(() => {
  void loadProjectsForSidebar();
});
</script>

<template>
  <aside class="sessions-sidebar" data-test="sessions-sidebar" :aria-label="t('sessions.header')">
    <div class="session-scroll">
      <div
        v-if="projects.activeProjects.length === 0 && session.sessions.length === 0"
        class="sessions-empty-state"
      >
        <p class="sessions-empty-state-text">
          {{ t("sessions.emptyState") }}
        </p>
      </div>
      <template v-for="sectionName in orderedSidebarSections" :key="sectionName">
        <ProjectSection
          v-if="sectionName === 'projects'"
          v-model:project-create-menu-open="projectCreateMenuOpen"
          :active-projects="projects.activeProjects"
          :archived-sessions="projects.archivedSessions"
          :active-session-id="activeSessionId"
          :pending-delete-project-id="pendingDeleteProjectId"
          :pending-archive-project-session-id="pendingArchiveProjectSessionId"
          :importing-project="importingProject"
          :project-rename="projectRename"
          :project-session-rename="projectSessionRename"
          :get-project-sessions="getProjectSessions"
          :create-blank-project="createBlankProject"
          :import-existing-project="importExistingProject"
          :toggle-project-expanded="toggleProjectExpanded"
          :create-project-session="createProjectSession"
          :request-delete-project="requestDeleteProject"
          :switch-to-project-session="switchToProjectSession"
          :request-archive-project-session="requestArchiveProjectSession"
          :archive-open="workspaceUi.archiveOpen"
          v-model:worktree-branch-input="worktreeBranchInput"
          :worktree-branch-project-id="worktreeBranchProjectId"
          :start-worktree-session="startWorktreeSession"
          :cancel-worktree-session="cancelWorktreeSession"
          :confirm-worktree-session="confirmWorktreeSession"
        />

        <SessionSection
          v-else
          :sessions="session.sessions"
          :active-session-id="activeSessionId"
          :pending-delete-session-id="pendingDeleteSessionId"
          :rename="sessionRename"
          :create-session="createSession"
          :switch-to-session="switchToSession"
          :request-delete-session="requestDeleteSession"
        />
      </template>
    </div>
  </aside>
</template>

<style>
.sessions-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.sessions-sidebar .session-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.sessions-sidebar .sidebar-section {
  border-bottom: 1px solid var(--app-border-color);
}
.sessions-sidebar .section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 12px 4px;
}
.sessions-sidebar .section-heading h3 {
  margin: 0;
  color: var(--app-text-color);
  font-size: 12px;
  font-weight: 600;
}
.sessions-sidebar .section-actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: 4px;
}
.sessions-sidebar .project-create-menu-item {
  width: 100%;
  border: none;
  font-family: inherit;
  text-align: left;
}
.sessions-sidebar .section-action-btn,
.sessions-sidebar .project-action-btn,
.sessions-sidebar .project-expand-btn,
.sessions-sidebar .project-title-btn {
  border: none;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font-family: inherit;
}
.sessions-sidebar .section-action-btn,
.sessions-sidebar .project-action-btn {
  min-height: 28px;
  padding: 4px 8px;
  border-radius: 4px;
  color: var(--app-text-color-2);
  font-size: 12px;
}
.sessions-sidebar .section-action-btn:hover,
.sessions-sidebar .project-action-btn:hover,
.sessions-sidebar .project-expand-btn:hover,
.sessions-sidebar .project-title-btn:hover,
.sessions-sidebar .project-session-item:hover {
  background: var(--app-hover-color);
}
.sessions-sidebar .section-action-btn:focus-visible,
.sessions-sidebar .project-action-btn:focus-visible,
.sessions-sidebar .project-expand-btn:focus-visible,
.sessions-sidebar .project-title-btn:focus-visible,
.sessions-sidebar .project-session-item:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
.sessions-sidebar .project-list,
.sessions-sidebar .project-session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.sessions-sidebar .project-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 40px;
  padding: 2px 8px;
}
.sessions-sidebar .project-expand-btn {
  flex-shrink: 0;
  width: 28px;
  min-height: 28px;
  border-radius: 4px;
  font-size: 13px;
}
.sessions-sidebar .project-title-btn {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 6px 4px;
  border-radius: 4px;
  text-align: left;
}
.sessions-sidebar .project-name,
.sessions-sidebar .project-path {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sessions-sidebar .project-name {
  font-size: 13px;
  font-weight: 600;
}
.sessions-sidebar .project-path {
  color: var(--app-text-color-3);
  font-size: 11px;
}
.sessions-sidebar .project-actions {
  flex: none;
  gap: 2px;
}
.sessions-sidebar .project-session-list {
  padding: 0 8px 4px 40px;
}
.sessions-sidebar .archived-session-list {
  padding: 0 8px 8px 16px;
}
.sessions-sidebar .project-session-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}
.sessions-sidebar .project-session-item {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
  min-height: 32px;
  padding: 6px 8px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font-family: inherit;
  font-size: 13px;
  text-align: left;
}
.sessions-sidebar .project-session-item.active {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  font-weight: 600;
}
.sessions-sidebar .project-session-branch {
  flex: none;
  max-width: 72px;
  color: var(--app-text-color-3);
  font-size: 11px;
}
.sessions-sidebar .project-session-actions {
  flex: none;
  gap: 2px;
}
.sessions-sidebar .archived-indicator {
  color: var(--app-text-color-3);
}
.sessions-sidebar .empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  min-height: 80px;
}
.sessions-sidebar .session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.sessions-sidebar .session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
  position: relative;
}
.sessions-sidebar .session-item:hover {
  background: var(--app-hover-color);
}
.sessions-sidebar .session-item.active {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  font-weight: 600;
}
.sessions-sidebar .session-indicator {
  color: var(--app-success-color);
  font-size: 10px;
  flex-shrink: 0;
}
.sessions-sidebar .session-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sessions-sidebar .session-actions {
  flex: none;
  gap: 4px;
}
.sessions-sidebar .row-actions {
  display: flex;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease;
}
.sessions-sidebar .row-actions .kx-icon-button {
  color: var(--app-text-color) !important;
  background: transparent !important;
  border: none !important;
}
.sessions-sidebar .row-actions .kx-icon-button svg {
  fill: currentColor !important;
}
html.dark .sessions-sidebar .row-actions .kx-icon-button {
  color: var(--app-text-color) !important;
  background: transparent !important;
  border: none !important;
}
html.dark .sessions-sidebar .row-actions .kx-icon-button svg {
  fill: currentColor !important;
}
.sessions-sidebar .session-item:hover .row-actions,
.sessions-sidebar .session-item:focus-within .row-actions,
.sessions-sidebar .project-row:hover .row-actions,
.sessions-sidebar .project-row:focus-within .row-actions,
.sessions-sidebar .project-session-row:hover .row-actions,
.sessions-sidebar .project-session-row:focus-within .row-actions {
  opacity: 1;
  pointer-events: auto;
}
.sessions-sidebar .icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  min-width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color-2);
  line-height: 1;
}
.sessions-sidebar .icon-btn:hover {
  background: var(--app-hover-color);
  color: var(--app-text-color);
}
.sessions-sidebar .icon-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
.sessions-sidebar .icon-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.sessions-sidebar .icon-btn svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.sessions-sidebar .action-delete:hover {
  background: color-mix(in srgb, var(--app-error-color) 10%, transparent);
}
.sessions-sidebar .rename-input {
  flex: 1;
  min-width: 0;
  border: 1px solid var(--app-primary-color);
  border-radius: 3px;
  padding: 2px 4px;
  font-size: 13px;
  outline: none;
  font-family: inherit;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.sessions-sidebar .project-rename-input,
.sessions-sidebar .project-session-rename-input {
  min-height: 28px;
}
.sessions-sidebar .empty-hint {
  padding: 12px;
  color: var(--app-text-color-3);
  font-size: 13px;
}

.sessions-sidebar .confirm-action {
  color: var(--app-error-color, #d03050) !important;
}
.sessions-sidebar .confirm-action:hover {
  background: color-mix(in srgb, var(--app-error-color, #d03050) 16%, transparent) !important;
}

@media (prefers-reduced-motion: no-preference) {
  .sessions-sidebar .project-expand-btn,
  .sessions-sidebar .project-title-btn,
  .sessions-sidebar .session-item {
    transition:
      background 0.15s,
      color 0.15s;
  }
}
.sessions-sidebar .sessions-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px 12px;
}
.sessions-sidebar .sessions-empty-state-text {
  color: var(--app-text-color-3);
  font-size: var(--app-text-sm);
  text-align: center;
}
</style>
