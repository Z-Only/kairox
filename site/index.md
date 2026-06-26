---
layout: home
title: Kairox
titleTemplate: Local-first AI agent workbench
hero:
  name: Kairox
  text: Local-first AI agent workbench
  tagline: A shared Rust core, terminal UI, Tauri desktop GUI, and embeddable SDK for observable, permission-aware AI agent workflows on your machine.
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
    details: Every session, tool call, permission decision, advisor review, autonomous checkpoint, and trajectory step is an event in SQLite. Restart any time — the UI rebuilds from the log.
  - title: TUI, desktop GUI, and SDK on one core
    details: Use the ratatui TUI for fast keyboard workflows, the Tauri + Vue desktop app for persistent workbench sessions, or `agent-sdk` to embed the same runtime in your own harness.
  - title: Permission-aware multimodal tools and MCP
    details: Approval × Sandbox policy gates every tool call. Built-in shell / filesystem / search / browser / computer-use tools, structured image attachments, LSP/DAP providers, and MCP marketplace servers stay composable and auditable.
  - title: Extensible autonomous workflows
    details: Native skills, plugins, model routing, hooks, advisor self-reflection, autonomous task checkpoints, and per-workspace configuration are first-class. Bring your own model and your own tools.
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
    caption="Desktop workbench with project sessions, live chat, trace events, task context, trajectory state, and model controls in one view."
  />
  <ThemeScreenshot
    light="/screenshots/trajectory.png"
    dark="/screenshots/trajectory-dark.png"
    zhLight="/screenshots/zh/trajectory.png"
    zhDark="/screenshots/zh/trajectory-dark.png"
    alt="Kairox trajectory viewer showing an expanded recorded tool step"
    caption="Trajectory viewer for replayable tool steps, including ordered action input, observation output, timing, and outcome state."
  />
  <ThemeScreenshot
    light="/screenshots/settings.png"
    dark="/screenshots/settings-dark.png"
    zhLight="/screenshots/zh/settings.png"
    zhDark="/screenshots/zh/settings-dark.png"
    alt="Kairox settings screen showing model and agent configuration"
    caption="Settings keeps model profiles, scope controls, and adjacent configuration areas visible from one tabbed surface."
  />
  <ThemeScreenshot
    light="/screenshots/autonomous.png"
    dark="/screenshots/autonomous-dark.png"
    zhLight="/screenshots/zh/autonomous.png"
    zhDark="/screenshots/zh/autonomous-dark.png"
    alt="Kairox autonomous task settings with an active task"
    caption="Autonomous task controls expose durable goals, pause/cancel state, session budgets, and current progress."
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
    <span>Add models, tools, capabilities, advisor policies, and workflows without forking the runtime.</span>
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
