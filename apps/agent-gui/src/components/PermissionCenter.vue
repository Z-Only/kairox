<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";
import type { TraceEntryData } from "../types/trace";

type PendingRequestFilter = "all" | "tool" | "memory";
type PendingRequestSort = "original" | "type" | "title" | "toolOrScope";

const { t } = useI18n();
const selectedFilter = ref<PendingRequestFilter>("all");
const selectedSort = ref<PendingRequestSort>("original");
const searchQuery = ref("");
const normalizedSearchQuery = computed(() => searchQuery.value.trim().toLowerCase());

// Memoise the pending-entries filter so the template doesn't recompute it
// on every render and so the `v-if`/`v-for` agree on the same source.
const pendingEntries = computed(() =>
  traceState.entries.filter(
    (e) => (e.kind === "permission" || e.kind === "memory") && e.status === "pending"
  )
);

function requestMatchesFilter(entry: TraceEntryData, filter: PendingRequestFilter) {
  switch (filter) {
    case "tool":
      return entry.kind === "permission";
    case "memory":
      return entry.kind === "memory";
    default:
      return true;
  }
}

function requestMatchesSearch(entry: TraceEntryData, query: string) {
  if (!query) return true;
  return [
    entry.id,
    entry.kind,
    entry.title,
    entry.toolId,
    entry.input,
    entry.reason,
    entry.scope,
    entry.content
  ].some((value) => value?.toLowerCase().includes(query));
}

function sortKeyForEntry(entry: TraceEntryData, sort: PendingRequestSort) {
  switch (sort) {
    case "type":
      return entry.kind;
    case "title":
      return entry.title ?? "";
    case "toolOrScope":
      return entry.kind === "memory" ? (entry.scope ?? "") : (entry.toolId ?? "");
    default:
      return "";
  }
}

function compareEntries(
  a: TraceEntryData,
  b: TraceEntryData,
  sort: PendingRequestSort,
  originalIndexById: Map<string, number>
) {
  if (sort !== "original") {
    const aKey = sortKeyForEntry(a, sort).toLowerCase();
    const bKey = sortKeyForEntry(b, sort).toLowerCase();
    const keyCompare = aKey.localeCompare(bKey);
    if (keyCompare !== 0) return keyCompare;
  }

  return (originalIndexById.get(a.id) ?? 0) - (originalIndexById.get(b.id) ?? 0);
}

const filterOptions = computed<{ id: PendingRequestFilter; label: string; count: number }[]>(() => [
  { id: "all", label: t("permission.filterAll"), count: pendingEntries.value.length },
  {
    id: "tool",
    label: t("permission.filterTools"),
    count: pendingEntries.value.filter((entry) => requestMatchesFilter(entry, "tool")).length
  },
  {
    id: "memory",
    label: t("permission.filterMemories"),
    count: pendingEntries.value.filter((entry) => requestMatchesFilter(entry, "memory")).length
  }
]);

const sortOptions: { id: PendingRequestSort; label: string }[] = [
  { id: "original", label: "Original order" },
  { id: "type", label: "Type" },
  { id: "title", label: "Title" },
  { id: "toolOrScope", label: "Tool or scope" }
];

const visibleEntries = computed(() => {
  const originalIndexById = new Map(pendingEntries.value.map((entry, index) => [entry.id, index]));
  const filteredEntries = pendingEntries.value.filter(
    (entry) =>
      requestMatchesFilter(entry, selectedFilter.value) &&
      requestMatchesSearch(entry, normalizedSearchQuery.value)
  );

  return filteredEntries
    .slice()
    .sort((a, b) => compareEntries(a, b, selectedSort.value, originalIndexById));
});
</script>

<template>
  <div
    :class="[
      'card',
      'permission-center',
      { 'permission-center--scrollable': pendingEntries.length > 0 }
    ]"
  >
    <div class="card-header">
      <h2>{{ t("permission.panelTitle") }}</h2>
    </div>
    <div class="card-content">
      <KxEmptyState
        v-if="pendingEntries.length === 0"
        class="permission-empty"
        data-test="permission-empty-state"
        compact
      >
        {{ t("permission.emptyState") }}
      </KxEmptyState>
      <template v-else>
        <SettingsFilterBar
          class="permission-filters"
          aria-label="Search pending requests"
          data-test="permission-filters"
        >
          <div class="settings-filter-bar__row">
            <KxChipGroup
              class="permission-type-filters"
              aria-label="Pending request filters"
              data-test="permission-type-filters"
            >
              <KxChipButton
                v-for="option in filterOptions"
                :key="option.id"
                size="compact"
                :selected="selectedFilter === option.id"
                :data-test="`permission-filter-${option.id}`"
                @click="selectedFilter = option.id"
              >
                {{ option.label }} {{ option.count }}
              </KxChipButton>
            </KxChipGroup>
            <KxInput
              v-model="searchQuery"
              type="search"
              aria-label="Search pending requests"
              placeholder="Search pending requests"
              data-test="permission-search-input"
              class="permission-search-input"
              size="compact"
            />
            <KxSelect
              v-model="selectedSort"
              aria-label="Pending request sort"
              data-test="permission-sort-select"
              class="permission-sort-select"
              size="compact"
            >
              <option v-for="option in sortOptions" :key="option.id" :value="option.id">
                {{ option.label }}
              </option>
            </KxSelect>
          </div>
        </SettingsFilterBar>
        <KxEmptyState
          v-if="visibleEntries.length === 0"
          class="permission-empty permission-filter-empty"
          data-test="permission-empty-state"
          compact
        >
          {{ t("permission.filteredEmptyState") }}
        </KxEmptyState>
        <ul v-else class="permission-list">
          <li v-for="entry in visibleEntries" :key="entry.id" class="permission-list-item">
            <PermissionPrompt :entry="entry" />
          </li>
        </ul>
      </template>
    </div>
  </div>
</template>

<style scoped>
.permission-center {
  box-sizing: border-box;
  width: 100%;
  max-width: 100%;
  border-top: 1px solid var(--app-border-color, #d7d7d7);
  max-height: 260px;
  overflow-x: hidden;
  overflow-y: hidden;
}
.permission-center--scrollable {
  overflow-y: auto;
}
.card-header {
  padding: 12px 12px 4px;
}
.card-content {
  box-sizing: border-box;
  padding: 4px 12px 12px;
  max-width: 100%;
}
.permission-center h2 {
  margin: 0;
  font-size: 14px;
}
.permission-empty {
  font-size: 13px;
}
.permission-filters {
  margin-bottom: 8px;
}
.permission-type-filters {
  flex: 1 1 auto;
}
.permission-search-input {
  flex: 1 1 180px;
}
.permission-sort-select {
  flex: 0 1 150px;
}
.permission-list {
  list-style: none;
  padding: 0;
  margin: 0;
  max-width: 100%;
}
.permission-list-item {
  padding: 4px 0;
}
.permission-list-item:hover {
  background: var(--app-hover-color, #f0f4f8);
}
</style>
