---
title: Roadmap
description: 精选展示已发布、进行中和远期规划的功能亮点。
outline: [2, 3]
---

# Roadmap

::: tip 权威来源
仓库根目录的 [`ROADMAP.md`](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md) 是规划的权威版本。本页对其中的重点做了精选整理，方便你不用读完上百条条目就能把握方向。如果本页与 `ROADMAP.md` 出现冲突，以仓库文件为准。
:::

Kairox 仍在 1.0 之前的积极开发阶段。本路线图按时间维度组织：当前已经交付的能力、正在进行中的工作，以及更长期的方向。

## 当前已发布（v0.32.x）

当前版本覆盖了 runtime、UI、MCP、skill 以及打包发行等基础能力。

### Runtime 与核心

- 以 `AppFacade` trait 作为 UI 与 runtime 之间唯一接缝的共享 Rust workspace。
- 基于事件溯源的状态管理，搭配 `SqliteEventStore`，session 在重启后仍然可以恢复。
- Agent loop 支持按模型的 context window 控制、由 budget 驱动的 prompt 装配、手动与自动 compaction，以及忙状态保护。
- session 进行中切换模型，并保留对应 profile；支持的模型还能选择 reasoning effort。
- 第二阶段的 DAG 执行能力，搭配 `AgentStrategy` 完成多 Agent 编排（planner / worker / reviewer）。
- 在 turn 结束时进行的无竞态自动 compaction（PR #531–#534）。

### Tool、permission 与 MCP

- 内置工具：`shell`、`fs.read`、`fs.write`、`fs.list`、`patch`、`search`。
- 正交的 Approval × Sandbox 策略引擎:`ApprovalPolicy`（`Never` / `OnRequest` / `Always`)控制 _什么时候_ 向用户询问;`SandboxPolicy`（`ReadOnly` / `WorkspaceWrite` / `DangerFullAccess`)控制 runtime 在结构上 _允许_ 做什么。旧的单轴 `PermissionMode` 枚举已在 v0.31.0 端到端移除（PR #517、#520)。
- MCP client 支持 stdio 和 SSE 两种 transport，并管理完整生命周期（`McpServer{Starting,Ready,Stopped,Failed}`）。
- MCP marketplace 内置目录，并支持远端来源；一键安装并提示 runtime 依赖。
- GUI 中提供 MCP 连接相关的操作。

### Memory 与上下文

- `<memory>` marker 协议，支持 session / user / workspace 多种 scope，以及审批语义。
- GUI 中的 memory 浏览器；TUI trace 面板支持删除。
- 基于 tiktoken 的上下文 budget 控制，达到可配置阈值时自动 compaction。

### UI

- **TUI** 基于 ratatui：三栏布局、流式 chat、trace 面板、permission overlay、命令面板，以及 settings / marketplace overlay。
- **GUI** 基于 Tauri 2 + Vue 3：持久化 session、任务图、可搜索的 trace 时间线、memory 浏览器、内联 permission 流程、按 session 配置 `ApprovalPolicy` 与 `SandboxPolicy` 的选择器、可调整宽度的工作台侧栏、项目 workspace，以及覆盖 model / agent / MCP / skill / plugin / hook / 指令的设置 tab。
- Tauri 2 的自动更新已经接入 GitHub Releases。

### 可扩展性

- 原生 **skill** 支持 workspace / user / session 三种 scope；支持从 SkillHub 安装。
- **Plugin** 通过 manifest 一次性打包 skill、tool、hook 与 MCP server；plugin 提供的 skill 在发现时带上命名空间。
- 可配置的 Agent 覆盖项按角色生效（model、`ApprovalPolicy`、`SandboxPolicy`、skill、tool 白名单）。

### 质量与 CI

- 并行 CI，并通过聚合任务 `ci-success` 总览；type-sync 闸门由 tauri-specta 把守；包含 clippy、oxlint、stylelint 与 oxfmt。
- Playwright 前端 E2E，借助浏览器侧的 IPC mock 运行。
- `tauri-pilot` 提供真实桌面端的 E2E 场景。
- 由 `GITHUB_TOKEN` 控制开关的 GitHub Models 实时冒烟测试。
- 针对 Rust 与 Vue 的按 crate 覆盖率闸门。

完整的已交付列表与对应 PR 链接，请翻阅 [`ROADMAP.md`](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md) 中的 **Near term** 章节。

## 进行中（中期）

- 覆盖更多模型 provider，并提供更精细的 profile 策略。
- 持续扩展 MCP 生态：更多 transport、更深入的发现机制、更丰富的 marketplace 元数据。
- 带签名的 plugin manifest，以及与 MCP 和 tool registry 共同编排的端到端安装流程。
- 为长时间运行的 Agent 工作提供更好的可观测与回放能力。
- 在第一阶段的 `facade_runtime` 拆分之后，继续推进 runtime 模块化。
- 在 planner / worker / reviewer 之外，提供可配置的专项子 Agent 角色。
- GUI 上的一等支持：指令编辑、hook 编写以及 plugin 开发。

## 长期方向

更长期的目标是打造一款成熟的本地优先 AI Agent 工作台，具备以下能力：

- 一个强大的 **skill 生态**，支持可组合的工作流、可复用的指令和能力发现。
- 一个建立在 MCP + tool registry + 带签名 manifest + marketplace 治理之上的强大 **plugin 生态**。
- 丰富的多 Agent 协作：委派、仲裁、专家团队、共享 memory、可审计的交接。
- 跨平台桌面分发的打磨，并支持自动更新。
- 无遥测的隐私实践，并在生产环境默认采用 `minimal_trace`。

## 如何影响 roadmap

- **场景反馈** —— 在 [discussion](https://github.com/Z-Only/kairox/discussions) 中开帖描述你想构建的东西，以及 Kairox 当前不足之处。
- **具体提案** —— 在 discussion 或 issue 中给出设计草案。对于复杂改动，我们更倾向于在 `docs/superpowers/specs/` 中写 spec，详见 [Contributing](./contributing)。
- **Pull request** —— 大多数已发布的能力最初都源自社区贡献的 PR。贡献流程详见 [Contributing](./contributing)。

## 版本策略与「已发布」的定义

Kairox 遵循 semver。在 1.0 之前，minor 版本（`0.X.0`）可能包含行为变更，patch 版本（`0.X.Y`）仅修 bug。上文「当前已发布」中的内容都包含在 `main` 上的最新 minor 版本中。

发布模型、产物校验以及安全披露流程详见 [Releases & Security](./releases-and-security)。

## 本页不涉及的内容

本页是经过精选的亮点合集，不涉及单个 PR 级别的历史（见 [`ROADMAP.md`](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md) 与 [Releases](https://github.com/Z-Only/kairox/releases)）、贡献流程（[Contributing](./contributing)），也不涉及如何让安全问题得到修复（[Releases & Security](./releases-and-security)）。
