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
  <ul class="hint" data-test="runtime-hint">
    <li v-for="r in requirements" :key="r.kind">
      <strong>{{ r.kind }}</strong>
      <span v-if="r.min_version"> ({{ r.min_version }})</span>
      <a
        v-if="r.install_hint"
        :href="r.install_hint"
        target="_blank"
        rel="noopener"
      >
        — install
      </a>
    </li>
  </ul>
</template>

<style scoped>
.hint {
  list-style: disc;
  margin: 0;
  padding-left: 18px;
}
</style>
