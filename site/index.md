---
layout: home
title: Kairox
titleTemplate: Local-first AI agent workbench
hero:
  name: Kairox
  text: Local-first AI agent workbench
  tagline: A shared Rust core, terminal UI, and Tauri desktop GUI for building observable, permission-aware AI agent workflows on your machine.
  image:
    src: /logo.svg
    alt: Kairox logo
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/Z-Only/kairox
features:
  - title: Local-first runtime
    details: Sessions, events, permissions, memory, and tools are coordinated through a shared Rust core designed for explicit local control.
  - title: Two focused interfaces
    details: Use the fast ratatui TUI for terminal workflows or the Tauri + Vue desktop app for persistent sessions, traces, and settings.
  - title: Extensible agent stack
    details: Native skills, plugins, MCP servers, model routing, hooks, and project configuration are first-class pieces of the workbench.
---

<script setup>
import { onMounted } from "vue";
import { withBase } from "vitepress";

onMounted(() => {
  const preferenceKey = "kairox.site.locale";
  const language = (navigator.languages?.[0] || navigator.language || "en").toLowerCase();

  if (!localStorage.getItem(preferenceKey) && language.startsWith("zh")) {
    localStorage.setItem(preferenceKey, "zh");
    window.location.replace(withBase("/zh/"));
    return;
  }

  localStorage.setItem(preferenceKey, "en");
});
</script>

## See Kairox

<div class="screenshot-grid">
  <figure>
    <img :src="withBase('/screenshots/workbench.png')" alt="Kairox desktop workbench with sessions, chat, trace, and task panels" />
    <figcaption>Desktop workbench with persistent sessions, chat, trace, and task context in one view.</figcaption>
  </figure>
  <figure>
    <img :src="withBase('/screenshots/settings.png')" alt="Kairox settings screen showing model and agent configuration" />
    <figcaption>Settings surfaces for models, agents, MCP, skills, plugins, hooks, and project instructions.</figcaption>
  </figure>
</div>

## What it is built for

<div class="kairox-link-grid">
  <a class="kairox-link-card" :href="withBase('/guide/architecture')">
    <strong>Understand the architecture</strong>
    <span>See how the facade-driven Rust core connects runtime, tools, memory, MCP, and UI surfaces.</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/releases/latest">
    <strong>Download the latest release</strong>
    <span>Install published desktop builds and follow release artifacts from GitHub Releases.</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/discussions">
    <strong>Join project discussion</strong>
    <span>Use GitHub Discussions for product direction, integration questions, and design proposals.</span>
  </a>
</div>
