---
title: 快速开始
description: 五分钟从克隆仓库走到第一个 session,无论是 TUI 还是桌面 GUI 都能跑起来。
outline: [2, 3]
---

<script setup>
import ReleaseBanner from "../../.vitepress/theme/components/ReleaseBanner.vue";
</script>

# 快速开始

Kairox 是一个本地优先的 AI Agent 工作台。仓库里包含一个 Rust workspace(覆盖 runtime、memory、models、tools、MCP、skills、plugins),一个基于 `ratatui` 构建的终端 UI,以及一个 Tauri 2 + Vue 3 的桌面 GUI。本页是从全新克隆到可用 Agent session 的五分钟最短路径。

如果你想要一份更深入的安装指南,涵盖各操作系统的前置条件以及 Tauri toolchain,请跳到 [安装](./installation)。如果想理解 runtime 在背后做了什么,请阅读 [架构](../concepts/architecture)。

<ReleaseBanner />

## 前置条件

你需要在本机上准备三套 toolchain。下表里的版本是我们测试时使用的最低版本——更新的版本完全没问题。

| Toolchain | 最低版本 | 用途                                                               |
| --------- | -------- | ------------------------------------------------------------------ |
| Rust      | stable   | 所有 crate,由 `rust-toolchain.toml` 锁定。                         |
| Node.js   | 22+      | 前端工具链、文档站点、生成的 TypeScript 类型。                     |
| Bun       | 1.3+     | Workspace 包管理器,用来替代 `npm`/`pnpm`/`yarn`。                  |
| `just`    | latest   | 任务运行器,通过 `cargo install just` 或 `brew install just` 安装。 |

要做桌面 GUI 相关的工作,你还需要 Tauri 2 的平台前置条件,详细说明见 [安装](./installation)。

::: warning 必须使用 Bun
Kairox 使用 Bun 作为 workspace 包管理器。仓库的 `packageManager` 字段会拒绝 `npm`、`pnpm` 和 `yarn`。请先安装 Bun:`curl -fsSL https://bun.sh/install | bash`。
:::

## 克隆并安装

```bash
git clone https://github.com/Z-Only/kairox.git
cd kairox
bun install
```

`bun install` 做了两件你应该了解的事:

1. 为 `apps/agent-gui` 这个 GUI workspace 安装前端依赖。
2. 通过 `prepare` 脚本安装 Husky pre-commit hook。少了这一步,commit 时不会触发格式化和 lint 的检查关卡。

通过 `just worktree <branch>` 创建的 worktree 会自动跑 `bun install`;手动创建的 worktree 不会,因此 `git worktree add` 之后请务必跑一次。

## 运行质量检查

在改动任何东西之前,先确认 workspace 能正常编译并且代码是干净的:

```bash
just check
```

`just check` 是以下三个关卡的合集:

| 关卡          | 底层命令               | 覆盖范围                                     |
| ------------- | ---------------------- | -------------------------------------------- |
| 格式检查      | `bun run format:check` | `oxfmt` + `cargo fmt --check`                |
| Lint          | `bun run lint`         | `oxlint`、`clippy`、Stylelint、parity matrix |
| Rust 测试套件 | `just test`            | `cargo test --workspace --all-targets`       |

如果在全新克隆下 `just check` 就失败了,请先停下来读一下错误信息——你的环境一定有问题。常见原因有:`agent-gui-tauri` 缺少平台依赖、Rust toolchain 过期、Bun 版本太旧。

## 试一下 TUI

TUI 是跑通一个 session 最快的方式。它默认使用一个内存里的 fake model client,你不需要任何 API key 就能用。

```bash
just tui
```

TUI 会在你的终端里打开,分成三栏:左侧是 session 列表,中间是聊天区,右侧是 trace。输入一段消息,然后按 <kbd>Ctrl+Enter</kbd> 发送。按 <kbd>F1</kbd> 可查看完整快捷键映射,或者直接跳到 [CLI & Keyboard](../reference/cli-and-keyboard) 查阅参考。

默认情况下 TUI 跑在 `fake` provider 上,它会回放预先配置好的响应。这对于不接入真实 API 的冒烟测试很有用。要使用真实的 provider,需要配置一个 profile(见下文)。

## 试一下 GUI

桌面 GUI 提供持久化 session、trace 时间线、memory 浏览器、MCP marketplace,以及一个把 TUI 全部能力都摆到键盘驱动菜单里的设置界面。

```bash
just tauri-dev
```

这会同时启动 Vite 开发服务器和原生 Tauri 窗口,Vue 前端和 Rust 后端都支持热重载。

如果首次运行时 Tauri 编译失败,几乎一定是缺少了某个平台前置条件(Linux 上的 WebKitGTK、Windows 上的 WebView2、macOS 上的 Xcode CLT)。[安装](./installation) 页面列出了所有依赖。

如果你只做前端工作、不需要原生窗口,可以用:

```bash
just gui-dev
```

## 配置一个 model profile

要和真实模型对话,请把示例配置复制一份,并指向你的 provider:

```bash
mkdir -p .kairox
cp kairox.toml.example .kairox/config.toml
cp .env.example .env
```

然后编辑 `.kairox/config.toml`。一个最短的 OpenAI profile 是这样的:

```toml
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
```

接着把 key 加到 `.env`:

```bash
OPENAI_API_KEY=sk-...
```

重启 TUI 或 GUI。profile 选择器(TUI 里按 <kbd>Alt+P</kbd>,GUI 里点 profile 下拉框)现在会列出 `fast`,在下一个 session 里选用它即可。

完整的配置 schema——所有 provider、所有字段、所有支持的 MCP transport,以及 `[context]` 预算章节——都在 [Configuration](../reference/configuration)。

## 接下来该读什么

挑一份与你的目标最相关的文档:

| 目标                                            | 阅读                                                               |
| ----------------------------------------------- | ------------------------------------------------------------------ |
| 在你的操作系统上搭建一个干净的开发环境。        | [安装](./installation)                                             |
| 跟着第一个真实 session 一步步走。               | [First Session](./first-session)                                   |
| 理解 runtime、event 流以及 Agent loop。         | [Runtime & Sessions](../concepts/runtime-and-sessions)             |
| 理解 memory 是如何存储、检索和 compaction 的。  | [Memory & Context](../concepts/memory-and-context)                 |
| 理解 Approval × Sandbox 策略引擎以及内置 tool。 | [Permissions & Tools](../concepts/permissions-and-tools)           |
| 用 MCP、skill 或 plugin 扩展 Kairox。           | [Extensibility: MCP / Skills / Plugins](../concepts/extensibility) |
| 查询某个 `just` 命令、TUI 快捷键或 GUI 快捷键。 | [CLI & Keyboard](../reference/cli-and-keyboard)                    |
| 找到某个 crate、查看它的公共 API,并跳转到源码。 | [Crate Index](../reference/crate-index)                            |
| 遇到了你看不懂的错误。                          | [Troubleshooting & FAQ](./troubleshooting)                         |

## 本页不涉及的内容

本页是“能跑起来”的最快路径。它不涉及各操作系统的安装排错([安装](./installation))、带截图的端到端首个 session 演示([First Session](./first-session)),也不涉及 runtime 背后的概念模型([架构](../concepts/architecture))。
