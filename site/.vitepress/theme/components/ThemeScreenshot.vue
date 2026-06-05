<script setup lang="ts">
import { computed } from "vue";
import { useData, withBase } from "vitepress";

const props = defineProps<{
  light: string;
  dark: string;
  zhLight?: string;
  zhDark?: string;
  alt: string;
  caption?: string;
}>();

const { lang } = useData();

const isZh = computed(() => lang.value.toLowerCase().startsWith("zh"));
const lightSrc = computed(() => (isZh.value && props.zhLight ? props.zhLight : props.light));
const darkSrc = computed(() => (isZh.value && props.zhDark ? props.zhDark : props.dark));
</script>

<template>
  <figure class="theme-screenshot">
    <img
      class="theme-screenshot__image theme-screenshot__image--light"
      :src="withBase(lightSrc)"
      :alt="alt"
    />
    <img
      class="theme-screenshot__image theme-screenshot__image--dark"
      :src="withBase(darkSrc)"
      :alt="alt"
    />
    <figcaption v-if="caption">{{ caption }}</figcaption>
  </figure>
</template>
