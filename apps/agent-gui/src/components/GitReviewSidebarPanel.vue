<script setup lang="ts">
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import type { ProjectGitDiffSectionInfo, ProjectGitFileChangeInfo } from "@/stores/project";
import DiffPreview from "@/components/chat/DiffPreview.vue";

const { t } = useI18n();
const workspaceUi = useWorkspaceUiStore();

interface GitReviewFileSummary {
  path: string;
  additions: number;
  deletions: number;
}

const collapsedFileKeys = ref<Set<string>>(new Set());
const expandedContextKeys = ref<Set<string>>(new Set());

const gitReviewSections = computed<ProjectGitDiffSectionInfo[]>(() => {
  const review = workspaceUi.gitReview;
  if (!review) return [];
  return [review.staged, review.unstaged, review.untracked].filter(
    (section): section is ProjectGitDiffSectionInfo => Boolean(section)
  );
});

const gitReviewFileSummaries = computed<GitReviewFileSummary[]>(() => {
  const review = workspaceUi.gitReview;
  if (!review) return [];

  const summaries = new Map<string, GitReviewFileSummary>();
  for (const path of review.changedFiles) {
    summaries.set(path, { path, additions: 0, deletions: 0 });
  }

  for (const section of gitReviewSections.value) {
    for (const file of section.files) {
      const summary = summaries.get(file.path) ?? {
        path: file.path,
        additions: 0,
        deletions: 0
      };
      summary.additions += file.additions;
      summary.deletions += file.deletions;
      summaries.set(file.path, summary);
    }
  }

  return Array.from(summaries.values());
});

watch(
  () => workspaceUi.gitReview,
  () => {
    collapsedFileKeys.value = new Set();
    expandedContextKeys.value = new Set();
  }
);

function lineStats(additions: number, deletions: number): string {
  return `+${additions} -${deletions}`;
}

function sectionFiles(section: ProjectGitDiffSectionInfo): ProjectGitFileChangeInfo[] {
  if (section.files.length > 0) return section.files;
  if (!section.diff) return [];
  return [
    {
      path: section.label,
      additions: section.additions,
      deletions: section.deletions,
      diff: section.diff
    }
  ];
}

function fileKey(section: ProjectGitDiffSectionInfo, file: ProjectGitFileChangeInfo): string {
  return `${section.label}\u0000${file.path}`;
}

function isFileCollapsed(key: string): boolean {
  return collapsedFileKeys.value.has(key);
}

function toggleFile(key: string): void {
  const next = new Set(collapsedFileKeys.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  collapsedFileKeys.value = next;
}

function isContextExpanded(key: string): boolean {
  return expandedContextKeys.value.has(key);
}

function toggleContext(key: string): void {
  const next = new Set(expandedContextKeys.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  expandedContextKeys.value = next;
}
</script>

<template>
  <section class="git-review-sidebar" data-test="git-review-panel">
    <header class="git-review-sidebar__header">
      <div class="git-review-sidebar__title">
        <strong>{{ t("chat.gitReview.title") }}</strong>
        <span v-if="workspaceUi.gitReview?.branch" data-test="git-review-branch">
          {{ workspaceUi.gitReview.branch }}
        </span>
      </div>
      <button
        type="button"
        class="git-review-sidebar__icon-button"
        data-test="git-review-refresh"
        :aria-label="t('chat.gitReview.refresh')"
        :title="t('chat.gitReview.refresh')"
        :disabled="workspaceUi.gitReviewLoading"
        @click="workspaceUi.refreshGitReview"
      >
        ↻
      </button>
    </header>

    <div
      v-if="workspaceUi.gitReviewLoading"
      class="git-review-sidebar__state"
      data-test="git-review-loading"
    >
      {{ t("chat.gitReview.loading") }}
    </div>
    <div
      v-else-if="workspaceUi.gitReviewError"
      class="git-review-sidebar__state git-review-sidebar__state--error"
      data-test="git-review-error"
      role="alert"
    >
      {{ t("chat.gitReview.failed", { error: workspaceUi.gitReviewError }) }}
    </div>
    <div
      v-else-if="!workspaceUi.gitReview"
      class="git-review-sidebar__state"
      data-test="git-review-empty"
    >
      {{ t("chat.gitReview.noContext") }}
    </div>
    <template v-else>
      <div
        v-if="workspaceUi.gitReview.changedFiles.length"
        class="git-review-summary"
        data-test="git-review-summary"
      >
        <span data-test="git-review-file-count">
          {{
            t("chat.gitReview.fileCount", {
              count: workspaceUi.gitReview.fileCount
            })
          }}
        </span>
        <span class="git-review-line-stats" data-test="git-review-line-stats">
          {{ lineStats(workspaceUi.gitReview.additions, workspaceUi.gitReview.deletions) }}
        </span>
      </div>
      <div
        v-if="gitReviewFileSummaries.length"
        class="git-review-files"
        data-test="git-review-files"
      >
        <span class="git-review-files__label">{{ t("chat.gitReview.changedFiles") }}</span>
        <ul>
          <li v-for="file in gitReviewFileSummaries" :key="file.path">
            <span>{{ file.path }}</span>
            <span class="git-review-line-stats">{{
              lineStats(file.additions, file.deletions)
            }}</span>
          </li>
        </ul>
      </div>
      <div v-else class="git-review-sidebar__state" data-test="git-review-clean">
        {{ t("chat.gitReview.clean") }}
      </div>

      <div v-if="gitReviewSections.length" class="git-review-sections">
        <section
          v-for="section in gitReviewSections"
          :key="section.label"
          class="git-review-section"
          data-test="git-review-section"
        >
          <header class="git-review-section__header">
            <h3>{{ section.label }}</h3>
            <span class="git-review-line-stats">
              {{ lineStats(section.additions, section.deletions) }}
            </span>
          </header>
          <div class="git-review-file-changes">
            <article
              v-for="file in sectionFiles(section)"
              :key="fileKey(section, file)"
              class="git-review-file-change"
              data-test="git-review-file-change"
            >
              <button
                type="button"
                class="git-review-file-change__toggle"
                data-test="git-review-file-toggle"
                :aria-expanded="!isFileCollapsed(fileKey(section, file))"
                @click="toggleFile(fileKey(section, file))"
              >
                <span class="git-review-file-change__chevron" aria-hidden="true">
                  {{ isFileCollapsed(fileKey(section, file)) ? "›" : "⌄" }}
                </span>
                <span class="git-review-file-change__path">{{ file.path }}</span>
                <span class="git-review-line-stats">{{
                  lineStats(file.additions, file.deletions)
                }}</span>
              </button>
              <div
                v-if="!isFileCollapsed(fileKey(section, file))"
                class="git-review-file-change__body"
                data-test="git-review-file-diff"
              >
                <DiffPreview
                  v-if="file.diff"
                  :text="file.diff"
                  collapse-unmodified
                  :unmodified-expanded="isContextExpanded(fileKey(section, file))"
                  @toggle-unmodified="toggleContext(fileKey(section, file))"
                />
              </div>
            </article>
          </div>
        </section>
      </div>
    </template>
  </section>
</template>

<style scoped>
.git-review-sidebar {
  box-sizing: border-box;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 10px 12px 14px;
  background: var(--app-panel-color);
}
.git-review-sidebar__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
}
.git-review-sidebar__title {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  font-size: 13px;
}
.git-review-sidebar__title strong {
  font-weight: 700;
}
.git-review-sidebar__title span {
  color: var(--app-text-color-3);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.git-review-sidebar__icon-button {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--app-text-color-2);
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
}
.git-review-sidebar__icon-button:hover,
.git-review-sidebar__icon-button:focus-visible {
  outline: none;
  border-color: var(--app-border-color);
  background: var(--app-hover-color);
  color: var(--app-text-color);
}
.git-review-sidebar__icon-button:disabled {
  cursor: wait;
  opacity: 0.6;
}
.git-review-sidebar__state {
  color: var(--app-text-color-2);
  font-size: 12px;
  line-height: 1.5;
}
.git-review-sidebar__state--error {
  color: var(--app-error-color);
}
.git-review-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  color: var(--app-text-color-2);
  font-size: 12px;
}
.git-review-line-stats {
  color: var(--app-text-color-3);
  font-family:
    ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
  font-size: 11px;
  white-space: nowrap;
}
.git-review-files {
  display: grid;
  gap: 6px;
  margin-bottom: 10px;
  min-width: 0;
}
.git-review-files__label {
  color: var(--app-text-color-3);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
}
.git-review-files ul {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 6px;
  min-width: 0;
  margin: 0;
  padding: 0;
  list-style: none;
}
.git-review-files li {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  color: var(--app-text-color-2);
  font-family:
    ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.git-review-files li span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
}
.git-review-sections {
  display: grid;
  gap: 10px;
}
.git-review-section__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
}
.git-review-section__header h3 {
  margin: 0;
  color: var(--app-text-color);
  font-size: 12px;
  font-weight: 700;
}
.git-review-file-changes {
  display: grid;
  gap: 6px;
}
.git-review-file-change {
  min-width: 0;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  overflow: hidden;
}
.git-review-file-change__toggle {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-height: 30px;
  padding: 5px 8px;
  border: 0;
  background: transparent;
  color: var(--app-text-color);
  cursor: pointer;
  font: inherit;
  text-align: left;
}
.git-review-file-change__toggle:hover,
.git-review-file-change__toggle:focus-visible {
  outline: none;
  background: var(--app-hover-color);
}
.git-review-file-change__chevron {
  color: var(--app-text-color-3);
  font-size: 13px;
  line-height: 1;
}
.git-review-file-change__path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family:
    ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
  font-size: 11px;
}
.git-review-file-change__body {
  border-top: 1px solid var(--app-border-color);
  background: var(--app-panel-color);
}
.git-review-section :deep(.diff-preview) {
  margin: 0;
  border-radius: 0;
  max-height: none;
}
</style>
