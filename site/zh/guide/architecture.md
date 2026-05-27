# 架构

Kairox 让 UI 层保持轻量，并通过 trait-based Rust 边界承载应用行为。`agent-core` facade 定义共享领域语言，运行时、模型、工具、记忆、存储、MCP、技能和插件则分别保持可测试的 crate 边界。

![Kairox 架构横幅](/banner.svg)

## 核心层

- `agent-core` 定义各界面共享的 facade、领域事件、ID、投影和构建信息。
- `agent-runtime` 编排会话、模型调用、任务图、权限、MCP 生命周期和多 Agent 策略。
- `agent-models` 将 OpenAI-compatible、Anthropic、Ollama 和 Fake provider 收敛到统一模型客户端接口。
- `agent-tools` 负责内置工具注册表和权限引擎。
- `agent-memory` 与 `agent-store` 提供持久记忆和 append-only 事件存储。
- `agent-skills` 与 `agent-plugins` 发现可复用的 prompt、工具、工作流和插件能力。

## UI 界面

终端 UI 和桌面 GUI 都依赖同一套核心契约。Tauri 应用通过 Rust commands 调用 facade，并把事件流转发到 Vue stores；TUI 则在终端布局中渲染会话、聊天、追踪和权限状态。

## 本地优先边界

Kairox 围绕显式本地控制设计。工具调用受权限系统约束，会话采用事件溯源，记忆按作用域存储，模型和 provider 配置来自本地项目与用户 profile。
