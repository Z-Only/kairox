<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ProfileInfo } from "../types";
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

const showNewSession = ref(false);
const selectedProfile = ref("fast");
const availableProfiles = ref<ProfileInfo[]>([]);
const editingSessionId = ref<string | null>(null);
const editingTitle = ref("");
const profileDropdownOpen = ref(false);
const renameInput = ref<HTMLInputElement | null>(null);
const showProjectCreateActions = ref(false);
const pendingDeleteSessionId = ref<string | null>(null);
const pendingDeleteProjectId = ref<string | null>(null);
const importingProject = ref(false);

function resetDeleteConfirmation() {
  pendingDeleteSessionId.value = null;
  pendingDeleteProjectId.value = null;
}

function getIconLabel(action: "rename" | "delete" | "confirm" | "import" | "new") {
  const labels = {
    rename: "Rename",
    delete: "Delete",
    confirm: "Confirm delete",
    import: "Import folder",
    new: "New session"
  };
  return labels[action];
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
  try {
    const result = await session.createSession(selectedProfile.value);
    showNewSession.value = false;
    profileDropdownOpen.value = false;
    await router.push({ name: "workbench", params: { sessionId: result.id } });
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

async function loadProfiles() {
  try {
    availableProfiles.value = (await invoke("get_profile_info")) as ProfileInfo[];
    if (availableProfiles.value.length > 0) {
      selectedProfile.value = availableProfiles.value[0].alias;
    }
  } catch (e) {
    console.error("Failed to load profiles:", e);
    // Fallback: try to get just profile names
    try {
      const names: string[] = await invoke("list_profiles");
      availableProfiles.value = names.map((name) => ({
        alias: name,
        provider: "unknown",
        model_id: "unknown",
        local: false,
        has_api_key: false
      }));
      if (names.length > 0) {
        selectedProfile.value = names[0];
      }
    } catch {
      // Ignore fallback failure
    }
  }
}

function openNewSessionDialog() {
  resetDeleteConfirmation();
  loadProfiles();
  showNewSession.value = true;
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

function selectProfile(alias: string) {
  selectedProfile.value = alias;
  profileDropdownOpen.value = false;
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
    showProjectCreateActions.value = false;
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
    showProjectCreateActions.value = false;
  } finally {
    importingProject.value = false;
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

async function toggleArchiveOpen() {
  workspaceUi.archiveOpen = !workspaceUi.archiveOpen;
  if (workspaceUi.archiveOpen && projects.archivedSessions.length === 0) {
    await projects.loadArchivedSessions();
  }
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

function keyIcon(hasApiKey: boolean): string {
  return hasApiKey ? "🔑" : "🚫";
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
            <h3>Projects</h3>
            <div class="section-actions">
              <button
                class="section-action-btn"
                type="button"
                data-test="new-project-btn"
                :aria-expanded="showProjectCreateActions"
                aria-controls="project-create-actions"
                @click="showProjectCreateActions = !showProjectCreateActions"
              >
                New
              </button>
              <button
                class="section-action-btn icon-btn"
                type="button"
                data-test="import-project-btn"
                :aria-label="getIconLabel('import')"
                :disabled="importingProject"
                @click="importExistingProject"
              >
                <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                  <path
                    d="M3 5.5A2.5 2.5 0 0 1 5.5 3H8l2 2h4.5A2.5 2.5 0 0 1 17 7.5v1h-1.5v-1a1 1 0 0 0-1-1H9.38l-2-2H5.5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h3V16h-3A2.5 2.5 0 0 1 3 13.5v-8Z"
                  />
                  <path
                    d="M13 8.75a.75.75 0 0 1 .75.75v3.19l1.22-1.22 1.06 1.06-2.5 2.5a.75.75 0 0 1-1.06 0l-2.5-2.5 1.06-1.06 1.22 1.22V9.5a.75.75 0 0 1 .75-.75Z"
                  />
                </svg>
              </button>
              <button
                class="section-action-btn"
                type="button"
                data-test="project-archive-toggle"
                :aria-label="
                  workspaceUi.archiveOpen
                    ? 'Hide archived project sessions'
                    : 'Show archived project sessions'
                "
                @click="toggleArchiveOpen"
              >
                Archive
              </button>
            </div>
          </div>

          <div
            v-if="showProjectCreateActions"
            id="project-create-actions"
            class="project-create-actions"
          >
            <button
              class="project-create-btn"
              type="button"
              data-test="create-blank-project-btn"
              @click="createBlankProject"
            >
              Create Blank Project
            </button>
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
                      ? `Collapse ${project.displayName}`
                      : `Expand ${project.displayName}`
                  "
                  @click.stop="toggleProjectExpanded(project)"
                >
                  {{ project.expanded ? "▾" : "▸" }}
                </button>
                <button
                  class="project-title-btn"
                  type="button"
                  :aria-label="`Toggle ${project.displayName}`"
                  @click="toggleProjectExpanded(project)"
                >
                  <span class="project-name">{{ project.displayName }}</span>
                  <span class="project-path">{{ project.rootPath }}</span>
                </button>
                <span class="row-actions project-actions">
                  <button
                    class="project-action-btn"
                    type="button"
                    data-test="project-new-session-btn"
                    :aria-label="`New session in ${project.displayName}`"
                    @click.stop="createProjectSession(project.projectId)"
                  >
                    New
                  </button>
                  <button
                    v-if="pendingDeleteProjectId !== project.projectId"
                    class="project-action-btn icon-btn project-remove-btn"
                    type="button"
                    :aria-label="getIconLabel('delete')"
                    data-test="project-delete-btn"
                    @click.stop="requestDeleteProject(project.projectId)"
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path
                        d="M7.5 3.5A1.5 1.5 0 0 1 9 2h2a1.5 1.5 0 0 1 1.5 1.5V4H16v1.5H4V4h3.5v-.5ZM9 4h2v-.5H9V4Z"
                      />
                      <path
                        d="M5.5 7h9l-.55 8.25A2.5 2.5 0 0 1 11.45 17h-2.9a2.5 2.5 0 0 1-2.5-1.75L5.5 7Zm2.25 1.5.43 6.25a1 1 0 0 0 .99.75h1.66a1 1 0 0 0 .99-.75l.43-6.25h-4.5Z"
                      />
                    </svg>
                  </button>
                  <button
                    v-else
                    class="project-action-btn icon-btn project-remove-btn"
                    type="button"
                    :aria-label="getIconLabel('confirm')"
                    data-test="project-delete-confirm"
                    @click.stop="requestDeleteProject(project.projectId)"
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                    </svg>
                  </button>
                </span>
              </div>

              <ul v-if="project.expanded" class="project-session-list">
                <li
                  v-for="projectSession in getProjectSessions(project.projectId)"
                  :key="projectSession.sessionId"
                >
                  <button
                    type="button"
                    :class="[
                      'project-session-item',
                      { active: projectSession.sessionId === activeSessionId }
                    ]"
                    data-test="project-session-btn"
                    :aria-label="`Open ${projectSession.title}`"
                    @click="switchToProjectSession(projectSession)"
                  >
                    <span class="session-indicator">●</span>
                    <span class="session-title">{{ projectSession.title }}</span>
                    <span v-if="projectSession.branch" class="project-session-branch">
                      {{ projectSession.branch }}
                    </span>
                  </button>
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
                :aria-label="`Open ${archivedSession.title}`"
                @click="switchToProjectSession(archivedSession)"
              >
                <span class="session-indicator archived-indicator">●</span>
                <span class="session-title">{{ archivedSession.title }}</span>
              </button>
            </li>
          </ul>
        </section>

        <section v-else class="sidebar-section" data-test="sessions-section">
          <div class="section-heading">
            <h3>Sessions</h3>
            <button
              class="new-session-btn"
              type="button"
              data-test="new-session-btn"
              :aria-label="getIconLabel('new')"
              @click="openNewSessionDialog"
            >
              {{ t("sessions.newButtonPrefix") }}{{ t("sessions.newButton") }}
            </button>
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
                  <button
                    class="icon-btn"
                    type="button"
                    :aria-label="t('common.confirm')"
                    data-test="session-rename-confirm"
                    @mousedown.prevent
                    @click.stop="confirmRename"
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                    </svg>
                  </button>
                </template>

                <!-- Normal display mode -->
                <template v-else>
                  <span class="session-title">{{ item.title }}</span>
                  <span class="row-actions session-actions">
                    <button
                      class="icon-btn"
                      :title="getIconLabel('rename')"
                      :aria-label="getIconLabel('rename')"
                      data-test="session-rename-btn"
                      @click.stop="startRename(item.id, item.title)"
                    >
                      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                        <path
                          d="M13.7 2.3a1 1 0 0 1 1.4 0l2.6 2.6a1 1 0 0 1 0 1.4l-9.45 9.45L4 16l.25-4.25L13.7 2.3Zm.7 1.4-8.7 8.7-.12 2.02 2.02-.12 8.7-8.7-1.9-1.9Z"
                        />
                      </svg>
                    </button>
                    <button
                      v-if="pendingDeleteSessionId !== item.id"
                      class="icon-btn action-delete"
                      :title="getIconLabel('delete')"
                      :aria-label="getIconLabel('delete')"
                      data-test="session-delete-btn"
                      @click.stop="requestDeleteSession(item.id)"
                    >
                      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                        <path
                          d="M7.5 3.5A1.5 1.5 0 0 1 9 2h2a1.5 1.5 0 0 1 1.5 1.5V4H16v1.5H4V4h3.5v-.5ZM9 4h2v-.5H9V4Z"
                        />
                        <path
                          d="M5.5 7h9l-.55 8.25A2.5 2.5 0 0 1 11.45 17h-2.9a2.5 2.5 0 0 1-2.5-1.75L5.5 7Zm2.25 1.5.43 6.25a1 1 0 0 0 .99.75h1.66a1 1 0 0 0 .99-.75l.43-6.25h-4.5Z"
                        />
                      </svg>
                    </button>
                    <button
                      v-else
                      class="icon-btn action-delete"
                      :title="getIconLabel('confirm')"
                      :aria-label="getIconLabel('confirm')"
                      data-test="session-delete-confirm"
                      @click.stop="requestDeleteSession(item.id)"
                    >
                      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                        <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                      </svg>
                    </button>
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

    <!-- New Session Dialog (kept as native <dialog> per Task 5 NIT #8 — out of
         scope for Task 7 spec §5.5 mapping). -->
    <dialog v-if="showNewSession" class="new-session-dialog" data-test="new-session-dialog" open>
      <h3>{{ t("sessions.newDialogTitle") }}</h3>
      <label>
        {{ t("sessions.profileLabel") }}
        <div class="profile-dropdown">
          <button class="profile-trigger" @click="profileDropdownOpen = !profileDropdownOpen">
            {{ selectedProfile }}
            <span class="caret">▼</span>
          </button>
          <div v-if="profileDropdownOpen" class="profile-menu">
            <div
              v-for="p in availableProfiles"
              :key="p.alias"
              :class="['profile-option', { selected: p.alias === selectedProfile }]"
              @click="selectProfile(p.alias)"
            >
              <div class="profile-info">
                <span class="profile-alias">{{ p.alias }}</span>
                <span class="profile-detail" :title="`${p.provider} · ${p.model_id}`">
                  {{ p.provider }} · {{ p.model_id }}
                </span>
              </div>
              <span class="profile-key">{{ keyIcon(p.has_api_key) }}</span>
            </div>
          </div>
        </div>
      </label>
      <div class="dialog-actions">
        <button data-test="create-session-btn" @click="createSession">
          {{ t("sessions.createButton") }}
        </button>
        <button
          @click="
            showNewSession = false;
            profileDropdownOpen = false;
          "
        >
          {{ t("sessions.cancelButton") }}
        </button>
      </div>
    </dialog>
  </aside>
</template>

<style scoped>
.sessions-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.new-session-btn {
  --sessions-new-button-bg: #1d4ed8;
  --sessions-new-button-fg: #fff;

  font-size: 12px;
  padding: 2px 8px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  background: var(--sessions-new-button-bg);
  color: var(--sessions-new-button-fg);
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
  color: var(--app-text-color-2);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.section-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}
.project-create-actions {
  padding: 0 12px 8px;
}
.project-create-btn {
  width: 100%;
  min-height: 32px;
  padding: 6px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  cursor: pointer;
  background: var(--app-card-color);
  color: var(--app-text-color);
  font-family: inherit;
  font-size: 12px;
  text-align: left;
}
.project-create-btn:hover {
  background: var(--app-hover-color);
}
.project-create-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
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
  flex-shrink: 0;
  gap: 2px;
}
.project-remove-btn:hover {
  background: color-mix(in srgb, var(--app-error-color) 10%, transparent);
}
.project-session-list {
  padding: 0 8px 4px 40px;
}
.archived-session-list {
  padding: 0 8px 8px 16px;
}
.project-session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
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
  flex-shrink: 0;
  color: var(--app-text-color-3);
  font-size: 11px;
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
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session-actions {
  gap: 4px;
  flex-shrink: 0;
}
.row-actions {
  display: flex;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease;
}
.session-item:hover .row-actions,
.session-item:focus-within .row-actions,
.project-row:hover .row-actions,
.project-row:focus-within .row-actions {
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
  border: 1px solid var(--app-primary-color);
  border-radius: 3px;
  padding: 2px 4px;
  font-size: 13px;
  outline: none;
  font-family: inherit;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.empty-hint {
  padding: 12px;
  color: var(--app-text-color-3);
  font-size: 13px;
}

/* New Session Dialog */
.new-session-dialog {
  min-width: 340px;
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  padding: 20px;
  z-index: 100;
  box-shadow: var(--app-shadow-2, 0 4px 16px rgba(0, 0, 0, 0.15));
}
.new-session-dialog h3 {
  margin: 0 0 12px;
}
.new-session-dialog label {
  display: block;
  margin-bottom: 12px;
  font-size: 13px;
}

/* Profile Dropdown */
.profile-dropdown {
  position: relative;
  margin-top: 6px;
}
.profile-trigger {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  cursor: pointer;
  font-size: 13px;
  text-align: left;
  color: var(--app-text-color);
}
.caret {
  font-size: 10px;
  color: var(--app-text-color-3);
}
.profile-menu {
  position: absolute;
  top: 100%;
  left: 0;
  min-width: 320px;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  box-shadow: var(--app-shadow-1, 0 4px 12px rgba(0, 0, 0, 0.1));
  z-index: 10;
  max-height: 200px;
  overflow-y: auto;
}
.profile-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  cursor: pointer;
  font-size: 12px;
}
.profile-option:hover {
  background: var(--app-hover-color);
}
.profile-option.selected {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  font-weight: 600;
}
.profile-alias {
  font-weight: 600;
  font-size: 13px;
}
.profile-detail {
  color: var(--app-text-color-2);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.profile-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.profile-key {
  flex-shrink: 0;
  font-size: 11px;
}

.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.dialog-actions button {
  padding: 6px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  cursor: pointer;
  background: var(--app-card-color);
  font-size: 13px;
}
.dialog-actions button:first-child {
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-primary-color);
}
</style>
