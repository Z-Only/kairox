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
  - title: Event-sourced local runtime
    details: Every session, tool call, and permission decision is an event in a SQLite store. Restart any time — nothing is held only in memory.
  - title: TUI and desktop GUI on one core
    details: Use the ratatui TUI for fast keyboard workflows or the Tauri + Vue desktop app for persistent sessions, trace timelines, and settings — both speak to the same Rust runtime.
  - title: Permission-aware tools and MCP
    details: An orthogonal Approval × Sandbox policy engine gates every tool call — `ApprovalPolicy` decides when you are asked, `SandboxPolicy` decides what is structurally allowed. Built-in shell / filesystem / search / browser / computer-use tools and a curated MCP marketplace make capabilities composable and auditable.
  - title: Extensible by design
    details: Native skills, plugins, model routing, hooks, and per-workspace configuration are first-class. Bring your own model and your own tools.
---

<script setup>
import { withBase } from "vitepress";
</script>

## See Kairox

<div class="screenshot-grid">
  <ThemeScreenshot
    light="/screenshots/workbench.png"
    dark="/screenshots/workbench-dark.png"
    zhLight="/screenshots/zh/workbench.png"
    zhDark="/screenshots/zh/workbench-dark.png"
    alt="Kairox desktop workbench with a project session, chat, trace, and task panels"
    caption="Desktop workbench with a project-scoped session, chat, trace, and task context in one view."
  />
  <ThemeScreenshot
    light="/screenshots/settings.png"
    dark="/screenshots/settings-dark.png"
    zhLight="/screenshots/zh/settings.png"
    zhDark="/screenshots/zh/settings-dark.png"
    alt="Kairox settings screen showing model and agent configuration"
    caption="Settings surfaces for models, agents, MCP, skills, plugins, hooks, and project instructions."
  />
</div>

## Where to go next

<div class="kairox-link-grid">
  <a class="kairox-link-card" :href="withBase('/guide/getting-started')">
    <strong>Get started in 5 minutes</strong>
    <span>Clone, install, and open your first TUI or desktop session against a real model.</span>
  </a>
  <a class="kairox-link-card" :href="withBase('/concepts/architecture')">
    <strong>Understand the architecture</strong>
    <span>The facade-driven Rust core, the event stream, and how runtime, tools, memory, and MCP fit together.</span>
  </a>
  <a class="kairox-link-card" :href="withBase('/concepts/extensibility')">
    <strong>Extend with MCP, skills, plugins</strong>
    <span>Add models, tools, capabilities, and workflows without forking the runtime.</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/releases/latest">
    <strong>Download the latest release</strong>
    <span>Pre-built desktop binaries for macOS, Linux, and Windows with auto-update.</span>
  </a>
  <a class="kairox-link-card" :href="withBase('/community/contributing')">
    <strong>Contribute</strong>
    <span>How to propose a change, build locally, and land a PR — Kairox is built almost entirely from community PRs.</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/discussions">
    <strong>Join the discussion</strong>
    <span>Use GitHub Discussions for product direction, integration questions, and design proposals.</span>
  </a>
</div>
