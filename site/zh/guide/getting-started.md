# 快速开始

Kairox 是本地优先的 AI Agent 工作台。仓库包含运行时所需的 Rust crates、终端 UI，以及 Tauri + Vue 桌面 GUI。

## 环境要求

- Rust stable 工具链
- Node.js 22 或更高版本
- Bun 1.3 或更高版本
- 构建桌面安装包时需要 Tauri 对应平台依赖

## 安装依赖

```bash
bun install
```

## 运行终端 UI

```bash
just tui
```

TUI 基于 ratatui 构建，适合在终端里快速启动 Agent 会话。

## 运行桌面 GUI

```bash
just tauri-dev
```

该命令会同时启动 Vite 前端和原生 Tauri 窗口。只做 Web 前端开发时可以使用：

```bash
just gui-dev
```

## 验证变更

```bash
just check
```

该命令会运行仓库的格式、lint 和 Rust 测试门禁。涉及 GUI 的改动还可能需要根据影响范围运行 Vitest、Playwright 或 tauri-pilot。
