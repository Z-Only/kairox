<script setup lang="ts">
import type { SkillCatalogEntry, SkillInstallTarget } from "@/generated/commands";

const props = defineProps<{
  entry: SkillCatalogEntry;
  installTarget: SkillInstallTarget;
  installing: boolean;
}>();

const emit = defineEmits<{ close: []; install: [entry: SkillCatalogEntry] }>();
const { t } = useI18n();

const targetLabel = computed(() =>
  props.installTarget === "project" ? t("skills.targetProject") : t("skills.targetUser")
);
</script>

<template>
  <KxDrawer
    :title="entry.name"
    :close-label="t('common.close')"
    panel-data-test="skill-catalog-detail"
    width="460px"
    @close="emit('close')"
  >
    <div class="detail-stack">
      <p class="description">{{ entry.description || t("skills.noDescription") }}</p>

      <dl class="meta-grid">
        <div>
          <dt>{{ t("skills.source") }}</dt>
          <dd>{{ entry.source }}</dd>
        </div>
        <div>
          <dt>{{ t("skills.installTarget") }}</dt>
          <dd>{{ targetLabel }}</dd>
        </div>
        <div v-if="entry.install_count != null">
          <dt>{{ t("skills.downloads") }}</dt>
          <dd>{{ entry.install_count.toLocaleString() }}</dd>
        </div>
        <div v-if="entry.github_stars != null">
          <dt>{{ t("skills.stars") }}</dt>
          <dd>{{ entry.github_stars.toLocaleString() }}</dd>
        </div>
        <div v-if="entry.security_score != null">
          <dt>{{ t("skills.security") }}</dt>
          <dd>{{ entry.security_score }}</dd>
        </div>
        <div v-if="entry.rating != null">
          <dt>{{ t("skills.rating") }}</dt>
          <dd>{{ entry.rating.toFixed(1) }}</dd>
        </div>
      </dl>

      <div class="detail-links">
        <a
          v-if="entry.source_url"
          :href="entry.source_url"
          target="_blank"
          rel="noopener noreferrer"
        >
          {{ t("skills.viewSource") }}
        </a>
        <a
          v-if="entry.package_url"
          :href="entry.package_url"
          target="_blank"
          rel="noopener noreferrer"
        >
          {{ t("skills.downloadPackage") }}
        </a>
      </div>
    </div>

    <template #footer>
      <KxButton
        variant="primary"
        size="sm"
        :disabled="installing"
        data-test="skill-catalog-detail-install"
        @click="emit('install', entry)"
      >
        {{
          installing ? t("skills.installing") : t("skills.installToTarget", { target: targetLabel })
        }}
      </KxButton>
      <KxButton size="sm" @click="emit('close')">
        {{ t("common.close") }}
      </KxButton>
    </template>
  </KxDrawer>
</template>

<style scoped>
.detail-stack {
  display: grid;
  gap: 14px;
}

.description {
  margin: 0;
  color: var(--app-text-color-2);
  line-height: 1.5;
}

.meta-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
  gap: 10px;
  margin: 0;
}

.meta-grid dt {
  color: var(--app-text-color-3);
  font-size: 12px;
  font-weight: 600;
}

.meta-grid dd {
  margin: 2px 0 0;
  overflow-wrap: anywhere;
}

.detail-links {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
</style>
