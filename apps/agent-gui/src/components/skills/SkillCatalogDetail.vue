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

function onOverlayClick(event: MouseEvent): void {
  if (event.target === event.currentTarget) {
    emit("close");
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="drawer-overlay" @click="onOverlayClick">
      <aside class="drawer" data-test="skill-catalog-detail">
        <header class="drawer-header">
          <span class="drawer-title">{{ entry.name }}</span>
          <button
            class="btn drawer-close-btn"
            type="button"
            :aria-label="t('common.close')"
            @click="emit('close')"
          >
            x
          </button>
        </header>

        <div class="drawer-body">
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
        </div>

        <footer class="drawer-footer">
          <button
            class="btn btn-primary btn-sm"
            type="button"
            :disabled="installing"
            data-test="skill-catalog-detail-install"
            @click="emit('install', entry)"
          >
            {{
              installing
                ? t("skills.installing")
                : t("skills.installToTarget", { target: targetLabel })
            }}
          </button>
          <button class="btn btn-sm" type="button" @click="emit('close')">
            {{ t("common.close") }}
          </button>
        </footer>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.drawer-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--app-z-modal);
  background: var(--app-backdrop-color);
}

.drawer {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: 460px;
  max-width: 90vw;
  display: flex;
  flex-direction: column;
  background: var(--app-body-color);
  box-shadow: var(--app-shadow-2);
}

.drawer-header,
.drawer-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-color: var(--app-border-color);
}

.drawer-header {
  justify-content: space-between;
  border-bottom: 1px solid var(--app-border-color);
}

.drawer-footer {
  border-top: 1px solid var(--app-border-color);
}

.drawer-title {
  min-width: 0;
  overflow: hidden;
  color: var(--app-text-color);
  font-size: 16px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-close-btn {
  padding: 2px 8px;
  line-height: 1.2;
}

.drawer-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

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
