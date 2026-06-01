<script setup lang="ts">
import { withBase } from "vitepress";

defineProps<{
  light: string;
  dark: string;
  alt: string;
  caption?: string;
}>();
</script>

<template>
  <figure class="theme-screenshot">
    <img
      class="theme-screenshot__image theme-screenshot__image--light"
      :src="withBase(light)"
      :alt="alt"
    />
    <img
      class="theme-screenshot__image theme-screenshot__image--dark"
      :src="withBase(dark)"
      :alt="alt"
    />
    <figcaption v-if="caption">{{ caption }}</figcaption>
  </figure>
</template>
