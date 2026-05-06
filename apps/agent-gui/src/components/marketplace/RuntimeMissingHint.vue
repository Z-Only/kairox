<script setup lang="ts">
import { NText } from "naive-ui";

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
      <NText strong>{{ r.kind }}</NText>
      <NText v-if="r.min_version" depth="3"> ({{ r.min_version }}) </NText>
      <NText v-if="r.install_hint" type="info">
        —
        <a :href="r.install_hint" target="_blank" rel="noopener">install</a>
      </NText>
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
