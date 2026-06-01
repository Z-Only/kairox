<script setup lang="ts">
import { onMounted, watchEffect } from "vue";
import { inBrowser, useData, withBase } from "vitepress";

const preferenceKey = "kairox.site.locale";
const supportedLocales = new Set(["en", "zh"]);

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
    if (locale && supportedLocales.has(locale)) {
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

const { lang } = useData();

onMounted(() => {
  if (!inBrowser || !isRootPath(window.location.pathname)) return;

  const saved = localStorage.getItem(preferenceKey);
  const locale = saved === "zh" || saved === "en" ? saved : preferredLocale();
  localStorage.setItem(preferenceKey, locale);
  if (locale === "zh") {
    window.location.replace(`${withBase("/zh/")}${window.location.search}${window.location.hash}`);
  }
});

watchEffect(() => {
  if (!inBrowser) return;
  if (isRootPath(window.location.pathname)) return;
  localStorage.setItem(preferenceKey, lang.value.startsWith("zh") ? "zh" : "en");
});
</script>

<template></template>
