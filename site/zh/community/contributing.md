---
title: Contributing
description: 如何为 Kairox 提出、构建、验证并提交改动 —— 完整的端到端 PR 流程。
outline: [2, 3]
---

# Contributing

Kairox 几乎完全由社区贡献的 PR 构建而来。本页是端到端的完整流程：如何提出改动、本地构建与验证、写出 reviewer 能合并的 PR，以及跟踪它一直到发布。

::: tip 权威来源
贡献规则的权威版本位于仓库根目录的 [`CONTRIBUTING.md`](https://github.com/Z-Only/kairox/blob/main/CONTRIBUTING.md) 和 [`AGENTS.md`](https://github.com/Z-Only/kairox/blob/main/AGENTS.md)。如果本页与它们冲突，以仓库文件为准。本页对工作流、意图以及背后的原因做了进一步展开。
:::

## 开始之前

让贡献顺利落地，需要做好三件事：

1. **安装好工具链**。参考 [Installation](../guide/installation)。没有 Bun、Rust stable、Node 22+ 和 `just`，就无法运行 CI 用到的那些闸门。
2. **对改动范围有自己的判断**。聚焦的小 PR 几天内就能合入。一口气改动六个不相关 crate 的大 PR，往往要等几周，甚至最终合不了。如果觉得改动很大，就拆开。
3. **判断自己是否需要写 spec**。Bug 修复和小功能可以直接提 PR。任何涉及 runtime 契约、event 词汇、permission 模型，或 UI 与 runtime 通信方式的改动 —— 都应先在 `docs/superpowers/specs/` 下写一份 spec。仓库里的 `superpowers` skill 集合解释了完整流程。

如果你不确定是否需要写 spec，可以在 [discussion](https://github.com/Z-Only/kairox/discussions) 里发帖询问。提前对齐的成本，远低于 PR 推倒重来。

## 找件事做

可以从这些地方入手：

- **[Good first issues](https://github.com/Z-Only/kairox/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)** —— 描述清晰、范围有限的小任务。
- **[ROADMAP.md](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md)** —— 中期和长期的全景；挑一个时间维度上的条目，提出你打算怎么做。
- **[Discussions](https://github.com/Z-Only/kairox/discussions)** —— 还未变成 issue 的功能请求和集成问题。
- **你自己的使用场景** —— 在 discussion 里描述你遇到的缺口；维护者会帮你把它打磨成可落地的 PR。

## 建立 worktree

Kairox 使用 git worktree 进行隔离开发。主 checkout 保持干净，以便随时 rebase 或处理紧急修复：

```bash
just worktree feat/my-feature
cd .worktrees/feat-my-feature
```

`just worktree` 会基于本地 `main` 创建 `.worktrees/feat-my-feature/`，并运行 `bun install` 让 Husky 的 pre-commit hook 注册到位。如果需要最新 upstream base，请先同步本地 `main`。

分支前缀约定：`feat/`、`fix/`、`refactor/`、`test/`、`docs/`、`chore/`、`ci/`。请选用与改动相匹配的那一个。

## 实现改动

遵循 [AGENTS.md](https://github.com/Z-Only/kairox/blob/main/AGENTS.md) 以及 crate map 中的依赖方向。三条经验法则：

- **从 `agent-core` 起步**：如果你需要新的领域类型或 event variant。Workspace 中的其余 crate 都依赖 `agent-core`，先在那里加好类型，所有下游 crate 都能直接复用。
- **UI 留到最后接入**。TUI 与 GUI 应该消费已经在 runtime 中端到端跑通的能力。如果某个功能需要新的 IPC 接口，先加 Tauri 命令与 `EventPayload`，再写 UI 消费侧。
- **测试紧贴代码**。runtime 测试用 `FakeModelClient`；存储测试用内存版 SQLite event store；GUI 行为用 Playwright + IPC mock。GitHub Models 的实时冒烟测试由 `GITHUB_TOKEN` 控制开关，未设置时会自动跳过。

## 质量闸门

CI 在本地的等价命令只有一个：

```bash
just check
```

它是以下三道子闸门的合集：

| Gate            | 实际运行的内容                                                     | 失败的原因                                |
| --------------- | ------------------------------------------------------------------ | ----------------------------------------- |
| Format check    | `oxfmt`（TS/Vue/Markdown）+ `cargo fmt --check`                    | 代码未格式化。用 `bun run format` 修复。  |
| Lint            | `oxlint`、`cargo clippy --all-targets -- -D warnings`、`stylelint` | 出现 warning。本地就把 warning 视作错误。 |
| Rust test suite | `cargo test --workspace --all-targets`                             | 测试失败或 panic。                        |

针对单项任务的常用配方：

| 任务                  | 命令                                                          |
| --------------------- | ------------------------------------------------------------- |
| Format check          | `just fmt-check` / `bun run format:check`                     |
| 自动格式化            | `bun run format`                                              |
| Lint                  | `just lint` / `bun run lint`                                  |
| Rust 测试             | `just test`                                                   |
| TUI 集成测试          | `just test-tui`                                               |
| 全栈 runtime 测试     | `just test-fullstack`                                         |
| MCP 专项测试          | `just test-mcp`                                               |
| GUI 单测（Vitest）    | `just test-gui`                                               |
| GUI E2E（Playwright） | `just test-e2e` / `just test-e2e-headed` / `just test-e2e-ui` |
| 桌面 E2E（pilot）     | `just test-pilot`                                             |
| 实时模型冒烟          | `just test-live`（未设置 `GITHUB_TOKEN` 时自动跳过）          |
| 全部测试层            | `just test-all`                                               |
| 类型同步检查          | `just check-types`                                            |
| 重新生成类型          | `just gen-types`                                              |

CI 中的 `ci-success` 任务聚合并行作业，是合并的必需检查项。如果 `just check` 全绿且你的改动没有触及 IPC 契约，PR 基本可以确定能过 CI。

## 类型同步工作流

`apps/agent-gui/src/generated/` 下的 TypeScript 绑定由 [tauri-specta](https://github.com/specta-rs/tauri-specta) 生成。**永远不要手动编辑这些文件。**

在修改任何 `#[tauri::command]` 签名、`EventPayload` variant，或 event 中引用的领域类型之后：

1. 运行 `just gen-types`，它会重新生成 `commands.ts` 和 `events.ts`。
2. 运行 `just check-types`。CI 的 `type-sync` 任务也会跑这一步，如果生成产物有漂移会卡住合并。
3. 如果你新加了前端会监听的 IPC 命令或 event，请同步更新 [`apps/agent-gui/e2e/tauri-mock.js`](https://github.com/Z-Only/kairox/blob/main/apps/agent-gui/e2e/tauri-mock.js)，让 Playwright E2E 仍然能在完整的 mock 上跑。
4. 新增的 `#[tauri::command]` 函数必须同时注册到 `tauri::generate_handler!`（在 `apps/agent-gui/src-tauri/src/lib.rs`）**和** `collect_commands!`（在 `apps/agent-gui/src-tauri/src/specta.rs`）中。漏掉任一处都会导致运行时或类型生成失败。

## 提交信息

使用 Conventional Commits，搭配项目自定义 scope。完整列表如下：

```
core, runtime, models, tools, memory, store, config, mcp, skills, plugins, tui, gui, deps, ci
```

可以通过 commitlint 的提交示例：

```
feat(runtime): add scheduler retry policy
fix(gui): handle empty trace state
feat(mcp): add SSE transport support
docs(readme): clarify local setup
chore(deps): bump tauri to 2.7
```

提交信息写得不对，本地 commit hook 会直接失败 —— commitlint 通过 Husky 运行。如果你绕过了 Husky（worktree 创建后没跑 `bun install`），CI 的 PR 标题检查也会拦下来。

## 提交 PR

```bash
git push -u origin <branch>
gh pr create --fill --base main
```

PR 模板会要求填写：

- 一段话总结改动及其动机；
- 你跑过的验证（哪些 `just` 配方、结果如何）；
- 任何 GUI 变更的截图或短视频；
- 如果改动与操作系统相关，请注明平台差异。

请完整填写。reviewer 会按模板填写的完整度优先 triage PR。

## Review 与迭代

reviewer 会：

- 检查改动是否符合 spec（如果有）或对应的 discussion；
- 检查测试是否覆盖了新行为（并能防止回归）；
- 对面向用户的改动运行 GUI 进行行为验证；
- 内联提出改动请求。

请把 fixup 推到同一分支。通过审核后，PR 会被 squash-merge 到 `main`（在贡献者流程允许的情况下启用 auto-merge）。Squash 意味着 PR 在 `main` 上对应一个提交，该提交的信息就是 PR 标题，所以标题很重要。

## 合并之后

你的提交会落到 `main`，并被以下流程捕获：

- 在下一次发布时由 [`git-cliff`](https://github.com/orhun/git-cliff) 按对应前缀（`feat:` → Features、`fix:` → Bug Fixes 等）分组写入 changelog。
- 在 `v*` tag 推送时由 [`release-build.yml`](https://github.com/Z-Only/kairox/blob/main/.github/workflows/release-build.yml) 触发的下一次桌面二进制构建拾取。

发布模型以及如何校验构建产物详见 [Releases & Security](./releases-and-security)。

## 出问题怎么办

如果你的 PR 合入后让 `main` 挂掉（少见但有可能 —— 比如 flaky 测试、某个 CI 矩阵条目漏了 skip）：

- 立刻开一个 `fix:` 跟进 PR。
- 不要对 `main` 强推。
- 维护者可能会在修复落地之前用 `revert:` 临时回滚你的 PR；这不是评判，而是项目维持 `main` 绿色的方式。

## 代码风格

- **Rust**：`cargo fmt`、`cargo clippy -- -D warnings`。沿用相邻代码的写法；引入新依赖请在 PR 描述中说明理由。
- **TypeScript / Vue**：`oxfmt` 负责格式化，`oxlint` 负责 lint，`stylelint` 负责 CSS。状态管理用 Pinia；优先使用 Composition API 的 setup store；不要绕过 `vue-i18n` 直接写面向用户的字符串。
- **Markdown / 文档**：`oxfmt` 同样负责 Markdown 格式。文字保持直白。

## 依赖更新

Dependabot 已经为 Bun、Cargo 和 GitHub Actions 配置完毕。依赖 PR 通过 Dependabot auto-merge 工作流，在 CI 通过后自动合并。如果你要手动 bump 某个依赖，请把 PR 范围收窄 —— 一次只动一个生态。

## 寻求帮助

- **[GitHub Discussions](https://github.com/Z-Only/kairox/discussions)** —— 设计、集成、范围相关的问题。
- **[GitHub Issues](https://github.com/Z-Only/kairox/issues)** —— 带可复现步骤与环境信息的 bug。
- **[Crate Index](../reference/crate-index)** —— 从「我想改动行为 X」到「定义行为 X 的代码」之间的映射。
- **[Architecture](../concepts/architecture)** —— 任何非平凡 PR 之前都建议读一遍；它讲清了 workspace 的规则。

## 本页不涉及的内容

本页讲的是贡献工作流，不涉及该构建什么（[Roadmap](./roadmap)）、如何校验你下载的产物（[Releases & Security](./releases-and-security)），也不涉及你即将改动代码的概念模型（[Architecture](../concepts/architecture)、[Runtime & Sessions](../concepts/runtime-and-sessions)）。
