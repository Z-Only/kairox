---
layout: home
title: Kairox
titleTemplate: 本地优先的 AI Agent 工作台
hero:
  name: Kairox
  text: 本地优先的 AI Agent 工作台
  tagline: 共享的 Rust 核心、终端 UI、Tauri 桌面 GUI 与可嵌入 SDK，让你在本机构建可观测、可授权的 AI Agent 工作流。
  image:
    src: /logo.svg
    alt: Kairox logo
  actions:
    - theme: brand
      text: 快速开始
      link: /zh/guide/getting-started
    - theme: alt
      text: 在 GitHub 上查看
      link: https://github.com/Z-Only/kairox
features:
  - title: 事件溯源的本地 runtime
    details: 每一个 session、tool 调用、permission 决策、advisor review、autonomous checkpoint 与 trajectory step 都是 SQLite 中的 event。随时重启 —— UI 从事件日志重建。
  - title: TUI、桌面 GUI 与 SDK 共享同一内核
    details: 用 ratatui TUI 进行高效键盘操作，用 Tauri + Vue 桌面应用管理持久化工作台 session，或用 `agent-sdk` 把同一个 runtime 嵌入自己的 harness。
  - title: 带 permission 的多模态 tool 与 MCP
    details: Approval × Sandbox 策略管控每一次 tool 调用。内置 shell / 文件系统 / 搜索 / browser / computer-use 工具、结构化图片附件、LSP/DAP provider 与 MCP marketplace server 都可组合且可审计。
  - title: 为自主工作流扩展而生
    details: 原生 skill、plugin、模型路由、hook、advisor 自反检查、autonomous task checkpoint 与按 workspace 的配置都是一等公民。带上你自己的模型和工具即可上手。
---

<script setup>
import { withBase } from "vitepress";
</script>

## 一览 Kairox

<div class="screenshot-grid">
  <ThemeScreenshot
    light="/screenshots/workbench.png"
    dark="/screenshots/workbench-dark.png"
    zhLight="/screenshots/zh/workbench.png"
    zhDark="/screenshots/zh/workbench-dark.png"
    alt="Kairox 桌面工作台，展示项目会话、聊天、trace 与任务面板"
    caption="桌面工作台将项目 session、实时聊天、trace event、任务上下文、trajectory 状态和模型控制集中在一个视图中。"
  />
  <ThemeScreenshot
    light="/screenshots/trajectory.png"
    dark="/screenshots/trajectory-dark.png"
    zhLight="/screenshots/zh/trajectory.png"
    zhDark="/screenshots/zh/trajectory-dark.png"
    alt="Kairox trajectory viewer，展示展开后的已记录 tool step"
    caption="Trajectory viewer 用于查看可 replay 的 tool step，包括按顺序记录的 action input、observation output、耗时和 outcome 状态。"
  />
  <ThemeScreenshot
    light="/screenshots/settings.png"
    dark="/screenshots/settings-dark.png"
    zhLight="/screenshots/zh/settings.png"
    zhDark="/screenshots/zh/settings-dark.png"
    alt="Kairox 设置界面，展示模型与 agent 配置"
    caption="设置页把 model profile、作用域控制和相邻配置区域放在同一个 tabbed surface 中。"
  />
  <ThemeScreenshot
    light="/screenshots/autonomous.png"
    dark="/screenshots/autonomous-dark.png"
    zhLight="/screenshots/zh/autonomous.png"
    zhDark="/screenshots/zh/autonomous-dark.png"
    alt="Kairox autonomous task 设置页，展示活跃任务"
    caption="Autonomous task 控制页展示持久化目标、暂停 / 取消状态、session 预算和当前进度。"
  />
</div>

## 下一步去哪

<div class="kairox-link-grid">
  <a class="kairox-link-card" :href="withBase('/zh/guide/getting-started')">
    <strong>5 分钟上手</strong>
    <span>克隆、安装，并基于真实模型打开你的第一个 TUI 或桌面 session。</span>
  </a>
  <a class="kairox-link-card" :href="withBase('/zh/concepts/architecture')">
    <strong>理解架构</strong>
    <span>facade 驱动的 Rust 核心、event 流，以及 runtime、tool、memory 与 MCP 如何拼装在一起。</span>
  </a>
  <a class="kairox-link-card" :href="withBase('/zh/concepts/extensibility')">
    <strong>用 MCP、skill、plugin 扩展</strong>
    <span>无需 fork runtime，即可新增模型、工具、能力、advisor 策略与工作流。</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/releases/latest">
    <strong>下载最新版本</strong>
    <span>面向 macOS、Linux 与 Windows 的预编译桌面二进制，支持自动更新。</span>
  </a>
  <a class="kairox-link-card" :href="withBase('/zh/community/contributing')">
    <strong>参与贡献</strong>
    <span>如何提出改动、在本地构建并把 PR 合入 —— Kairox 几乎完全由社区 PR 构建。</span>
  </a>
  <a class="kairox-link-card" href="https://github.com/Z-Only/kairox/discussions">
    <strong>加入讨论</strong>
    <span>在 GitHub Discussions 中探讨产品方向、集成问题与设计提案。</span>
  </a>
</div>
