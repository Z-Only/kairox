<script setup lang="ts">
import type { SidebarRenameController } from "@/composables/sidebar/useSidebarRename";
import type { ProjectInfo, ProjectSessionInfo } from "@/stores/project";

const { t } = useI18n();

const projectCreateMenuOpen = defineModel<boolean>("projectCreateMenuOpen", { required: true });
const worktreeBranchInput = defineModel<string>("worktreeBranchInput", { default: "" });

defineProps<{
  activeProjects: ProjectInfo[];
  archivedSessions: ProjectSessionInfo[];
  activeSessionId: string | null;
  pendingDeleteProjectId: string | null;
  pendingArchiveProjectSessionId: string | null;
  importingProject: boolean;
  projectRename: SidebarRenameController;
  projectSessionRename: SidebarRenameController;
  getProjectSessions: (projectId: string) => ProjectSessionInfo[];
  createBlankProject: () => Promise<void> | void;
  importExistingProject: () => Promise<void> | void;
  toggleProjectExpanded: (project: ProjectInfo) => Promise<void> | void;
  createProjectSession: (projectId: string) => Promise<void> | void;
  requestDeleteProject: (projectId: string) => Promise<void> | void;
  switchToProjectSession: (projectSession: ProjectSessionInfo) => Promise<void> | void;
  requestArchiveProjectSession: (sessionId: string) => Promise<void> | void;
  archiveOpen: boolean;
  worktreeBranchProjectId: string | null;
  startWorktreeSession: (projectId: string) => void;
  cancelWorktreeSession: () => void;
  confirmWorktreeSession: () => Promise<void> | void;
}>();
</script>

<template>
  <section class="sidebar-section" data-test="projects-section">
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

    <div class="sidebar-section-scroll" data-test="projects-scroll-region">
      <ul class="project-list">
        <li
          v-for="project in activeProjects"
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
            <template v-if="projectRename.editingId.value === project.projectId">
              <KxEditableLabel
                v-model="projectRename.title.value"
                :input-ref="(el) => projectRename.bindInput(el, project.projectId)"
                :input-data-test="`project-rename-input-${project.projectId}`"
                confirm-data-test="project-rename-confirm"
                :confirm-label="t('common.confirm')"
                @confirm="projectRename.confirm"
                @cancel="projectRename.cancel"
                @click.stop
              />
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
                <KxTooltip :text="t('sessions.newSessionInProject', { name: project.displayName })">
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
                <KxTooltip
                  :text="t('sessions.newWorktreeSessionInProject', { name: project.displayName })"
                >
                  <KxIconButton
                    :label="
                      t('sessions.newWorktreeSessionInProject', { name: project.displayName })
                    "
                    :data-test="`project-new-worktree-session-btn-${project.projectId}`"
                    @click.stop="startWorktreeSession(project.projectId)"
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                      <path
                        d="M3 4.5A1.5 1.5 0 0 1 4.5 3h5.75v1.5H4.5v11h11v-5.75H17v7.25H3V4.5Zm6.25 9.75h1.5V11H14V9.5h-3.25V6.25h-1.5V9.5H6V11h3.25v3.25Z"
                      />
                    </svg>
                  </KxIconButton>
                </KxTooltip>
                <KxTooltip :text="t('sessions.renameTitle')">
                  <KxIconButton
                    :label="t('sessions.renameTitle')"
                    :data-test="`project-rename-action-${project.projectId}`"
                    @click.stop="projectRename.start(project.projectId, project.displayName)"
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

          <div
            v-if="worktreeBranchProjectId === project.projectId"
            class="worktree-branch-input-row"
          >
            <input
              v-model="worktreeBranchInput"
              class="rename-input worktree-branch-input"
              :placeholder="t('sessions.worktreeBranchPlaceholder')"
              data-test="worktree-branch-input"
              @keydown.enter="confirmWorktreeSession"
              @keydown.escape="cancelWorktreeSession"
            />
            <KxTooltip :text="t('common.confirm')">
              <KxIconButton
                :label="t('common.confirm')"
                :title="t('common.confirm')"
                data-test="worktree-branch-confirm"
                @click.stop="confirmWorktreeSession"
              >
                <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                  <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                </svg>
              </KxIconButton>
            </KxTooltip>
          </div>

          <ul v-if="project.expanded" class="project-session-list">
            <li
              v-for="projectSession in getProjectSessions(project.projectId)"
              :key="projectSession.sessionId"
              class="project-session-row"
            >
              <template v-if="projectSessionRename.editingId.value === projectSession.sessionId">
                <KxEditableLabel
                  v-model="projectSessionRename.title.value"
                  :input-ref="(el) => projectSessionRename.bindInput(el, projectSession.sessionId)"
                  :input-data-test="`project-session-rename-input-${projectSession.sessionId}`"
                  :confirm-data-test="`project-session-rename-confirm-${projectSession.sessionId}`"
                  :confirm-label="t('common.confirm')"
                  @confirm="projectSessionRename.confirm"
                  @cancel="projectSessionRename.cancel"
                  @click.stop
                />
              </template>
              <template v-else>
                <button
                  type="button"
                  :class="[
                    'project-session-item',
                    { active: projectSession.sessionId === activeSessionId }
                  ]"
                  data-test="project-session-btn"
                  :aria-label="t('sessions.openProjectSession', { title: projectSession.title })"
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
                      @click.stop="
                        projectSessionRename.start(projectSession.sessionId, projectSession.title)
                      "
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
                      pendingArchiveProjectSessionId === projectSession.sessionId
                        ? t('sessions.confirmArchive')
                        : t('sessions.archive')
                    "
                  >
                    <KxIconButton
                      :label="
                        pendingArchiveProjectSessionId === projectSession.sessionId
                          ? t('sessions.confirmArchive')
                          : t('sessions.archive')
                      "
                      :class="{
                        'confirm-action':
                          pendingArchiveProjectSessionId === projectSession.sessionId
                      }"
                      :data-test="`project-session-archive-action-${projectSession.sessionId}`"
                      @click.stop="requestArchiveProjectSession(projectSession.sessionId)"
                    >
                      <svg
                        v-if="pendingArchiveProjectSessionId === projectSession.sessionId"
                        viewBox="0 0 20 20"
                        aria-hidden="true"
                        focusable="false"
                      >
                        <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
                      </svg>
                      <svg v-else viewBox="0 0 20 20" aria-hidden="true" focusable="false">
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

      <ul v-if="archiveOpen" class="project-session-list archived-session-list">
        <li v-for="archivedSession in archivedSessions" :key="archivedSession.sessionId">
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
    </div>
  </section>
</template>
