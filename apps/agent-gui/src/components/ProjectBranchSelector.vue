<script setup lang="ts">
import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";

const props = defineProps<{
  projectId: string;
  branch: string | null;
}>();

const projectStore = useProjectStore();
const session = useSessionStore();
const open = ref(false);
const search = ref("");
const branches = ref<string[]>([]);
const loading = ref(false);

const normalizedSearch = computed(() => search.value.trim().toLocaleLowerCase());
const filteredBranches = computed(() => {
  const query = normalizedSearch.value;
  if (!query) return branches.value;
  return branches.value.filter((branch) => branch.toLocaleLowerCase().includes(query));
});
const createCandidate = computed(() => {
  const branch = search.value.trim();
  if (!branch) return null;
  if (branches.value.some((existing) => existing === branch)) return null;
  return branch;
});
const activeBranchLabel = computed(() => props.branch ?? branches.value[0] ?? "branch");

function branchTestId(branch: string): string {
  return branch.replace(/[^a-zA-Z0-9_-]+/g, "-");
}

async function loadBranches(): Promise<void> {
  if (loading.value) return;
  loading.value = true;
  try {
    branches.value = await projectStore.listProjectBranches(props.projectId);
    if (!props.branch && branches.value[0]) {
      session.setPendingProjectBranch(branches.value[0]);
    }
  } catch (error) {
    console.error("Failed to load project branches:", error);
    branches.value = [];
  } finally {
    loading.value = false;
  }
}

function selectBranch(branch: string): void {
  session.setPendingProjectBranch(branch);
  open.value = false;
  search.value = "";
}

watch(open, (isOpen) => {
  if (isOpen) void loadBranches();
});

onMounted(() => {
  void loadBranches();
});
</script>

<template>
  <div class="project-branch-selector" data-test="project-branch-selector">
    <button
      type="button"
      class="project-branch-git-meta"
      data-test="session-git-meta"
      @click="open = !open"
    >
      {{ activeBranchLabel }}
    </button>
    <div v-if="open" class="project-branch-popover" data-test="project-branch-popover">
      <KxInput
        v-model="search"
        size="compact"
        data-test="project-branch-search"
        placeholder="Search branches"
        aria-label="Search branches"
      />
      <div class="project-branch-list">
        <button
          v-for="branch in filteredBranches"
          :key="branch"
          type="button"
          class="project-branch-option"
          :class="{ active: branch === props.branch }"
          :data-test="`project-branch-option-${branchTestId(branch)}`"
          @click="selectBranch(branch)"
        >
          {{ branch }}
        </button>
        <button
          v-if="createCandidate"
          type="button"
          class="project-branch-option project-branch-option--create"
          data-test="project-branch-create"
          @click="selectBranch(createCandidate)"
        >
          Create {{ createCandidate }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.project-branch-selector {
  position: relative;
  display: inline-flex;
  min-width: 0;
  max-width: min(100%, 420px);
  color: var(--app-muted-text-color, var(--app-text-color));
  font-size: 12px;
}

.project-branch-git-meta {
  display: inline-flex;
  min-width: 0;
  min-height: 22px;
  max-width: 100%;
  align-items: center;
  overflow: hidden;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
  opacity: 0.72;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-branch-git-meta:hover,
.project-branch-git-meta:focus-visible {
  color: var(--app-primary-color);
  opacity: 1;
  outline: none;
}

.project-branch-popover {
  position: absolute;
  z-index: 20;
  bottom: calc(100% + 6px);
  left: 0;
  width: min(280px, 80vw);
  padding: 8px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md, 6px);
  background: var(--app-card-color);
  box-shadow: 0 8px 22px rgba(0, 0, 0, 0.16);
}

.project-branch-list {
  display: flex;
  max-height: 180px;
  flex-direction: column;
  gap: 2px;
  margin-top: 6px;
  overflow-y: auto;
}

.project-branch-option {
  width: 100%;
  min-height: 28px;
  padding: 4px 6px;
  border: 0;
  border-radius: var(--app-radius-sm, 4px);
  background: transparent;
  color: var(--app-text-color);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  text-align: left;
}

.project-branch-option:hover,
.project-branch-option:focus-visible,
.project-branch-option.active {
  background: var(--app-muted-surface-color);
  outline: none;
}
</style>
