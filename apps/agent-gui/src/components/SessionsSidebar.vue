<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "@/stores/session";
import { useProjectStore, type ProjectInfo, type ProjectSessionInfo } from "@/stores/project";
import { useWorkspaceUiStore, type SidebarSection } from "@/stores/workspaceUi";

const { t } = useI18n();

const session = useSessionStore();
const projects = useProjectStore();
const workspaceUi = useWorkspaceUiStore();
const route = useRoute();
const router = useRouter();

// The active session is derived from the URL (`/workbench/:sessionId?`),
// so navigation through the sidebar drives the router and the router
// drives the store via WorkbenchView's watcher.
const activeSessionId = computed<string | null>(() => {
  const v = route.params.sessionId;
  const id = Array.isArray(v) ? v[0] : v;
  return id ?? session.currentSessionId;
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

const editingSessionId = ref<string | null>(null);
const editingTitle = ref("");
const renameInput = ref<HTMLInputElement | null>(null);
const projectCreateMenuOpen = ref(false);
const editingProjectId = ref<string | null>(null);
const editingProjectName = ref("");
const editingProjectSessionId = ref<string | null>(null);
const editingProjectSessionTitle = ref("");
const projectRenameInput = ref<HTMLInputElement | null>(null);
const projectSessionRenameInput = ref<HTMLInputElement | null>(null);
const pendingDeleteSessionId = ref<string | null>(null);
const pendingDeleteProjectId = ref<string | null>(null);
const importingProject = ref(false);

function resetDeleteConfirmation() {
  pendingDeleteSessionId.value = null;
  pendingDeleteProjectId.value = null;
}

async function switchToSession(sessionId: string) {
  if (editingSessionId.value) return;
  if (sessionId === activeSessionId.value) return;
  resetDeleteConfirmation();
  try {
    await router.push({ name: "workbench", params: { sessionId } });
  } catch (e) {
    console.error("Failed to navigate to session:", e);
  }
}

async function createSession() {
  resetDeleteConfirmation();
  try {
    const result = await session.createSession(undefined);
    await router.push({ name: "workbench", params: { sessionId: result.id } });
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

function startRename(sessionId: string, currentTitle: string) {
  resetDeleteConfirmation();
  editingSessionId.value = sessionId;
  editingTitle.value = currentTitle;
  nextTick(() => {
    renameInput.value?.focus();
    renameInput.value?.select();
  });
}

// Functional ref for the rename `<input>` inside `v-for`. Vue 3 treats a
// string `ref="renameInput"` inside `v-for` as an array (one entry per
// iteration); the previous code happened to work because
// `editingSessionId === item.id` ensures only one `<input>` is rendered at
// any time, but it was a latent foot-gun. The functional ref pins the
// variable to the single editing row explicitly.
function bindRenameInput(el: Element | null, itemId: string) {
  if (editingSessionId.value === itemId) {
    renameInput.value = (el as HTMLInputElement) ?? null;
  }
}

async function confirmRename() {
  if (editingSessionId.value && editingTitle.value.trim()) {
    await session.renameSession(editingSessionId.value, editingTitle.value.trim());
  }
  editingSessionId.value = null;
}

function cancelRename() {
  editingSessionId.value = null;
}

async function requestDeleteSession(sessionId: string) {
  if (pendingDeleteSessionId.value !== sessionId) {
    pendingDeleteSessionId.value = sessionId;
    pendingDeleteProjectId.value = null;
    return;
  }
  await session.deleteSession(sessionId);
  pendingDeleteSessionId.value = null;
}

function getProjectSessions(projectId: string): ProjectSessionInfo[] {
  return projects.sessionsByProject.get(projectId) ?? [];
}

async function activateProjectSession(projectSession: ProjectSessionInfo) {
  resetDeleteConfirmation();
  await session.switchProjectSession(projectSession);
  await router.push({ name: "workbench", params: { sessionId: projectSession.sessionId } });
}

async function switchToProjectSession(projectSession: ProjectSessionInfo) {
  try {
    await activateProjectSession(projectSession);
  } catch (e) {
    console.error("Failed to open project session:", e);
  }
}

async function createProjectSession(projectId: string) {
  try {
    const projectSession = await projects.createProjectDraftSession(projectId);
    await activateProjectSession(projectSession);
  } catch (e) {
    console.error("Failed to start project session:", e);
  }
}

async function createBlankProject() {
  resetDeleteConfirmation();
  try {
    await projects.createBlankProject();
    projectCreateMenuOpen.value = false;
  } catch (e) {
    console.error("Failed to create blank project:", e);
  }
}

async function importExistingProject() {
  if (importingProject.value) return;
  resetDeleteConfirmation();
  importingProject.value = true;
  try {
    const selectedPath = await open({ directory: true, multiple: false });
    if (!selectedPath || Array.isArray(selectedPath)) return;
    await projects.addExistingProject(selectedPath);
    await projects.loadProjects();
    projectCreateMenuOpen.value = false;
  } finally {
    importingProject.value = false;
  }
}

function startProjectRename(project: ProjectInfo) {
  resetDeleteConfirmation();
  editingProjectId.value = project.projectId;
  editingProjectName.value = project.displayName;
  nextTick(() => {
    projectRenameInput.value?.focus();
    projectRenameInput.value?.select();
  });
}

function bindProjectRenameInput(el: Element | null, projectId: string) {
  if (editingProjectId.value === projectId) {
    projectRenameInput.value = (el as HTMLInputElement) ?? null;
  }
}

async function confirmProjectRename() {
  if (editingProjectId.value && editingProjectName.value.trim()) {
    await projects.renameProject(editingProjectId.value, editingProjectName.value.trim());
  }
  editingProjectId.value = null;
}

function cancelProjectRename() {
  editingProjectId.value = null;
}

function startProjectSessionRename(projectSession: ProjectSessionInfo) {
  resetDeleteConfirmation();
  editingProjectSessionId.value = projectSession.sessionId;
  editingProjectSessionTitle.value = projectSession.title;
  nextTick(() => {
    projectSessionRenameInput.value?.focus();
    projectSessionRenameInput.value?.select();
  });
}

function bindProjectSessionRenameInput(el: Element | null, sessionId: string) {
  if (editingProjectSessionId.value === sessionId) {
    projectSessionRenameInput.value = (el as HTMLInputElement) ?? null;
  }
}

async function confirmProjectSessionRename() {
  if (editingProjectSessionId.value && editingProjectSessionTitle.value.trim()) {
    await projects.renameProjectSession(
      editingProjectSessionId.value,
      editingProjectSessionTitle.value.trim()
    );
  }
  editingProjectSessionId.value = null;
}

function cancelProjectSessionRename() {
  editingProjectSessionId.value = null;
}

async function archiveProjectSession(sessionId: string) {
  resetDeleteConfirmation();
  try {
    await projects.archiveProjectSession(sessionId);
  } catch (e) {
    console.error("Failed to archive project session:", e);
  }
}

async function toggleProjectExpanded(project: ProjectInfo) {
  const expanded = !project.expanded;
  try {
    await projects.updateProjectExpanded(project.projectId, expanded);
    if (expanded) {
      await projects.loadProjectSessions(project.projectId);
    }
  } catch (e) {
    console.error("Failed to update project expansion:", e);
  }
}

async function requestDeleteProject(projectId: string) {
  if (pendingDeleteProjectId.value !== projectId) {
    pendingDeleteProjectId.value = projectId;
    pendingDeleteSessionId.value = null;
    return;
  }
  await projects.removeProject(projectId);
  pendingDeleteProjectId.value = null;
}

async function loadProjectsForSidebar() {
  try {
    await projects.loadProjects();
    await Promise.all(
      projects.activeProjects
        .filter((project) => project.expanded)
        .map((project) => projects.loadProjectSessions(project.projectId))
    );
  } catch (e) {
    console.error("Failed to load projects:", e);
  }
}

onMounted(() => {
  void loadProjectsForSidebar();
});
</script>

<template>
  <aside class="sessions-sidebar" data-test="sessions-sidebar" :aria-label="t('sessions.header')">
    <div class="session-scroll">
      <template v-for="sectionName in orderedSidebarSections" :key="sectionName">
        <section
          v-if="sectionName === 'projects'"
          class="sidebar-section"
          data-test="projects-section"
        >
          <div class="section-heading">
            <h3>{{ t("sessions.projectHeader") }}</h3>
            <div class="section-actions">
              <KxDropdownMenu
                v-model:open="projectCreateMenuOpen"
                content-data-test="project-create-menu"
                align="end"
              >
                <template #trigger>
                  <KxIconButton
                    :label="t('sessions.newProject')"
                    :title="t('sessions.newProject')"
                    data-test="project-create-trigger"
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path d="M9.25 3h1.5v6.25H17v1.5h-6.25V17h-1.5v-6.25H3v-1.5h6.25V3Z" />
                    </svg>
                  </KxIconButton>
                </template>
                <template #content>
                  <button
                    class="kx-dropdown-item project-create-menu-item"
                    type="button"
                    data-test="project-create-blank"
                    @click="createBlankProject"
                  >
                    {{ t("sessions.createBlankProject") }}
                  </button>
                  <button
                    class="kx-dropdown-item project-create-menu-item"
                    type="button"
                    data-test="project-import-folder"
                    :disabled="importingProject"
                    @click="importExistingProject"
                  >
                    {{ t("sessions.importFolder") }}
                  </button>
                </template>
              </KxDropdownMenu>
            </div>
          </div>

          <ul class="project-list">
            <li
              v-for="project in projects.activeProjects"
              :key="project.projectId"
              class="project-item"
              data-test="project-item"
            >
              <div class="project-row">
                <button
                  class="project-expand-btn"
                  type="button"
                  data-test="project-expand-btn"
                  :aria-label="
                    project.expanded
                      ? t('sessions.collapseProject', { name: project.displayName })
                      : t('sessions.expandProject', { name: project.displayName })
                  "
                  @click.stop="toggleProjectExpanded(project)"
                >
                  {{ project.expanded ? "▾" : "▸" }}
                </button>
                <template v-if="editingProjectId === project.projectId">
                  <input
                    :ref="(el) => bindProjectRenameInput(el as Element | null, project.projectId)"
                    v-model="editingProjectName"
                    class="rename-input project-rename-input"
                    :data-test="`project-rename-input-${project.projectId}`"
                    @keydown.enter="confirmProjectRename"
                    @keydown.escape="cancelProjectRename"
                    @blur="confirmProjectRename"
                    @click.stop
                  />
                  <KxTooltip :text="t('common.confirm')">
                    <KxIconButton
                      :label="t('common.confirm')"
                      :title="t('common.confirm')"
                      data-test="project-rename-confirm"
                      @mousedown.prevent
                      @click.stop="confirmProjectRename"
                    >
                      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                        <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                      </svg>
                    </KxIconButton>
                  </KxTooltip>
                </template>
                <template v-else>
                  <button
                    class="project-title-btn"
                    type="button"
                    :aria-label="t('sessions.toggleProject', { name: project.displayName })"
                    @click="toggleProjectExpanded(project)"
                  >
                    <span class="project-name truncate" :title="project.displayName">
                      {{ project.displayName }}
                    </span>
                    <span class="project-path truncate" :title="project.branch || project.rootPath">
                      {{ project.branch || project.rootPath }}
                    </span>
                  </button>
                  <span class="row-actions project-actions">
                    <KxTooltip
                      :text="t('sessions.newSessionInProject', { name: project.displayName })"
                    >
                      <KxIconButton
                        :label="t('sessions.newSessionInProject', { name: project.displayName })"
                        :data-test="'project-new-session-btn'"
                        @click.stop="createProjectSession(project.projectId)"
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                          <path d="M9.25 3h1.5v6.25H17v1.5h-6.25V17h-1.5v-6.25H3v-1.5h6.25V3Z" />
                        </svg>
                      </KxIconButton>
                    </KxTooltip>
                    <KxTooltip :text="t('sessions.renameTitle')">
                      <KxIconButton
                        :label="t('sessions.renameTitle')"
                        :data-test="`project-rename-action-${project.projectId}`"
                        @click.stop="startProjectRename(project)"
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                          <path
                            d="M13.7 2.3a1 1 0 0 1 1.4 0l2.6 2.6a1 1 0 0 1 0 1.4l-9.45 9.45L4 16l.25-4.25L13.7 2.3Zm.7 1.4-8.7 8.7-.12 2.02 2.02-.12 8.7-8.7-1.9-1.9Z"
                          />
                        </svg>
                      </KxIconButton>
                    </KxTooltip>
                    <KxTooltip
                      :text="
                        pendingDeleteProjectId === project.projectId
                          ? t('sessions.confirmDeleteTitle')
                          : t('common.delete')
                      "
                    >
                      <KxIconButton
                        :label="
                          pendingDeleteProjectId === project.projectId
                            ? t('sessions.confirmDeleteTitle')
                            : t('common.delete')
                        "
                        :title="
                          pendingDeleteProjectId === project.projectId
                            ? t('sessions.confirmDeleteTitle')
                            : t('common.delete')
                        "
                        variant="danger"
                        :data-test="
                          pendingDeleteProjectId === project.projectId
                            ? 'project-delete-confirm'
                            : 'project-delete-btn'
                        "
                        @click.stop="requestDeleteProject(project.projectId)"
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                          <path
                            v-if="pendingDeleteProjectId === project.projectId"
                            d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z"
                          />
                          <template v-else>
                            <path
                              d="M7.5 3.5A1.5 1.5 0 0 1 9 2h2a1.5 1.5 0 0 1 1.5 1.5V4H16v1.5H4V4h3.5v-.5ZM9 4h2v-.5H9V4Z"
                            />
                            <path
                              d="M5.5 7h9l-.55 8.25A2.5 2.5 0 0 1 11.45 17h-2.9a2.5 2.5 0 0 1-2.5-1.75L5.5 7Zm2.25 1.5.43 6.25a1 1 0 0 0 .99.75h1.66a1 1 0 0 0 .99-.75l.43-6.25h-4.5Z"
                            />
                          </template>
                        </svg>
                      </KxIconButton>
                    </KxTooltip>
                  </span>
                </template>
              </div>

              <ul v-if="project.expanded" class="project-session-list">
                <li
                  v-for="projectSession in getProjectSessions(project.projectId)"
                  :key="projectSession.sessionId"
                  class="project-session-row"
                >
                  <template v-if="editingProjectSessionId === projectSession.sessionId">
                    <input
                      :ref="
                        (el) =>
                          bindProjectSessionRenameInput(
                            el as Element | null,
                            projectSession.sessionId
                          )
                      "
                      v-model="editingProjectSessionTitle"
                      class="rename-input project-session-rename-input"
                      :data-test="`project-session-rename-input-${projectSession.sessionId}`"
                      @keydown.enter="confirmProjectSessionRename"
                      @keydown.escape="cancelProjectSessionRename"
                      @blur="confirmProjectSessionRename"
                      @click.stop
                    />
                    <KxTooltip :text="t('common.confirm')">
                      <KxIconButton
                        :label="t('common.confirm')"
                        :title="t('common.confirm')"
                        :data-test="`project-session-rename-confirm-${projectSession.sessionId}`"
                        @mousedown.prevent
                        @click.stop="confirmProjectSessionRename"
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                          <path
                            d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z"
                          />
                        </svg>
                      </KxIconButton>
                    </KxTooltip>
                  </template>
                  <template v-else>
                    <button
                      type="button"
                      :class="[
                        'project-session-item',
                        { active: projectSession.sessionId === activeSessionId }
                      ]"
                      data-test="project-session-btn"
                      :aria-label="
                        t('sessions.openProjectSession', { title: projectSession.title })
                      "
                      @click="switchToProjectSession(projectSession)"
                    >
                      <span class="session-indicator">●</span>
                      <span class="session-title truncate" :title="projectSession.title">
                        {{ projectSession.title }}
                      </span>
                      <span
                        v-if="projectSession.branch && projectSession.branch !== 'main'"
                        class="project-session-branch truncate"
                        :title="projectSession.branch"
                      >
                        {{ projectSession.branch }}
                      </span>
                    </button>
                    <span class="row-actions project-session-actions">
                      <KxTooltip :text="t('sessions.renameTitle')">
                        <KxIconButton
                          :label="t('sessions.renameTitle')"
                          :data-test="`project-session-rename-action-${projectSession.sessionId}`"
                          @click.stop="startProjectSessionRename(projectSession)"
                        >
                          <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                            <path
                              d="M13.7 2.3a1 1 0 0 1 1.4 0l2.6 2.6a1 1 0 0 1 0 1.4l-9.45 9.45L4 16l.25-4.25L13.7 2.3Zm.7 1.4-8.7 8.7-.12 2.02 2.02-.12 8.7-8.7-1.9-1.9Z"
                            />
                          </svg>
                        </KxIconButton>
                      </KxTooltip>
                      <KxTooltip :text="t('sessions.archive')">
                        <KxIconButton
                          :label="t('sessions.archive')"
                          :data-test="`project-session-archive-action-${projectSession.sessionId}`"
                          @click.stop="archiveProjectSession(projectSession.sessionId)"
                        >
                          <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                            <path
                              d="M4 3h12v3H4V3Zm1.5 1.5v.75h9v-.75h-9ZM5 7h10v8.5A1.5 1.5 0 0 1 13.5 17h-7A1.5 1.5 0 0 1 5 15.5V7Zm3 2v1.5h4V9H8Z"
                            />
                          </svg>
                        </KxIconButton>
                      </KxTooltip>
                    </span>
                  </template>
                </li>
              </ul>
            </li>
          </ul>

          <ul v-if="workspaceUi.archiveOpen" class="project-session-list archived-session-list">
            <li
              v-for="archivedSession in projects.archivedSessions"
              :key="archivedSession.sessionId"
            >
              <button
                type="button"
                :class="[
                  'project-session-item',
                  { active: archivedSession.sessionId === activeSessionId }
                ]"
                data-test="project-session-btn"
                :aria-label="t('sessions.openProjectSession', { title: archivedSession.title })"
                @click="switchToProjectSession(archivedSession)"
              >
                <span class="session-indicator archived-indicator">●</span>
                <span class="session-title truncate" :title="archivedSession.title">
                  {{ archivedSession.title }}
                </span>
              </button>
            </li>
          </ul>
        </section>

        <section v-else class="sidebar-section" data-test="sessions-section">
          <div class="section-heading">
            <h3>{{ t("sessions.header") }}</h3>
            <div class="section-actions">
              <KxTooltip :text="t('sessions.newButton')">
                <KxIconButton
                  :label="t('sessions.newButton')"
                  :title="t('sessions.newButton')"
                  data-test="new-session-btn"
                  @click="createSession"
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                    <path d="M9.25 3h1.5v6.25H17v1.5h-6.25V17h-1.5v-6.25H3v-1.5h6.25V3Z" />
                  </svg>
                </KxIconButton>
              </KxTooltip>
            </div>
          </div>
          <template v-if="session.sessions.length > 0">
            <!-- Kept hand-rolled because NListItem #suffix slot cannot express the current compact row layout. -->
            <ul class="session-list">
              <li
                v-for="item in session.sessions"
                :key="item.id"
                :class="['session-item', { active: item.id === activeSessionId }]"
                data-test="session-item"
                @click="switchToSession(item.id)"
              >
                <span class="session-indicator">●</span>

                <!-- Inline rename mode -->
                <template v-if="editingSessionId === item.id">
                  <input
                    :ref="(el) => bindRenameInput(el as Element | null, item.id)"
                    v-model="editingTitle"
                    class="rename-input"
                    data-test="session-rename-input"
                    @keydown.enter="confirmRename"
                    @keydown.escape="cancelRename"
                    @blur="confirmRename"
                    @click.stop
                  />
                  <KxTooltip :text="t('common.confirm')">
                    <KxIconButton
                      :label="t('common.confirm')"
                      :title="t('common.confirm')"
                      data-test="session-rename-confirm"
                      @mousedown.prevent
                      @click.stop="confirmRename"
                    >
                      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                        <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                      </svg>
                    </KxIconButton>
                  </KxTooltip>
                </template>

                <!-- Normal display mode -->
                <template v-else>
                  <span class="session-title truncate" :title="item.title">{{ item.title }}</span>
                  <span class="row-actions session-actions">
                    <KxTooltip :text="t('sessions.renameTitle')">
                      <KxIconButton
                        :label="t('sessions.renameTitle')"
                        data-test="session-rename-btn"
                        @click.stop="startRename(item.id, item.title)"
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                          <path
                            d="M13.7 2.3a1 1 0 0 1 1.4 0l2.6 2.6a1 1 0 0 1 0 1.4l-9.45 9.45L4 16l.25-4.25L13.7 2.3Zm.7 1.4-8.7 8.7-.12 2.02 2.02-.12 8.7-8.7-1.9-1.9Z"
                          />
                        </svg>
                      </KxIconButton>
                    </KxTooltip>
                    <KxTooltip :text="t('sessions.archive')">
                      <KxIconButton
                        :label="t('sessions.archive')"
                        data-test="session-archive-btn"
                        @click.stop="requestDeleteSession(item.id)"
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                          <path
                            d="M4 3h12v3H4V3Zm1.5 1.5v.75h9v-.75h-9ZM5 7h10v8.5A1.5 1.5 0 0 1 13.5 17h-7A1.5 1.5 0 0 1 5 15.5V7Zm3 2v1.5h4V9H8Z"
                          />
                        </svg>
                      </KxIconButton>
                    </KxTooltip>
                  </span>
                </template>
              </li>
            </ul>
          </template>
          <div v-else class="empty-state empty-hint" data-test="sessions-empty">
            {{ t("sessions.emptyHint") }}
          </div>
        </section>
      </template>
    </div>
  </aside>
</template>

<style scoped>
.sessions-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.session-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.sidebar-section {
  border-bottom: 1px solid var(--app-border-color);
}
.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 12px 4px;
}
.section-heading h3 {
  margin: 0;
  color: var(--app-text-color);
  font-size: 12px;
  font-weight: 600;
}
.section-actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: 4px;
}
.project-create-menu-item {
  width: 100%;
  border: none;
  font-family: inherit;
  text-align: left;
}
.section-action-btn,
.project-action-btn,
.project-expand-btn,
.project-title-btn {
  border: none;
  cursor: pointer;
  background: transparent;
  color: var(--app-text-color);
  font-family: inherit;
}
.section-action-btn,
.project-action-btn {
  min-height: 28px;
  padding: 4px 8px;
  border-radius: 4px;
  color: var(--app-text-color-2);
  font-size: 12px;
}
.section-action-btn:hover,
.project-action-btn:hover,
.project-expand-btn:hover,
.project-title-btn:hover,
.project-session-item:hover {
  background: var(--app-hover-color);
}
.section-action-btn:focus-visible,
.project-action-btn:focus-visible,
.project-expand-btn:focus-visible,
.project-title-btn:focus-visible,
.project-session-item:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
.project-list,
.project-session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.project-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 40px;
  padding: 2px 8px;
}
.project-expand-btn {
  flex-shrink: 0;
  width: 28px;
  min-height: 28px;
  border-radius: 4px;
  font-size: 13px;
}
.project-title-btn {
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
.project-name,
.project-path {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.project-name {
  font-size: 13px;
  font-weight: 600;
}
.project-path {
  color: var(--app-text-color-3);
  font-size: 11px;
}
.project-actions {
  flex: none;
  gap: 2px;
}
.project-session-list {
  padding: 0 8px 4px 40px;
}
.archived-session-list {
  padding: 0 8px 8px 16px;
}
.project-session-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}
.project-session-item {
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
.project-session-item.active {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  font-weight: 600;
}
.project-session-branch {
  flex: none;
  max-width: 72px;
  color: var(--app-text-color-3);
  font-size: 11px;
}
.project-session-actions {
  flex: none;
  gap: 2px;
}
.archived-indicator {
  color: var(--app-text-color-3);
}
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  min-height: 80px;
}
.session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
  position: relative;
}
.session-item:hover {
  background: var(--app-hover-color);
}
.session-item.active {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  font-weight: 600;
}
.session-indicator {
  color: var(--app-success-color);
  font-size: 10px;
  flex-shrink: 0;
}
.session-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session-actions {
  flex: none;
  gap: 4px;
}
.row-actions {
  display: flex;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease;
}
.row-actions :deep(.kx-icon-button) {
  color: var(--app-text-color) !important;
  background: transparent !important;
  border: none !important;
}
.row-actions :deep(.kx-icon-button) svg {
  fill: currentColor !important;
}
html.dark .row-actions :deep(.kx-icon-button) {
  color: var(--app-text-color) !important;
  background: transparent !important;
  border: none !important;
}
html.dark .row-actions :deep(.kx-icon-button) svg {
  fill: currentColor !important;
}
.session-item:hover .row-actions,
.session-item:focus-within .row-actions,
.project-row:hover .row-actions,
.project-row:focus-within .row-actions,
.project-session-row:hover .row-actions,
.project-session-row:focus-within .row-actions {
  opacity: 1;
  pointer-events: auto;
}
.icon-btn {
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
.icon-btn:hover {
  background: var(--app-hover-color);
  color: var(--app-text-color);
}
.icon-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
.icon-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.icon-btn svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.action-delete:hover {
  background: color-mix(in srgb, var(--app-error-color) 10%, transparent);
}
.rename-input {
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
.project-rename-input,
.project-session-rename-input {
  min-height: 28px;
}
.empty-hint {
  padding: 12px;
  color: var(--app-text-color-3);
  font-size: 13px;
}
</style>
