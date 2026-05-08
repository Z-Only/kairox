<script setup lang="ts">
defineProps<{
  requirements: Array<{
    kind: string;
    min_version: string | null;
    install_hint: string | null;
  }>;
}>();
</script>

<template>
  <!-- The Requirements list inside CatalogDetail is a static, low-density
       hint rather than a blocking alert; an NAlert wrapper would dominate
       the surrounding form. We keep the semantic <ul><li> markup (the test
       suite asserts `findAll("li")`) and lean on NText for theme-aware
       coloring of the install hint link. -->
  <ul class="hint" data-test="runtime-hint">
    <li v-for="r in requirements" :key="r.kind">
      <strong>{{ r.kind }}</strong>
      <span v-if="r.min_version" :style="{ color: 'var(--app-text-color-3)' }">
        ({{ r.min_version }})
      </span>
      <span v-if="r.install_hint" :style="{ color: 'var(--app-info-color)' }">
        —
        <a :href="r.install_hint" target="_blank" rel="noopener">install</a>
      </span>
    </li>
  </ul>
</template>

<style scoped>
.hint {
  list-style: disc;
  margin: 0;
  padding-left: 18px;
}
.hint li {
  display: flex;
  gap: 4px;
  align-items: baseline;
}
</style>
