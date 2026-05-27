---
layout: home
title: Kairox
titleTemplate: 本地优先的 AI Agent 工作台
hero:
  name: Kairox
  text: 本地优先的 AI Agent 工作台
  tagline: 通过共享 Rust 核心、终端界面和 Tauri 桌面 GUI，在本机运行可观测、可授权、可扩展的 AI Agent 工作流。
  image:
    src: /logo.svg
    alt: Kairox logo
  actions:
    - theme: brand
      text: 快速开始
      link: /zh/guide/getting-started
    - theme: alt
      text: 查看 GitHub
      link: https://github.com/Z-Only/kairox
features:
  - title: 本地优先运行时
    details: 会话、事件、权限、记忆和工具通过共享 Rust 核心协同工作，默认强调本地可控。
  - title: 两种高效界面
    details: 终端用户可以使用 ratatui TUI，桌面用户可以使用 Tauri + Vue GUI 管理会话、追踪和设置。
  - title: 可扩展 Agent 栈
    details: 原生技能、插件、MCP 服务器、模型路由、Hooks 和项目配置都是一等能力。
---

<script setup>
import { onMounted } from "vue";
import { withBase } from "vitepress";

onMounted(() => {
  localStorage.setItem("kairox.site.locale", "zh");
});
</script>

## 查看 Kairox

<div class="screenshot-grid">
  <figure>
    <img :src="withBase('/screenshots/workbench.png')" alt="Kairox 桌面工作台，包含会话、聊天、追踪和任务面板" />
    <figcaption>桌面工作台将持久会话、聊天、追踪和任务上下文集中在一个视图中。</figcaption>
  </figure>
  <figure>
    <img :src="withBase('/screenshots/settings.png')" alt="Kairox 设置界面，展示模型和 Agent 配置" />
    <figcaption>设置页覆盖模型、Agent、MCP、技能、插件、Hooks 和项目指令。</figcaption>
  </figure>
</div>

## 适合什么场景

<div class="kairox-link-grid">
  <a class="kairox-link-card" :href="withBase('/zh/guide/architecture')">
    <strong>理解架构</strong>
    <span>了解 facade 驱动的 Rust 核心如何连接运行时、工具、记忆、MCP 和 UI。</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/releases/latest">
    <strong>下载最新版本</strong>
    <span>通过 GitHub Releases 获取已发布的桌面构建和发布产物。</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/discussions">
    <strong>参与项目讨论</strong>
    <span>使用 GitHub Discussions 讨论产品方向、集成问题和设计提案。</span>
  </a>
</div>
