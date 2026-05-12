<script setup lang="ts">
import type { SkillCatalogEntry } from "@/generated/commands";

const props = defineProps<{ entry: SkillCatalogEntry; installing: boolean }>();
const emit = defineEmits<{ install: [] }>();

const trustTagClass = computed<string>(() => {
  const score = props.entry.security_score;
  if (score == null) return "";
  if (score >= 90) return "tag-success";
  if (score >= 70) return "tag-warning";
  return "tag-error";
});

const starsDisplay = computed<string>(() => {
  const s = props.entry.github_stars;
  if (s == null) return "";
  if (s >= 1000) return `${(s / 1000).toFixed(1)}k`;
  return String(s);
});

const ratingDisplay = computed<string>(() => {
  const r = props.entry.rating;
  if (r == null) return "";
  return r.toFixed(1);
});
</script>

<template>
  <div class="card catalog-card" data-test="skill-catalog-card">
    <div class="card-body">
      <div class="card-head">
        <span class="display-name">{{ entry.name }}</span>
        <span
          v-if="entry.security_score != null"
          class="tag sec-tag"
          :class="trustTagClass"
          :title="`Security score: ${entry.security_score}`"
        >
          {{ entry.security_score }}
        </span>
      </div>
      <span class="summary">{{ entry.description || "No description" }}</span>
      <div class="meta-row">
        <span v-if="entry.install_count != null" class="meta-item">
          {{ entry.install_count.toLocaleString() }} installs
        </span>
        <span v-if="starsDisplay" class="meta-item"> ★ {{ starsDisplay }} </span>
        <span v-if="ratingDisplay" class="meta-item">
          {{ ratingDisplay }}
        </span>
      </div>
      <div class="tags">
        <span class="tag tag-source">{{ entry.source }}</span>
        <a
          v-if="entry.source_url"
          :href="entry.source_url"
          target="_blank"
          rel="noopener noreferrer"
          class="tag tag-link"
        >
          View source
        </a>
      </div>
    </div>
    <div class="card-footer">
      <button
        class="btn btn-primary btn-sm"
        type="button"
        :disabled="installing"
        :data-test="`skill-catalog-install-${entry.catalog_id}`"
        @click="emit('install')"
      >
        {{ installing ? "Installing…" : "Install" }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.catalog-card {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.display-name {
  font-weight: 600;
}

.sec-tag {
  margin-left: auto;
}

.summary {
  font-size: 13px;
  color: var(--app-text-color-2);
}

.meta-row {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--app-text-color-3);
}

.meta-item {
  display: flex;
  align-items: center;
  gap: 2px;
}

.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-top: 4px;
}

.tag-source {
  background: var(--app-code-bg);
  color: var(--app-info-color);
}

.tag-link {
  text-decoration: none;
}

.tag-link:hover {
  text-decoration: underline;
}

.card-footer {
  padding-top: 8px;
  border-top: 1px solid var(--app-border-color);
}
</style>
