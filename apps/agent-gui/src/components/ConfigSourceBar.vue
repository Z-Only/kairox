<script setup lang="ts">
import { useProjectStore } from "@/stores/project";

const emit = defineEmits<{
  (e: "source-change", source: "user" | "project", projectId?: string): void;
}>();

const { t } = useI18n();
const projectStore = useProjectStore();

const source = ref<"user" | "project">("user");
const selectedProjectId = ref<string>("");

const missingProjects = computed(() => projectStore.activeProjects.filter((p) => !p.pathExists));

const projectOptions = computed(() =>
  projectStore.activeProjects.map((p) => ({
    value: p.projectId,
    label: p.displayName,
    missing: !p.pathExists
  }))
);

function onSourceChange(newSource: "user" | "project"): void {
  source.value = newSource;
  if (newSource === "user") {
    selectedProjectId.value = "";
  } else if (projectStore.activeProjects.length > 0 && !selectedProjectId.value) {
    selectedProjectId.value = projectStore.activeProjects[0].projectId;
  }
  emit("source-change", newSource, newSource === "project" ? selectedProjectId.value : undefined);
}

function onProjectChange(): void {
  emit("source-change", "project", selectedProjectId.value);
}

onMounted(() => {
  void projectStore.loadProjects();
});
</script>

<template>
  <div>
    <div
      v-if="missingProjects.length > 0"
      class="config-source-banner"
      data-test="path-warning-banner"
    >
      <span>{{ t("settings.pathWarning", { count: missingProjects.length }) }}</span>
    </div>

    <div class="config-source-bar" data-test="config-source-bar">
      <div class="segmented" data-test="source-segmented">
        <button
          :class="['segmented__btn', { active: source === 'user' }]"
          data-test="source-btn-user"
          @click="onSourceChange('user')"
        >
          {{ t("settings.userConfig") }}
        </button>
        <button
          :class="['segmented__btn', { active: source === 'project' }]"
          data-test="source-btn-project"
          @click="onSourceChange('project')"
        >
          {{ t("settings.projectConfig") }}
        </button>
      </div>

      <template v-if="source === 'project'">
        <div class="select-wrapper">
          <KxSelect
            v-model="selectedProjectId"
            data-test="project-select"
            size="compact"
            @change="onProjectChange"
          >
            <option v-for="opt in projectOptions" :key="opt.value" :value="opt.value">
              {{ opt.missing ? "⚠ " : "" }}{{ opt.label }}
            </option>
          </KxSelect>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.config-source-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 2px 0;
  flex-wrap: wrap;
}
.segmented {
  display: inline-flex;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  overflow: hidden;
}
.segmented__btn {
  padding: 4px 12px;
  border: none;
  border-right: 1px solid var(--app-border-color);
  background: transparent;
  color: var(--app-text-color-2);
  font-size: 0.82rem;
  cursor: pointer;
  transition:
    background 0.15s,
    color 0.15s;
}
.segmented__btn:last-child {
  border-right: none;
}
.segmented__btn.active {
  background: var(--app-primary-color);
  color: #fff;
}
.segmented__btn:hover:not(.active) {
  background: var(--app-hover-color);
}
.config-source-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 12px;
  background: color-mix(in srgb, #f59e0b 15%, transparent);
  border: 1px solid color-mix(in srgb, #f59e0b 44%, transparent);
  border-radius: 6px;
  margin-bottom: 4px;
  font-size: 0.82rem;
  color: var(--app-text-color);
}
</style>
