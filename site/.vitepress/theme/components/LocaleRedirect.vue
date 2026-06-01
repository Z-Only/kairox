<script setup lang="ts">
import { onMounted } from "vue";
import { inBrowser, withBase } from "vitepress";

function normalizeLocale(language: string): "en" | "zh" | null {
  const normalized = language.toLowerCase();
  if (normalized === "zh" || normalized.startsWith("zh-")) return "zh";
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
  return null;
}

function preferredLocale(): "en" | "zh" {
  const languages = [...(navigator.languages ?? []), navigator.language].filter(Boolean);

  for (const language of languages) {
    const locale = normalizeLocale(language);
    if (locale) {
      return locale;
    }
  }

  return "en";
}

function isRootPath(pathname: string): boolean {
  const base = withBase("/");
  const candidates = new Set(["/", "/index.html", base, `${base.replace(/\/$/, "")}/index.html`]);
  return candidates.has(pathname);
}

onMounted(() => {
  if (!inBrowser || !isRootPath(window.location.pathname)) return;

  if (preferredLocale() === "zh") {
    window.location.replace(`${withBase("/zh/")}${window.location.search}${window.location.hash}`);
  }
});
</script>

<template></template>
