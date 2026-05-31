---
title: 排错与 FAQ
description: 针对最常见的错误和问题的精选答案。
outline: [2, 3]
---

# 排错与 FAQ

本页收集了使用 Kairox 第一周内大多数用户都会撞上的症状,并给出底层原因和修复办法。如果这里没有你的问题,先去 [GitHub discussions](https://github.com/Z-Only/kairox/discussions) 搜一下,找不到匹配再提问。

## 安装与构建

### `bun: command not found`

Bun 装好了,但不在你的 PATH 里。安装器把它放到了 `~/.bun/bin`。加进去:

```bash
export PATH="$HOME/.bun/bin:$PATH"
```

把这一行写到你的 shell rc(`~/.zshrc`、`~/.bashrc`)里以便持久生效。

### 运行脚本时报 `npm`、`pnpm` 或 `yarn` 的错

Kairox 通过 `packageManager` 字段强制使用 Bun。请用 `bun install`、`bun run <script>` 和 `just` recipe。不要混用包管理器。

### Linux 上找不到 `webkit2gtk` 或 `libsoup`

你缺少 Tauri 2 的平台依赖。按 [安装](./installation) 里的说明安装。Ubuntu 24.04+ 上的包名是 `libwebkit2gtk-4.1-dev`;更早的版本是 `libwebkit2gtk-4.0-dev`。按你的发行版实际提供的版本对应安装。

### Windows 上 `link.exe not found` 或 MSVC 报错

安装 Visual Studio 2022 C++ Build Tools 并勾选 “Desktop development with C++” workload:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

### macOS 上 `xcrun: error: invalid active developer path`

```bash
xcode-select --install
```

### 首次跑 `cargo build` 很慢

首次 Tauri 构建会编译几百个 crate 并下载平台 SDK。在一台普通笔电上预计需要 10–20 分钟。后续构建是增量的,只需几秒。

如果构建看上去卡住了,确认 `cargo` 是否还在实际产出输出(它会打印编译进度),还是卡在某个网络操作。超过 30 分钟没有任何输出是不正常的,这时请检查磁盘空间和网络。

### Husky pre-commit hook 没跑

你在跑 `bun install` 之前就 commit 了。Husky 是通过 `prepare` 脚本安装 hook 的。请跑:

```bash
bun install
```

然后下次 commit 就会经过格式和 lint 的关卡。

### 全新检出上 `just check` 就失败

如果在干净的 `origin/main` 检出上 `just check` 就失败了,那是你的环境有问题,不是仓库本身的问题。第一条报错会告诉你原因:

- 缺少平台依赖(几乎总是) —— 见 [安装](./installation)。
- Rust toolchain 过期 —— `rustup update stable`。
- Bun 版本过旧 —— `bun upgrade`。

不要在一个本就有问题的基线上开始动手。

## Session 与 runtime

### “Model returned no content” 或者回复为空

最常见的原因是 profile 配错了。请检查:

1. `provider`、`model_id` 和 `base_url` 与你的 provider 期望的是否一致。
2. `api_key_env` 指向的 env 变量是否真的在你的 shell 里设置了。
3. 模型名是否和 provider 文档里写的完全一致(是 `gpt-4.1`,不是 `gpt4`;是 `claude-sonnet-4-20250514`,不是 `claude-4`)。

如果看起来都没问题,跑一下 live 冒烟测试:

```bash
just test-live    # 没设 GITHUB_TOKEN 时会自动 skip
```

它会用一个已知可用的 profile 跑 GitHub Models,从端到端验证你的 toolchain。

### 模型想跑的 tool 报 “Permission denied”

这是默认 `ApprovalPolicy::OnRequest` + `SandboxPolicy::WorkspaceWrite` 组合下的预期行为:policy engine 会对任何有风险的操作弹提示,你不允许就拒,或者在当前 sandbox 结构上不允许时直接失败(例如在 `SandboxPolicy::ReadOnly` 下的任何写操作)。

- 只允许这一次:TUI 里按 <kbd>Y</kbd>,GUI 里点 **Allow**。
- 在这个 workspace 里持续允许:GUI 里点 **Always allow**。
- 不希望被 sandbox 已放行的调用再打扰:把 `ApprovalPolicy` 切到 `Never`(TUI 状态栏选择器;GUI `ChatApprovalSelector`)。
- 想允许当前 sandbox 正在拒绝的写操作:把 `SandboxPolicy` 切到更宽的变体,如 `WorkspaceWrite` 或 `DangerFullAccess`(TUI:<kbd>B</kbd> 循环切换;GUI `ChatSandboxSelector`)。批准不能放宽 sandbox —— 只有切换 sandbox 才行。

完整决策矩阵见 [Permissions 与 Tools](../concepts/permissions-and-tools)。

### Context compaction 触发得太频繁(或者从来不触发)

调整 `kairox.toml` 里的 `[context].auto_compact_threshold`。默认是 `0.85`(当用量达到当前模型 context window 的 85% 时触发 compaction)。调低(比如 `0.7`)会更早触发;调高(比如 `0.95`)会延后。设成 `1.0` 等同于关闭自动 compaction;手动 compaction 仍然可以通过命令面板触发。

如果你当前用的模型 context window 比较特殊、Kairox 还不认识,可以在 profile 上显式设置 `context_window` 和 `output_limit`。详见 [Configuration](../reference/configuration)。

### Session 中途切换模型没生效

切换会通过 session actor 排队,并在下一个 turn 的边界生效。如果你是在某个 turn 正在 streaming 时切的,切换会等当前 turn 完成后再让新模型接管。切换落地时,trace 里会出现一条 `ProfileChanged` event。

### “Failed to assemble context: budget exceeded”

某一条消息——通常是粘进来的整个文件或者一个很大的 tool 结果——超过了可用 context。可选的处理方式:

1. 先手动触发一次 compaction。
2. 切到一个 `context_window` 更大的 profile。
3. 调低 `auto_compact_threshold`,让下一次 compaction 更早触发。
4. 直接精简输入。

## MCP

### MCP server 一直停留在 “Starting”

transport 的握手卡住了。请检查:

- 对于 stdio:`command` 在你的 PATH 里存在吗?试着在终端里手动跑——`npx -y @modelcontextprotocol/server-filesystem /tmp` 应该会在 stderr 上打印 MCP 消息。
- 对于 SSE 或 Streamable HTTP:`url` 是否可达?`curl -i <url>` 应该返回 200(或者一个流式响应)。
- 对于需要 env 变量的 stdio server:对应的 env 变量是否真的设置了?空字符串约定(`MY_VAR = ""`)的含义是“读取同名的 env 变量”。如果这个 env 变量没设,server 能起来但无法完成认证。

server 会发出 `McpServerStarting` → `McpServerReady`(或 `McpServerFailed`)——失败 event 会携带可在 trace 里查看的诊断 payload。

### MCP server 的 tool 没在 picker 里出现

server 起来了,但不是所有 tool 都注册成功。有些 server 必须等某个子命令成功之后才会暴露 tool(比如一个 GitHub server 需要 token 才能枚举它的 tool 集合)。在 trace 里查找 `Ready` 之后是否还跟着一条 `McpServerFailed`——这通常意味着握手成功但 tool discovery 失败。

### “MCP server 一直在重启”

默认配置下,`auto_restart = true`、`max_restart_attempts = 3`。失败重启三次之后,manager 就会放弃,并发出带诊断的 `McpServerFailed`。看 trace 里的底层错误;往往是缺少一个 env 变量,或者命令路径过期了。把配置修好,再到 marketplace 视图里 stop/start server(或者在 TUI 里 `Ctrl+C` 手动杀掉一下,让 manager 来重启)。

## GUI

### GUI 启动了,但窗口是空白的

Vite 开发服务器崩了,或者还没启动完 Tauri 就去加载 URL 了。先停掉,重新跑 `just tauri-dev`,留意终端里 Vite 的报错。如果页面加载了但内容缺失,打开 devtools(开发构建里在窗口里右键 → “Inspect Element”),看一下 console 报错。

### 预构建 GUI 提示 “auto-update failed”

updater 联不上 GitHub Releases。检查网络以及 GitHub 是否可达。更新会留到下次启动再下载——失败不是致命错误。

### 设置改了没生效

有些设置(模型默认值、MCP server 注册)在下一个 session 才生效;另一些(skill、`ApprovalPolicy`、`SandboxPolicy`)是立即生效的。如果某个设置没生效,新建一个 session 再试一次。如果还是没生效,请连同具体的设置项提个 issue。

## 数据与存储

### Kairox 把数据存在哪儿?

| 内容                             | 位置                                                   |
| -------------------------------- | ------------------------------------------------------ |
| Session event(聊天、trace、task) | `~/.kairox/kairox.db`(SQLite)                          |
| Memory 条目                      | 同一个 SQLite 数据库,放在独立的表里                    |
| 用户作用域的 skill               | `~/.kairox/skills/`                                    |
| Workspace 作用域的 skill         | `<workspace>/.kairox/skills/`                          |
| 项目配置                         | `<workspace>/.kairox/config.toml`                      |
| 用户配置                         | `~/.kairox/config.toml`                                |
| Plugin                           | `~/.kairox/plugins/` 或 `<workspace>/.kairox/plugins/` |

具体路径在 Windows 上可能不同(`%APPDATA%\kairox\`),macOS 上则是 `~/Library/Application Support/kairox/`。GUI 的 “About” 面板里能看到你这次安装解析出来的实际路径。

### 怎么重置 memory?

在 GUI 里,打开 memory 浏览器,选中条目然后删除。从 TUI 的 trace 面板:选中一条 memory 条目,按 <kbd>D</kbd>(再按 <kbd>Y</kbd> 确认)。

要全部清掉,关掉 app,删除 `~/.kairox/kairox.db`(或者先备份)。下次启动会创建一个全新的数据库。

### 怎么导出一个 session?

session 就是 SQLite 里的行;导出能力目前在 API 层有。眼下你可以直接用 `sqlite3` CLI 查数据库,按 session ID 抽取 event。一个一等公民的导出 UI 在 roadmap 上。

## 日志

### 怎么开启 verbose 日志?

启动前设置 `RUST_LOG`:

```bash
RUST_LOG=agent_runtime=debug,agent_models=debug just tui
```

过滤器使用 `tracing` 的语法。要看全部(非常吵):

```bash
RUST_LOG=debug just tui
```

对于 GUI,在 `just tauri-dev` 之前设置同样的 env 变量。日志会输出到启动它的那个终端里。

### 生产环境的隐私与 tracing

当配置了真实的 model client 或 shell tool 时,生产配置会默认走最小化 trace。这一行为是在代码里强制执行的,不在 TOML 层。只有当配置的 provider 和 tool 在开发场景下足够安全时(比如 `fake` provider + 没有真实 shell),才会自动允许 verbose tracing。详见 [Configuration](../reference/configuration#privacy-defaults)。

## 寻求帮助

如果你的问题这里没有:

- 搜一下 [GitHub Discussions](https://github.com/Z-Only/kairox/discussions) —— 多数问答都已经在那里了。
- 开一个新的 discussion 来讨论产品或集成相关的问题。
- 针对可复现的 bug,带一个最小复现开一个 GitHub Issue。
- 想深入源码的话,通过 [Crate Index](../reference/crate-index) 找到正确的模块入口。

## 本页不涉及的内容

本页是精选过的 FAQ。它不涉及 runtime 背后的概念模型([架构](../concepts/architecture))、详尽的配置 schema([Configuration](../reference/configuration)),也不涉及贡献流程([Contributing](../community/contributing))。
