---
title: 安装
description: 各操作系统的前置条件、TUI 与 GUI 的源码构建路径,以及最常见的安装错误。
outline: [2, 3]
---

<script setup>
import ReleaseBanner from "../../.vitepress/theme/components/ReleaseBanner.vue";
</script>

# 安装

Kairox 以一个 Rust workspace 加上一个 Tauri 2 桌面应用的形式发布。你可以从 GitHub Releases 下载预构建的桌面二进制,也可以从源码构建。本页同时覆盖这两条路径,并列出常常让首次构建者踩坑的各操作系统前置条件。

如果你只想走最快的路径,请跳到 [快速开始](./getting-started)。遇到不熟悉的错误,请查看 [Troubleshooting & FAQ](./troubleshooting)。

<ReleaseBanner />

## 两种安装路径

| 路径             | 你能得到什么                                                               | 何时选它                               |
| ---------------- | -------------------------------------------------------------------------- | -------------------------------------- |
| 预构建桌面二进制 | 打包好的 GUI 应用。可通过应用内 updater 自动升级。                         | 你只想用 Kairox,而不打算修改它。       |
| 从源码构建       | TUI 二进制、GUI 开发服务器、热重载、完整测试套件。任何贡献都需要这条路径。 | 你打算开发、调试,或者跑最新的 `main`。 |

两条路径可以共存,不会冲突。

## 路径 1 —— 预构建桌面二进制

每个 release 都会构建针对 macOS、Linux 和 Windows 的 GUI。请到最新 release 页面下载:

- **macOS(Apple Silicon)** —— 文件名以 `aarch64` 结尾的 `.dmg`。
- **macOS(Intel)** —— 文件名以 `x86_64` 结尾的 `.dmg`。
- **Linux** —— `.AppImage`(推荐,在任何发行版都能跑)、`.deb`(Debian/Ubuntu)、`.rpm`(Fedora/RHEL)。
- **Windows** —— `.msi`(推荐)或 `.exe`。

安装完成后,启动“Kairox”,在设置面板里配置一个 model profile(见 [First Session](./first-session))。auto-updater 会在每次启动时检查新 release;更新会在下次启动时应用。

TUI 暂时还没有打包好的预构建二进制,目前需要从源码构建。

## 路径 2 —— 从源码构建

源码构建会给你 TUI 二进制、GUI 开发服务器,以及跑测试或修改 runtime 所需要的完整 toolchain。

### Toolchain 前置条件

你需要三套 toolchain 和一个任务运行器:

```bash
# Rust(stable)。rust-toolchain.toml 会自动锁定版本。
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js 22+(用 nvm、mise、asdf、fnm 这类版本管理器,或者直接用系统包管理器)。

# Bun 1.3+
curl -fsSL https://bun.sh/install | bash

# just(任务运行器)
cargo install just
# 或者
brew install just
```

### 平台前置条件

Rust runtime 和 TUI 不需要额外的系统依赖就能构建。Tauri 2 桌面 GUI 则需要平台库来渲染窗口:

#### macOS

```bash
xcode-select --install
```

这会安装 Command Line Tools,其中包含 Apple SDK 头文件和 `clang`。不需要任何其他系统包。

#### Linux(Debian / Ubuntu)

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

完整的规范列表在 [`ci.yml`](https://github.com/Z-Only/kairox/blob/main/.github/workflows/ci.yml) 里。如果你的发行版使用不同的包管理器或不同的 webkit 版本,按用途对应即可:WebKitGTK 4.1、libxdo、libssl、librsvg,以及编译基础工具。

#### Linux(Fedora / RHEL)

```bash
sudo dnf install -y \
  webkit2gtk4.1-devel \
  openssl-devel \
  curl \
  wget \
  file \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  gcc-c++ \
  make
```

#### Windows

安装 WebView2 runtime(大多数现代 Windows 10/11 已经自带)以及 Visual Studio C++ build tools。最简单的方式:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
winget install Microsoft.EdgeWebView2Runtime
```

在 Visual Studio Build Tools 安装器里,勾选 “Desktop development with C++” 这个 workload。

### 克隆并初始化

```bash
git clone https://github.com/Z-Only/kairox.git
cd kairox
bun install
```

`bun install` 会安装 JS 依赖,并配置好 Husky pre-commit hook。每次全新克隆之后,或者创建新的 worktree 之后,务必跑一次。

### 跑一遍 workspace 的关卡

```bash
just check
```

这会执行格式检查、lint,以及完整的 Rust 测试套件。在基于 `origin/main` 的全新检出上,它应该是绿的。如果失败了,第一个报错会告诉你缺什么——几乎总是平台前置条件没装齐。

### 构建 TUI

```bash
just tui
# 或者
cargo run -p agent-tui
```

这会以 debug 模式编译并运行 TUI。要 release 二进制:

```bash
cargo build --release -p agent-tui
./target/release/kairox
```

### 构建 GUI(开发模式)

```bash
just tauri-dev
```

这会同时启动 Vite 前端开发服务器和原生 Tauri 窗口,二者都支持热重载。首次构建会比较慢,因为 Tauri 需要下载平台 SDK 和大量 crate;之后的构建是增量的。

如果只做前端迭代(比如调一个 Vue 组件的样式,不想重新编译 Rust):

```bash
just gui-dev
```

### 构建 GUI(打包二进制)

```bash
just tauri-build
```

输出 bundle 会放在 `apps/agent-gui/src-tauri/target/release/bundle/` 下。具体文件名取决于宿主操作系统(`.dmg`、`.AppImage`、`.deb`、`.rpm`、`.msi`)。

要更快但不优化的变体,可以用来给打包流程本身做冒烟测试:

```bash
just tauri-build-fast
```

## Worktree(给贡献者)

Kairox 使用 git worktree 来做隔离开发。创建一个 worktree,辅助命令会顺便帮你跑 `bun install`:

```bash
just worktree feat/my-feature
```

这会基于 `origin/main` 创建 `.worktrees/feat-my-feature/`,并配置好 Husky。原 checkout 保持干净,方便做 rebase 和紧急修复。贡献流程详见 [Contributing](../community/contributing)。

## 配置

Kairox 从一个 TOML 文件读取配置:

```bash
mkdir -p .kairox
cp kairox.toml.example .kairox/config.toml
cp .env.example .env
```

编辑 `.kairox/config.toml` 来定义 model profile、MCP server 和 `[context]` 预算章节。编辑 `.env` 来设置 API key(`OPENAI_API_KEY`、`ANTHROPIC_API_KEY` 等等)。完整 schema 在 [Configuration](../reference/configuration)。

发现顺序:runtime 会从你当前工作目录开始,向上最多走 5 层父目录寻找 `.kairox/config.toml`。如果都没找到,会回落到 `~/.kairox/config.toml`,再不行就用内置默认值。

## 常见安装错误

::: details Linux 上找不到 `webkit2gtk`
缺少 WebKitGTK 4.1 的开发头文件。按上面的发行版说明安装即可。Ubuntu 24.04+ 上的包是 `libwebkit2gtk-4.1-dev`;更早的版本可能是 `-4.0-dev`——用 `apt search webkit2gtk` 确认一下。
:::

::: details Windows 上 `link.exe not found` 或 MSVC 报错
缺少 Visual Studio C++ Build Tools。通过 `winget install Microsoft.VisualStudio.2022.BuildTools` 安装,并确保勾选了 “Desktop development with C++” workload。
:::

::: details macOS 上 `xcrun: error: invalid active developer path`
缺少 Xcode Command Line Tools。运行 `xcode-select --install` 后重试。
:::

::: details 安装完之后 `bun: command not found`
Bun 装在 `~/.bun/bin/bun`。把它加到 PATH:`export PATH="$HOME/.bun/bin:$PATH"`。写进你的 shell rc 里以便持久生效。
:::

::: details `cargo build` 卡在 “Compiling agent-gui-tauri”
首次 Tauri 构建会下载平台 SDK 并编译大量 crate。在网络较慢的环境下需要 10–20 分钟。后续构建只需几秒。如果超过 30 分钟还没动静,请查看 cargo 输出确认是否真的卡住,而不只是 I/O 慢。
:::

::: details Husky pre-commit hook 没触发
你在 `git commit` 之前没跑 `bun install`。跑一下 `bun install` 再重试。
:::

::: details Linux 上 `linker 'cc' not found`
安装编译基础工具:`sudo apt install build-essential`,或者你所在发行版的对应包。
:::

这里没列出的问题,请查看 [Troubleshooting & FAQ](./troubleshooting),或者去 [discussion](https://github.com/Z-Only/kairox/discussions) 提问。

## 验证安装

安装完成后,以下三条命令能证明 workspace 是健康的:

```bash
just check         # 格式 + lint + 测试
just tui           # 打开 TUI,按 Ctrl+C 退出
just tauri-dev     # 构建并打开桌面 GUI
```

三条命令都通过,就可以进入 [First Session](./first-session) 了。

## 本页不涉及的内容

本页是安装参考。它不涉及 runtime 背后的概念模型([架构](../concepts/architecture))、完整的 TOML 配置 schema([Configuration](../reference/configuration)),也不涉及如何交互式地使用 Kairox([First Session](./first-session))。
