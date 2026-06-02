<script setup lang="ts">
import { commands } from "@/generated/commands";
import { useConfirm } from "@/composables/useConfirm";
import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";
import type { ProjectSessionInfo } from "@/stores/project";
import SettingsItemMeta from "@/components/ui/SettingsItemMeta.vue";
import SettingsItemSummary from "@/components/ui/SettingsItemSummary.vue";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";

type ArchiveSortMode = "original" | "recent" | "title" | "project" | "branch";

const { t } = useI18n();
const { confirm: confirmAction } = useConfirm();
const projectStore = useProjectStore();
const sessionStore = useSessionStore();
const loading = ref(false);
const error = ref<string | null>(null);
const busySessionId = ref<string | null>(null);
const archiveSearchQuery = ref("");
const archiveSortMode = ref<ArchiveSortMode>("original");
const archiveSortOptions: { value: ArchiveSortMode; label: string }[] = [
  { value: "original", label: "Original order" },
  { value: "recent", label: "Recently archived" },
  { value: "title", label: "Title" },
  { value: "project", label: "Project" },
  { value: "branch", label: "Branch" }
];

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

const normalizedArchiveSearchQuery = computed(() => archiveSearchQuery.value.trim().toLowerCase());

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

function searchableArchiveText(session: ProjectSessionInfo): string {
  return [
    session.sessionId,
    session.title,
    session.profile,
    getProjectDisplayName(session),
    session.projectId,
    session.worktreePath,
    session.branch,
    session.visibility,
    session.deletedAt,
    session.deletedAt ? formatArchivedAt(session.deletedAt) : null
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

const filteredArchivedSessions = computed(() => {
  const query = normalizedArchiveSearchQuery.value;
  if (!query) return projectStore.archivedSessions;
  return projectStore.archivedSessions.filter((session) =>
    searchableArchiveText(session).includes(query)
  );
});

function compareArchiveText(first: string | null | undefined, second: string | null | undefined) {
  const firstValue = first?.trim() ?? "";
  const secondValue = second?.trim() ?? "";
  if (!firstValue && !secondValue) return 0;
  if (!firstValue) return 1;
  if (!secondValue) return -1;
  return firstValue.localeCompare(secondValue, undefined, { numeric: true, sensitivity: "base" });
}

function archiveSortTimestamp(value: string | null): number {
  if (!value) return Number.NEGATIVE_INFINITY;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : Number.NEGATIVE_INFINITY;
}

function compareArchivedSessions(
  first: ProjectSessionInfo,
  second: ProjectSessionInfo,
  mode: ArchiveSortMode
): number {
  switch (mode) {
    case "recent":
      return archiveSortTimestamp(second.deletedAt) - archiveSortTimestamp(first.deletedAt);
    case "title":
      return compareArchiveText(first.title, second.title);
    case "project":
      return compareArchiveText(getProjectDisplayName(first), getProjectDisplayName(second));
    case "branch":
      return compareArchiveText(first.branch, second.branch);
    case "original":
      return 0;
  }
}

const displayedArchivedSessions = computed(() => {
  const sessions = filteredArchivedSessions.value;
  if (archiveSortMode.value === "original") return sessions;
  return sessions
    .map((session, index) => ({ session, index }))
    .sort((first, second) => {
      const sortResult = compareArchivedSessions(
        first.session,
        second.session,
        archiveSortMode.value
      );
      return sortResult || first.index - second.index;
    })
    .map(({ session }) => session);
});

function setArchiveSortMode(value: string): void {
  if (archiveSortOptions.some((option) => option.value === value)) {
    archiveSortMode.value = value as ArchiveSortMode;
  }
}

async function restoreSession(sessionId: string): Promise<void> {
  busySessionId.value = sessionId;
  error.value = null;
  try {
    await commands.restoreArchivedSession(sessionId);
    await Promise.all([projectStore.loadArchivedSessions(), sessionStore.loadSessions()]);
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
  <section
    class="archive-settings"
    :aria-label="t('settings.archive')"
    data-test="archive-settings-pane"
  >
    <SettingsState v-if="error" tone="error" data-test="archive-page-error">
      {{ error }}
    </SettingsState>

    <div v-if="stats.total > 0" class="archive-stats" data-test="archive-stats">
      <SettingsStatusTag>{{
        t("settings.archiveTotal", { count: stats.total })
      }}</SettingsStatusTag>
      <SettingsStatusTag>
        {{ t("settings.archiveProjects", { count: stats.projects }) }}
      </SettingsStatusTag>
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

    <template v-else>
      <SettingsFilterBar aria-label="Search archived sessions" data-test="archive-filters">
        <div class="settings-filter-bar__row">
          <KxInput
            v-model="archiveSearchQuery"
            type="search"
            size="compact"
            aria-label="Search archived sessions"
            placeholder="Search archived sessions"
            data-test="archive-search-input"
          />
          <KxSelect
            :model-value="archiveSortMode"
            aria-label="Archived session sort"
            data-test="archive-sort-select"
            size="compact"
            @update:model-value="setArchiveSortMode"
          >
            <option v-for="option in archiveSortOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </KxSelect>
        </div>
      </SettingsFilterBar>

      <SettingsState
        v-if="displayedArchivedSessions.length === 0"
        tone="empty"
        data-test="archive-filter-empty"
      >
        No archived sessions match your search.
      </SettingsState>

      <SettingsCardList
        v-else
        :aria-label="t('settings.archiveSessions')"
        data-test="archive-list"
        :scroll="false"
        columns="auto"
        dense
      >
        <SettingsCardItem
          v-for="session in displayedArchivedSessions"
          :key="session.sessionId"
          class="archive-row"
          :data-test="`archive-row-${session.sessionId}`"
        >
          <SettingsItemSummary :title="session.title" :heading-level="4">
            <SettingsItemMeta as="div" compact wrap-values>
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
            </SettingsItemMeta>
          </SettingsItemSummary>

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
    </template>
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
.archive-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
