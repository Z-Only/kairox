<script setup lang="ts">
import type { SkillCatalogEntry } from "@/generated/commands";

const props = defineProps<{
  entry: SkillCatalogEntry;
  installing: boolean;
  installed: boolean;
}>();
const emit = defineEmits<{ install: []; select: [] }>();
const { t } = useI18n();

const securityTone = computed<"neutral" | "success" | "warning" | "error">(() => {
  const score = props.entry.security_score;
  if (score == null) return "neutral";
  if (score >= 90) return "success";
  if (score >= 70) return "warning";
  return "error";
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

const installCountDisplay = computed<string>(() =>
  props.entry.install_count.toLocaleString(undefined, { maximumFractionDigits: 0 })
);

const installButtonLabel = computed<string>(() => {
  if (props.installed) return t("skills.installed");
  if (props.installing) return t("skills.installing");
  return t("skills.install");
});
</script>

<template>
  <div class="card catalog-card" data-test="skill-catalog-card">
    <button class="card-body card-body-btn" type="button" @click="emit('select')">
      <div class="card-head">
        <span class="display-name">{{ entry.name }}</span>
        <KxBadge
          v-if="entry.security_score != null"
          class="security-badge"
          :tone="securityTone"
          :title="t('skills.securityScore', { score: entry.security_score })"
        >
          {{ entry.security_score }}
        </KxBadge>
      </div>
      <span class="summary">{{ entry.description || t("skills.noDescription") }}</span>
      <div class="meta-row">
        <span v-if="entry.install_count != null" class="meta-item">
          {{ t("skills.installs", { count: installCountDisplay }) }}
        </span>
        <span v-if="starsDisplay" class="meta-item"> ★ {{ starsDisplay }} </span>
        <span v-if="ratingDisplay" class="meta-item">
          {{ ratingDisplay }}
        </span>
      </div>
      <div class="tags">
        <KxTag tone="info">{{ entry.source }}</KxTag>
      </div>
    </button>
    <div class="card-footer">
      <KxTag
        v-if="entry.source_url"
        as="a"
        tone="info"
        :href="entry.source_url"
        target="_blank"
        rel="noopener noreferrer"
      >
        {{ t("skills.viewSource") }}
      </KxTag>
      <KxButton
        variant="primary"
        size="sm"
        :disabled="installing || installed"
        :data-test="`skill-catalog-install-${entry.catalog_id}`"
        @click.stop="emit('install')"
      >
        {{ installButtonLabel }}
      </KxButton>
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

.card-body-btn {
  all: unset;
  box-sizing: border-box;
  width: 100%;
  cursor: pointer;
  padding: 12px;
  text-align: left;
}

.card-body-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  border-radius: 6px;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.display-name {
  font-weight: 600;
}

.security-badge {
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

.card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--app-border-color);
}
</style>
