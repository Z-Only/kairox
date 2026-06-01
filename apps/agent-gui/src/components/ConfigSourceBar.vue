<script setup lang="ts">
import { useProjectStore } from "@/stores/project";

const props = withDefaults(
  defineProps<{
    initialSource?: "user" | "project";
    initialProjectId?: string;
  }>(),
  {
    initialSource: "user",
    initialProjectId: undefined
  }
);

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

function selectAvailableProject(): string | undefined {
  const projects = projectStore.activeProjects;
  if (projects.length === 0) {
    return selectedProjectId.value || undefined;
  }

  const selectedStillExists = projects.some(
    (project) => project.projectId === selectedProjectId.value
  );
  if (!selectedProjectId.value || !selectedStillExists) {
    selectedProjectId.value = projects[0].projectId;
  }

  return selectedProjectId.value;
}

function onSourceChange(newSource: "user" | "project"): void {
  source.value = newSource;
  if (newSource === "user") {
    selectedProjectId.value = "";
    emit("source-change", newSource, undefined);
    return;
  }

  emit("source-change", newSource, selectAvailableProject());
}

function onProjectChange(): void {
  emit("source-change", "project", selectedProjectId.value);
}

watch(
  () => [props.initialSource, props.initialProjectId] as const,
  ([initialSource, initialProjectId]) => {
    source.value = initialSource;
    selectedProjectId.value = initialSource === "project" ? (initialProjectId ?? "") : "";
  },
  { immediate: true }
);

watch(
  () => projectStore.activeProjects.map((project) => project.projectId),
  () => {
    if (source.value !== "project") return;
    emit("source-change", "project", selectAvailableProject());
  }
);

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
              {{ opt.missing ? "Missing - " : "" }}{{ opt.label }}
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
  gap: 10px;
  padding: 2px 0;
  flex-wrap: wrap;
}
.segmented {
  display: inline-flex;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md);
  overflow: hidden;
  background: var(--app-card-color);
  box-shadow: var(--app-shadow-sm);
}
.segmented__btn {
  min-height: 30px;
  padding: 4px 12px;
  border: none;
  border-right: 1px solid var(--app-border-color);
  background: transparent;
  color: var(--app-text-color-2);
  font-size: var(--app-text-sm);
  font-weight: 650;
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
  color: var(--app-primary-contrast-color);
}
.segmented__btn:hover:not(.active) {
  background: var(--app-hover-color);
}
.config-source-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  background: color-mix(in srgb, var(--app-warning-color) 14%, var(--app-card-color));
  border: 1px solid color-mix(in srgb, var(--app-warning-color) 44%, transparent);
  border-radius: var(--app-radius-md);
  margin-bottom: 6px;
  font-size: var(--app-text-sm);
  color: var(--app-text-color);
}
</style>
