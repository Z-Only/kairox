---
title: CLI 与键盘
description: 所有 `just` recipe、所有 `bun` 脚本、TUI 键位表,以及 GUI 键盘快捷键。
outline: [2, 3]
---

# CLI 与键盘

Kairox 既是 workspace,也是桌面应用,还是终端应用。这意味着你需要三套肌肉记忆:运行 workspace 的 shell 命令、驱动 TUI 的按键,以及驱动 GUI 的按键。本页就是这张速查表。

## `just` recipe

Kairox 使用 [`just`](https://github.com/casey/just) 作为任务运行器。通过 `cargo install just` 安装;`just --list` 可列出所有 recipe。

### 快速检查

| Recipe           | 作用                                                                      |
| ---------------- | ------------------------------------------------------------------------- |
| `just fmt-check` | 以 check 模式运行所有 formatter(`cargo fmt --check` 加 `oxfmt --check`)。 |
| `just lint`      | 在整个 workspace 上跑 Clippy,并对 GUI 源码跑 `oxlint` 和 Stylelint。      |
| `just test`      | `cargo test --workspace --all-targets`。                                  |
| `just test-gui`  | 跑 Vue 前端的 Vitest 套件(`bun --filter agent-gui test`)。                |
| `just coverage`  | Rust 基于源码的覆盖率门禁,加上 GUI V8 覆盖率门禁。                        |
| `just check`     | 格式检查 + lint + Rust 测试,等同于本地一遍完整的 CI gate。                |

### 格式化

| Recipe     | 作用                                               |
| ---------- | -------------------------------------------------- |
| `just fmt` | 自动格式化 Rust(`cargo fmt`)与 web 源码(`oxfmt`)。 |

### 开发

| Recipe                  | 作用                                                                      |
| ----------------------- | ------------------------------------------------------------------------- |
| `just tui`              | 运行 TUI 应用(`cargo run -p agent-tui`)。                                 |
| `just gui-dev`          | 启动 GUI 开发服务器(Vite 热重载),会先重新生成 TS 类型。                   |
| `just tauri-dev`        | 以开发模式运行 Tauri 桌面应用(Vite + 原生窗口),会先重新生成 TS 类型。     |
| `just gui-build`        | 构建 GUI web 资源。                                                       |
| `just tauri-build`      | 构建 Tauri 桌面二进制,并打包各平台的 installer。                          |
| `just tauri-build-fast` | 构建 Tauri 桌面二进制但不打包 installer(本地迭代更快)。                   |
| `just gui-size`         | 构建 GUI 并打印体积最大的若干产物文件。                                   |
| `just rust-size`        | 打印 `agent-tui` 与 `agent-gui-tauri` 的 release 二进制体积(需要先构建)。 |

### 发布

| Recipe                        | 作用                                                                                                                  |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `just release <version> ...`  | 对指定版本运行 `scripts/release.sh`。                                                                                 |
| `just release-dry <version>`  | 预览 `release.sh` 会执行的内容,但不真正执行。                                                                         |
| `just changelog <tag>`        | 运行 `git cliff --tag <tag>` 并格式化输出。                                                                           |
| `just bump-version <version>` | 在 `Cargo.toml`、`Cargo.lock`、根 `package.json`、`apps/agent-gui/package.json` 与 `tauri.conf.json` 之间同步版本号。 |

### Worktree

| Recipe                 | 作用                                                                                                 |
| ---------------------- | ---------------------------------------------------------------------------------------------------- |
| `just worktree <name>` | 从 `main` 分出一个 git worktree,放在 `.worktrees/<sanitized-name>` 下,然后在其中执行 `bun install`。 |

### 类型同步与代码生成

| Recipe             | 作用                                                                                                            |
| ------------------ | --------------------------------------------------------------------------------------------------------------- |
| `just gen-types`   | 基于 Tauri command 和 `EventPayload`,通过 Specta 重新生成 `apps/agent-gui/src/generated/{commands,events}.ts`。 |
| `just check-types` | 先执行 `just gen-types`,若生成结果与已提交版本不一致则失败。                                                    |

### 集成与端到端测试

| Recipe                 | 作用                                                                                                                          |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `just test-e2e`        | 针对 GUI 前端的 Playwright E2E 测试(对接 Tauri IPC mock)。                                                                    |
| `just test-e2e-headed` | 与 `test-e2e` 相同,但以 headed(可见浏览器)模式运行,便于调试。                                                                 |
| `just test-e2e-ui`     | 与 `test-e2e` 相同,但在 Playwright UI runner 中运行。                                                                         |
| `just test-tui`        | 确定性的 TUI 测试栈,无需真实终端。                                                                                            |
| `just test-tui-pty`    | 真实 PTY 的 TUI 冒烟测试(CI 上运行的那个),会先构建二进制。                                                                    |
| `just test-fullstack`  | 全栈 runtime 集成测试。                                                                                                       |
| `just test-all`        | `test` + `test-tui` + `test-fullstack` + `test-gui`。                                                                         |
| `just test-mcp`        | 跨 `agent-mcp`、`agent-tools`、`agent-config` 与 `agent-runtime` 的所有 MCP 相关测试。                                        |
| `just test-live`       | GitHub Models 在线冒烟测试(没有 `GITHUB_TOKEN` 时自动跳过)。                                                                  |
| `just test-pilot`      | 启用 `pilot` feature 启动 Tauri dev app,并运行 `tauri-pilot` E2E 场景。需要 `tauri-pilot-cli`;在 Linux 上请用 `xvfb-run -a`。 |
| `just test-pilot-live` | `test-pilot` 加上 `KAIROX_PILOT_LIVE_MODELS=1`,会对真实的 GitHub Models 跑测试,需要 `GITHUB_TOKEN`。                          |

## `bun` 脚本

根目录 `package.json` 暴露了一组可用 Bun 运行的脚本。大多数都被 `just` 包了一层;当你只想跑其中一步时,可以直接调用它们。

| 脚本                         | 作用                                                                                                      |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- |
| `bun run format`             | 以写入模式运行所有 formatter。                                                                            |
| `bun run format:check`       | 以检查模式运行所有 formatter(不写文件)。                                                                  |
| `bun run format:rust`        | 仅 `cargo fmt --all`。                                                                                    |
| `bun run format:web`         | 仅 `oxfmt --write .`。                                                                                    |
| `bun run lint`               | web lint + Rust lint + TUI/GUI parity 矩阵检查。                                                          |
| `bun run lint:web`           | `oxlint` 加 Stylelint。                                                                                   |
| `bun run lint:rust`          | 全 workspace 跑 Clippy,启用 `-D warnings`。                                                               |
| `bun run lint:parity-matrix` | 自定义脚本,确保 TUI/GUI 的特性 parity 被持续跟踪。                                                        |
| `bun run lint:style`         | 仅 Stylelint。                                                                                            |
| `bun run lint:oxlint`        | 仅 oxlint。                                                                                               |
| `bun run site:dev`           | 文档站点的 VitePress 开发服务器。                                                                         |
| `bun run site:build`         | 构建 VitePress 站点,并运行 `scripts/generate-llms-txt.mjs` 产出 `dist/llms.txt` 与 `dist/llms-full.txt`。 |
| `bun run site:preview`       | 预览已构建的文档站点。                                                                                    |
| `bun run coverage:rust`      | Rust 基于源码的覆盖率门禁(`scripts/run-rust-coverage.sh`)。                                               |
| `bun run coverage:web`       | GUI V8 覆盖率门禁(Vitest)。                                                                               |
| `bun run prepare`            | Husky install hook,在 `bun install` 后自动执行。                                                          |

在 GUI 应用内(`apps/agent-gui/package.json`)还有以下几个脚本:

| 脚本(在 `apps/agent-gui` 内或通过 `bun --filter agent-gui` 运行) | 作用                                     |
| ---------------------------------------------------------------- | ---------------------------------------- |
| `dev`                                                            | 在 `0.0.0.0:1420` 启动 Vite 开发服务器。 |
| `build`                                                          | Vite 生产构建。                          |
| `tauri:dev`                                                      | Tauri dev(Vite + 原生窗口)。             |
| `tauri:build`                                                    | Tauri 生产构建(带 installer 打包)。      |
| `test`                                                           | Vitest 单元测试。                        |
| `test:e2e`                                                       | Playwright E2E,使用 2 个 worker。        |
| `test:e2e:headed` / `test:e2e:ui`                                | Playwright 套件的 headed/UI 变体。       |

## TUI 键位表

TUI 基于 `ratatui` + `crossterm` 构建。键位的权威来源是 `crates/agent-tui/src/keybindings/resolver.rs` 中的 resolver。

### 全局

| 按键                  | 动作                                               |
| --------------------- | -------------------------------------------------- |
| <kbd>F1</kbd>         | 打开帮助 overlay。                                 |
| <kbd>Tab</kbd>        | 在面板之间切换焦点(Chat → Sessions → Trace)。      |
| <kbd>Esc</kbd>        | 退出当前 overlay / 取消当前 modal / 离开搜索模式。 |
| <kbd>Ctrl+C</kbd>     | 中断当前 turn;若没有正在进行的 turn,则退出应用。   |
| <kbd>Ctrl+Enter</kbd> | 不论焦点和输入模式,直接发送已组装的输入。          |
| <kbd>Ctrl+P</kbd>     | 切换命令面板。                                     |

### Alt 组合键(overlay、侧边栏、焦点)

| 按键             | 动作                         |
| ---------------- | ---------------------------- |
| <kbd>Alt+1</kbd> | 焦点切到 Chat 面板。         |
| <kbd>Alt+2</kbd> | 焦点切到 Sessions 侧边栏。   |
| <kbd>Alt+3</kbd> | 焦点切到 Trace 侧边栏。      |
| <kbd>Alt+S</kbd> | 切换 Sessions 侧边栏的显示。 |
| <kbd>Alt+T</kbd> | 切换 Trace 侧边栏的显示。    |
| <kbd>Alt+E</kbd> | 切换输入模式(单行 ↔ 多行)。  |
| <kbd>Alt+P</kbd> | 打开 profile 选择器。        |
| <kbd>Alt+C</kbd> | 切换 context 详情面板。      |
| <kbd>Alt+N</kbd> | 新建 session。               |
| <kbd>Alt+Q</kbd> | 退出。                       |
| <kbd>Alt+H</kbd> | 切换 Hooks overlay。         |
| <kbd>Alt+I</kbd> | 切换 Instructions overlay。  |

### Ctrl 组合键 overlay

| 按键              | 动作                                 |
| ----------------- | ------------------------------------ |
| <kbd>Ctrl+G</kbd> | 切换 Plugins overlay。               |
| <kbd>Ctrl+L</kbd> | 切换 Model overlay(当前模型与预算)。 |
| <kbd>Ctrl+M</kbd> | 切换 MCP overlay(server 状态)。      |
| <kbd>Ctrl+S</kbd> | 切换 Skills overlay。                |

### Chat 面板(获得焦点时)

| 按键                                         | 动作                                |
| -------------------------------------------- | ----------------------------------- |
| <kbd>Enter</kbd>                             | 单行模式下发送;多行模式下插入换行。 |
| <kbd>Ctrl+Enter</kbd>                        | 不论模式都直接发送已组装的输入。    |
| <kbd>Up</kbd> / <kbd>Down</kbd>              | 浏览输入历史。                      |
| <kbd>Alt+Up</kbd> / <kbd>Down</kbd>          | 选择上一条/下一条排队消息。         |
| <kbd>Alt+Left</kbd> / <kbd>Right</kbd>       | 将所选排队消息在队列中上移/下移。   |
| <kbd>Alt+Enter</kbd>                         | 立即发送所选的排队消息。            |
| <kbd>Alt+Delete</kbd> / <kbd>Backspace</kbd> | 删除所选的排队消息。                |
| <kbd>Backspace</kbd>                         | 向前删除一个字符。                  |
| <kbd>Delete</kbd>                            | 向后删除。                          |

### Sessions 面板(获得焦点时)

| 按键             | 动作                   |
| ---------------- | ---------------------- |
| <kbd>Enter</kbd> | 选中高亮的 session。   |
| <kbd>F2</kbd>    | 重命名高亮的 session。 |
| <kbd>A</kbd>     | 打开归档管理器。       |

### Trace 面板(获得焦点时)

| 按键                               | 动作                                                    |
| ---------------------------------- | ------------------------------------------------------- |
| <kbd>Left</kbd> / <kbd>Right</kbd> | 切换 trace 标签页(也可用 <kbd>[</kbd> / <kbd>]</kbd>)。 |
| <kbd>F5</kbd>                      | 切换 trace 密度(紧凑 ↔ 详细)。                          |
| <kbd>/</kbd>                       | 进入 memory 搜索。                                      |
| <kbd>S</kbd>                       | 循环切换 memory scope(session / user / workspace)。     |
| <kbd>R</kbd>                       | 重试所选 task。                                         |
| <kbd>C</kbd>                       | 取消所选 task。                                         |
| <kbd>Y</kbd>                       | 确认删除 memory。                                       |
| <kbd>D</kbd>                       | 删除所选 memory。                                       |

### Permission 提示

当 permission modal 处于显示状态时:

| 按键           | 动作                                       |
| -------------- | ------------------------------------------ |
| <kbd>Y</kbd>   | 同意本次调用。                             |
| <kbd>N</kbd>   | 拒绝本次调用。                             |
| <kbd>D</kbd>   | 拒绝本次调用**并**拒绝今后所有相同的调用。 |
| <kbd>Esc</kbd> | 拒绝(等价于 <kbd>N</kbd>)。                |

### 策略切换

| 按键               | 动作                                            |
| ------------------ | ----------------------------------------------- |
| <kbd>A</kbd>(大写) | 循环切换当前 session 的 permission 策略(mode)。 |
| <kbd>B</kbd>(大写) | 循环切换 sandbox 策略。                         |
| <kbd>x</kbd>       | 为当前聚焦项打开上下文菜单。                    |

## GUI 键盘快捷键

桌面应用沿用了操作系统标准的应用框架快捷键(Tauri 提供 <kbd>Cmd+Q</kbd> / <kbd>Alt+F4</kbd>、窗口切换等)。Kairox 特有的快捷键如下:

### Chat

| 按键                                | 动作                                     |
| ----------------------------------- | ---------------------------------------- |
| <kbd>Enter</kbd>                    | 在 composer 中无修饰键时,发送消息。      |
| <kbd>Shift+Enter</kbd>              | 在 composer 中插入换行。                 |
| <kbd>j</kbd> / <kbd>ArrowDown</kbd> | 将焦点切到 chat 面板的下一条 stream 项。 |
| <kbd>k</kbd> / <kbd>ArrowUp</kbd>   | 将焦点切到上一条 stream 项。             |
| <kbd>/</kbd>                        | 在 composer 为空时,触发内联命令面板。    |
| <kbd>@</kbd>                        | 在 composer 中触发文件提及面板。         |

### 命令与提及面板

| 按键                                      | 动作             |
| ----------------------------------------- | ---------------- |
| <kbd>ArrowDown</kbd> / <kbd>ArrowUp</kbd> | 移动高亮项。     |
| <kbd>Enter</kbd>                          | 选中高亮的条目。 |
| <kbd>Esc</kbd>                            | 关闭面板。       |

### 可编辑标签(session 名称等)

| 按键             | 动作       |
| ---------------- | ---------- |
| <kbd>Enter</kbd> | 确认编辑。 |
| <kbd>Esc</kbd>   | 取消编辑。 |

### 保留的修饰键组合

当没有正在编辑时,chat 面板会忽略所有带修饰键(Ctrl、Cmd、Alt)的按键 —— 这些组合留给应用未来可能安装的全局快捷键(例如 <kbd>Cmd+K</kbd> 命令面板、<kbd>Ctrl+J</kbd> 插入换行、<kbd>Alt+G</kbd> workspace 导航)。如果你发现某个想要的快捷键还没有,请通过全局 handler 注册,而不要让 chat 面板重载它。

## 权威来源位置

- **Recipe**:仓库根目录的 [`justfile`](https://github.com/Z-Only/kairox/blob/main/justfile)。
- **脚本**:根 [`package.json`](https://github.com/Z-Only/kairox/blob/main/package.json) 与 [`apps/agent-gui/package.json`](https://github.com/Z-Only/kairox/blob/main/apps/agent-gui/package.json)。
- **TUI 键位表**:[`crates/agent-tui/src/keybindings/resolver.rs`](https://github.com/Z-Only/kairox/blob/main/crates/agent-tui/src/keybindings/resolver.rs)。
- **GUI 键盘 handler**:[`apps/agent-gui/src/components/`](https://github.com/Z-Only/kairox/tree/main/apps/agent-gui/src/components) 中各个 `@keydown` 绑定。

如果本页中的某条绑定与源代码不一致,以源代码为准。请提 issue,以便修正本页。
