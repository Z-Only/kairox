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

const sessionSearch = ref("");
const normalizedSessionSearch = computed(() => sessionSearch.value.trim().toLocaleLowerCase());

function searchableText(parts: Array<string | null | undefined>): string {
  return parts.filter(Boolean).join(" ").toLocaleLowerCase();
}

function matchesSessionSearch(parts: Array<string | null | undefined>): boolean {
  const query = normalizedSessionSearch.value;
  if (!query) return true;
  return searchableText(parts).includes(query);
}

function clearSessionSearch() {
  sessionSearch.value = "";
}

function projectMatchesSearch(project: (typeof projects.sidebarProjects)[number]): boolean {
  return matchesSessionSearch([project.displayName, project.rootPath]);
}

function projectSessionMatchesSearch(
  projectSession: ReturnType<typeof getProjectSessions>[number]
): boolean {
  return matchesSessionSearch([
    projectSession.title,
    projectSession.profile,
    projectSession.branch,
    projectSession.worktreePath
  ]);
}

const filteredSessions = computed(() =>
  session.sessions.filter((item) => matchesSessionSearch([item.title, item.profile]))
);

const filteredActiveProjects = computed(() => {
  if (!normalizedSessionSearch.value) return projects.sidebarProjects;

  return projects.sidebarProjects.filter(
    (project) =>
      projectMatchesSearch(project) ||
      getProjectSessions(project.projectId).some(projectSessionMatchesSearch)
  );
});

const filteredArchivedSessions = computed(() =>
  projects.archivedSessions.filter(projectSessionMatchesSearch)
);

function getFilteredProjectSessions(projectId: string) {
  const projectSessions = getProjectSessions(projectId);
  if (!normalizedSessionSearch.value) return projectSessions;

  const project = projects.sidebarProjects.find((entry) => entry.projectId === projectId);
  if (project && projectMatchesSearch(project)) return projectSessions;

  return projectSessions.filter(projectSessionMatchesSearch);
}

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
    <div class="session-search">
      <div class="session-search-row">
        <KxInput
          v-model="sessionSearch"
          type="search"
          size="compact"
          data-test="session-search-input"
          :placeholder="t('sessions.searchPlaceholder')"
          :aria-label="t('sessions.searchPlaceholder')"
        />
        <KxIconButton
          v-if="sessionSearch"
          class="session-search-clear"
          label="Clear session search"
          title="Clear session search"
          data-test="session-search-clear"
          size="sm"
          @click="clearSessionSearch"
        >
          <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
            <path
              d="M5.22 4.16 10 8.94l4.78-4.78 1.06 1.06L11.06 10l4.78 4.78-1.06 1.06L10 11.06l-4.78 4.78-1.06-1.06L8.94 10 4.16 5.22l1.06-1.06Z"
            />
          </svg>
        </KxIconButton>
      </div>
    </div>
    <div class="session-scroll">
      <KxEmptyState
        v-if="
          projects.sidebarProjects.length === 0 &&
          projects.missingProjects.length === 0 &&
          session.sessions.length === 0
        "
        class="sessions-empty-state"
        density="inline"
        data-test="sessions-root-empty"
      >
        {{ t("sessions.emptyState") }}
      </KxEmptyState>
      <template v-for="sectionName in orderedSidebarSections" :key="sectionName">
        <ProjectSection
          v-if="sectionName === 'projects'"
          v-model:project-create-menu-open="projectCreateMenuOpen"
          :active-projects="filteredActiveProjects"
          :missing-projects="projects.missingProjects"
          :archived-sessions="filteredArchivedSessions"
          :active-session-id="activeSessionId"
          :pending-delete-project-id="pendingDeleteProjectId"
          :pending-archive-project-session-id="pendingArchiveProjectSessionId"
          :importing-project="importingProject"
          :project-rename="projectRename"
          :project-session-rename="projectSessionRename"
          :get-project-sessions="getFilteredProjectSessions"
          :create-blank-project="createBlankProject"
          :import-existing-project="importExistingProject"
          :remove-missing-projects="projects.removeMissingProjects"
          :toggle-project-expanded="toggleProjectExpanded"
          :create-project-session="createProjectSession"
          :request-delete-project="requestDeleteProject"
          :switch-to-project-session="switchToProjectSession"
          :request-archive-project-session="requestArchiveProjectSession"
          :archive-open="workspaceUi.archiveOpen"
        />

        <SessionSection
          v-else
          :sessions="filteredSessions"
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
  background: var(--app-panel-color);
}
.sessions-sidebar .session-search {
  flex: none;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-panel-color);
}
.sessions-sidebar .session-search-row {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
}
.sessions-sidebar .session-search-row .kx-input {
  flex: 1 1 auto;
}
.sessions-sidebar .session-search-clear {
  flex: none;
}
.sessions-sidebar .session-search-clear svg {
  width: 14px;
  height: 14px;
  fill: currentColor;
}
.sessions-sidebar .session-scroll {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
}
.sessions-sidebar .sidebar-section {
  display: flex;
  flex: 1;
  min-height: 0;
  max-height: 50%;
  flex-direction: column;
  border-bottom: 1px solid var(--app-border-color);
}
.sessions-sidebar .sidebar-section-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.sessions-sidebar .section-heading {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px 6px;
}
.sessions-sidebar .section-heading h3 {
  margin: 0;
  color: var(--app-text-color);
  font-size: var(--app-text-sm);
  font-weight: 720;
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
  border-radius: var(--app-radius-md);
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
.sessions-sidebar .missing-projects-notice {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin: 0 8px 6px;
  padding: 6px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md);
  color: var(--app-text-color-2);
  font-size: 12px;
}
.sessions-sidebar .project-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 40px;
  padding: 3px 8px;
}
.sessions-sidebar .project-expand-btn {
  flex-shrink: 0;
  width: 28px;
  min-height: 28px;
  border-radius: var(--app-radius-md);
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
  border-radius: var(--app-radius-md);
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
  border-radius: var(--app-radius-md);
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font-family: inherit;
  font-size: 13px;
  text-align: left;
}
.sessions-sidebar .project-session-item.active {
  background: var(--app-selected-color);
  color: var(--app-text-color);
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
.sessions-sidebar .session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.sessions-sidebar .session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 36px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
  position: relative;
}
.sessions-sidebar .session-item:hover {
  background: var(--app-hover-color);
}
.sessions-sidebar .session-item.active {
  background: var(--app-selected-color);
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
  border-radius: var(--app-radius-md);
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
  border-radius: var(--app-radius-sm);
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
</style>
