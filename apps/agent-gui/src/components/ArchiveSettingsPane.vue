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
    if (s.deletedAt && (!recentDate || s.deletedAt > recentDate)) {
      recentDate = s.deletedAt;
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

function formatArchivedAt(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(new Date(value));
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
    <SettingsState v-if="error" tone="error" data-test="archive-page-error">
      {{ error }}
    </SettingsState>

    <div v-if="stats.total > 0" class="archive-stats" data-test="archive-stats">
      <span class="tag">{{ t("settings.archiveTotal", { count: stats.total }) }}</span>
      <span class="tag">{{ t("settings.archiveProjects", { count: stats.projects }) }}</span>
    </div>

    <SettingsState v-if="loading" tone="loading" data-test="archive-loading-state">
      {{ t("common.loading") }}
    </SettingsState>
    <SettingsState
      v-else-if="projectStore.archivedSessions.length === 0"
      tone="empty"
      data-test="archive-empty-state"
    >
      {{ t("settings.archiveEmpty") }}
    </SettingsState>

    <SettingsCardList
      v-else
      aria-label="Archived sessions"
      data-test="archive-list"
      :scroll="false"
      columns="auto"
      dense
    >
      <SettingsCardItem
        v-for="session in projectStore.archivedSessions"
        :key="session.sessionId"
        class="archive-row"
        :data-test="`archive-row-${session.sessionId}`"
      >
        <div class="archive-row__main">
          <h4>{{ session.title }}</h4>
          <p class="archive-row__meta">
            <span>{{ getProjectDisplayName(session) }}</span>
            <span v-if="session.profile">{{ session.profile }}</span>
            <span v-if="session.branch">{{ session.branch }}</span>
            <time
              v-if="session.deletedAt"
              :datetime="session.deletedAt"
              :data-test="`archive-time-${session.sessionId}`"
            >
              {{ t("settings.archiveArchivedAt", { time: formatArchivedAt(session.deletedAt) }) }}
            </time>
          </p>
        </div>

        <template #actions>
          <KxInlineAction
            variant="primary"
            :disabled="busySessionId === session.sessionId"
            :data-test="`archive-restore-${session.sessionId}`"
            @click="restoreSession(session.sessionId)"
          >
            {{
              busySessionId === session.sessionId
                ? t("common.loading")
                : t("settings.archiveRestore")
            }}
          </KxInlineAction>
          <KxInlineAction
            variant="danger"
            :disabled="busySessionId === session.sessionId"
            :data-test="`archive-delete-${session.sessionId}`"
            @click="permanentlyDelete(session.sessionId)"
          >
            {{ t("settings.archivePermanentDelete") }}
          </KxInlineAction>
        </template>
      </SettingsCardItem>
    </SettingsCardList>
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
.archive-row__main {
  min-width: 0;
}
.archive-row__main h4 {
  margin: 0 0 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.archive-row__meta {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  color: var(--app-text-color-2);
  font-size: 0.82rem;
}
.archive-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
