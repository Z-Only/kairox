<script setup lang="ts">
import { commands } from "@/generated/commands";
import { useConfirm } from "@/composables/useConfirm";
import { useProjectStore } from "@/stores/project";
import type { ProjectSessionInfo } from "@/stores/project";

const { t } = useI18n();
const { confirm: confirmAction } = useConfirm();
const projectStore = useProjectStore();
const loading = ref(false);
const error = ref<string | null>(null);
const busySessionId = ref<string | null>(null);

const projectMap = computed(() => {
  const map = new Map<string, string>();
  for (const p of projectStore.projects) {
    map.set(p.projectId, p.displayName);
  }
  return map;
});

const stats = computed(() => {
  const sessions = projectStore.archivedSessions;
  const projectIds = new Set(sessions.map((s) => s.projectId).filter(Boolean) as string[]);
  let recentDate: string | null = null;
  for (const s of sessions) {
    if (s.worktreePath && (!recentDate || s.worktreePath > recentDate)) {
      recentDate = s.worktreePath;
    }
  }
  return {
    total: sessions.length,
    projects: projectIds.size,
    recentDate
  };
});

function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

function getProjectDisplayName(session: ProjectSessionInfo): string {
  return session.projectId ? (projectMap.value.get(session.projectId) ?? session.projectId) : "-";
}

async function restoreSession(sessionId: string): Promise<void> {
  busySessionId.value = sessionId;
  error.value = null;
  try {
    await commands.restoreArchivedSession(sessionId);
    await projectStore.loadArchivedSessions();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    busySessionId.value = null;
  }
}

async function permanentlyDelete(sessionId: string): Promise<void> {
  const confirmed = await confirmAction({
    title: t("sessions.confirmDeleteTitle"),
    message: t("settings.archiveDeleteConfirm"),
    confirmText: t("settings.archivePermanentDelete"),
    cancelText: t("common.cancel"),
    type: "error"
  });
  if (!confirmed) return;
  busySessionId.value = sessionId;
  error.value = null;
  try {
    await commands.permanentlyDeleteSession(sessionId);
    await projectStore.loadArchivedSessions();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    busySessionId.value = null;
  }
}

onMounted(() => {
  void projectStore.loadArchivedSessions();
});
</script>

<template>
  <section class="archive-settings" aria-label="Archive" data-test="archive-settings-pane">
    <p v-if="error" class="alert alert-error" role="alert" data-test="archive-page-error">
      {{ error }}
    </p>

    <div v-if="stats.total > 0" class="archive-stats" data-test="archive-stats">
      <span class="tag">{{ t("settings.archiveTotal", { count: stats.total }) }}</span>
      <span class="tag">{{ t("settings.archiveProjects", { count: stats.projects }) }}</span>
    </div>

    <p v-if="loading" class="alert alert-info" role="status">
      {{ t("common.loading") }}
    </p>
    <p v-else-if="projectStore.archivedSessions.length === 0" class="empty-state">
      {{ t("settings.archiveEmpty") }}
    </p>

    <div v-else class="archive-list" role="list" aria-label="Archived sessions">
      <article
        v-for="session in projectStore.archivedSessions"
        :key="session.sessionId"
        class="card archive-row"
        role="listitem"
        :data-test="`archive-row-${session.sessionId}`"
      >
        <div class="card-body archive-row__body">
          <div class="archive-row__main">
            <h4>{{ session.title }}</h4>
            <p class="archive-row__meta">
              <span>{{ getProjectDisplayName(session) }}</span>
              <span v-if="session.profile">{{ session.profile }}</span>
              <span v-if="session.branch">{{ session.branch }}</span>
            </p>
          </div>
          <div class="archive-row__actions">
            <button
              class="btn btn-sm btn-primary"
              type="button"
              :disabled="busySessionId === session.sessionId"
              :data-test="`archive-restore-${session.sessionId}`"
              @click="restoreSession(session.sessionId)"
            >
              {{
                busySessionId === session.sessionId
                  ? t("common.loading")
                  : t("settings.archiveRestore")
              }}
            </button>
            <button
              class="btn btn-sm btn-danger"
              type="button"
              :disabled="busySessionId === session.sessionId"
              :data-test="`archive-delete-${session.sessionId}`"
              @click="permanentlyDelete(session.sessionId)"
            >
              {{ t("settings.archivePermanentDelete") }}
            </button>
          </div>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.archive-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.archive-stats {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.archive-list {
  display: grid;
  gap: 12px;
}
.archive-row__body {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}
.archive-row__main {
  min-width: 0;
}
.archive-row__main h4 {
  margin: 0 0 4px;
}
.archive-row__meta {
  display: flex;
  gap: 12px;
  color: var(--app-text-color-2);
  font-size: 0.82rem;
}
.archive-row__actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-shrink: 0;
}

.archive-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
