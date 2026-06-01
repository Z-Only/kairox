---
layout: home
title: Kairox
titleTemplate: 本地优先的 AI Agent 工作台
hero:
  name: Kairox
  text: 本地优先的 AI Agent 工作台
  tagline: 共享的 Rust 核心、终端 UI 与 Tauri 桌面 GUI，让你在本机构建可观测、可授权的 AI Agent 工作流。
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
    details: 每一个 session、tool 调用与 permission 决策都是 SQLite 中的 event。随时重启 —— 没有任何东西只活在内存里。
  - title: TUI 与桌面 GUI 共享同一内核
    details: 用 ratatui TUI 进行高效键盘操作，或用 Tauri + Vue 桌面应用获得持久会话、trace 时间线和设置 —— 两者背后是同一个 Rust runtime。
  - title: 带 permission 的 tool 与 MCP
    details: 正交的 Approval × Sandbox 策略引擎管控每一次 tool 调用 —— `ApprovalPolicy` 决定何时询问用户，`SandboxPolicy` 决定 runtime 在结构上允许什么。内置的 shell / 文件系统 / 搜索工具，加上经过整理的 MCP marketplace，让能力既可组合又可审计。
  - title: 为扩展而生
    details: 原生 skill、plugin、模型路由、hook 与按 workspace 的配置都是一等公民。带上你自己的模型和工具即可上手。
---

<script setup>
import { withBase } from "vitepress";
</script>

## 一览 Kairox

<div class="screenshot-grid">
  <ThemeScreenshot
    light="/screenshots/workbench.png"
    dark="/screenshots/workbench-dark.png"
    alt="Kairox 桌面工作台，展示项目会话、聊天、trace 与任务面板"
    caption="桌面工作台将项目会话、聊天、trace 与任务上下文集中在一个视图中。"
  />
  <ThemeScreenshot
    light="/screenshots/settings.png"
    dark="/screenshots/settings-dark.png"
    alt="Kairox 设置界面，展示模型与 agent 配置"
    caption="覆盖 model、agent、MCP、skill、plugin、hook 与项目指令的设置面板。"
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
    <span>无需 fork runtime，即可新增模型、工具、能力与工作流。</span>
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
