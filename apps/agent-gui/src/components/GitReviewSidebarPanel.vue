<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import type { ProjectGitDiffSectionInfo } from "@/stores/project";
import DiffPreview from "@/components/chat/DiffPreview.vue";

const { t } = useI18n();
const workspaceUi = useWorkspaceUiStore();

const gitReviewSections = computed<ProjectGitDiffSectionInfo[]>(() => {
  const review = workspaceUi.gitReview;
  if (!review) return [];
  return [review.staged, review.unstaged, review.untracked].filter(
    (section): section is ProjectGitDiffSectionInfo => Boolean(section)
  );
});
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
        class="git-review-files"
        data-test="git-review-files"
      >
        <span class="git-review-files__label">{{ t("chat.gitReview.changedFiles") }}</span>
        <ul>
          <li v-for="file in workspaceUi.gitReview.changedFiles" :key="file">
            {{ file }}
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
          <h3>{{ section.label }}</h3>
          <pre v-if="section.stat" class="git-review-stat">{{ section.stat }}</pre>
          <DiffPreview v-if="section.diff" :text="section.diff" />
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
.git-review-sections {
  display: grid;
  gap: 10px;
}
.git-review-section h3 {
  margin: 0 0 4px;
  color: var(--app-text-color);
  font-size: 12px;
  font-weight: 700;
}
.git-review-stat {
  margin: 0 0 4px;
  padding: 6px 8px;
  border-radius: 4px;
  background: var(--app-code-bg);
  color: var(--app-text-color-2);
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
}
.git-review-section :deep(.diff-preview) {
  max-height: none;
}
</style>
