---
title: Installation
description: Per-OS prerequisites, build-from-source paths for TUI and GUI, and the most common install errors.
outline: [2, 3]
---

<script setup>
import ReleaseBanner from "../.vitepress/theme/components/ReleaseBanner.vue";
</script>

# Installation

Kairox ships as a Rust workspace plus a Tauri 2 desktop app. You can install pre-built desktop binaries from GitHub Releases, or build from source. This page covers both paths and lists the per-OS prerequisites that trip up first-time builders.

If you only want the fastest path, jump to [Getting Started](./getting-started). If you hit an unfamiliar error, see [Troubleshooting & FAQ](./troubleshooting).

<ReleaseBanner />

## Two install paths

| Path                     | What you get                                                                            | When to use                                           |
| ------------------------ | --------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Pre-built desktop binary | The GUI as a packaged app. Auto-updates via the in-app updater.                         | You want to use Kairox, not change it.                |
| Build from source        | TUI binary, GUI dev server, hot reload, full test suite. Required for any contribution. | You want to develop, debug, or run the latest `main`. |

You can install both; they do not conflict.

## Path 1 — Pre-built desktop binary

The GUI is built for macOS, Linux, and Windows on every release. Download from the latest release page:

- **macOS (Apple Silicon)** — `.dmg` ending in `aarch64`.
- **macOS (Intel)** — `.dmg` ending in `x86_64`.
- **Linux** — `.AppImage` (recommended, works everywhere), `.deb` (Debian/Ubuntu), `.rpm` (Fedora/RHEL).
- **Windows** — `.msi` (recommended) or `.exe`.

After install, launch "Kairox" and configure a model profile from the settings panel (see [First Session](./first-session)). The auto-updater checks for new releases on launch; updates apply on the next launch.

The TUI is not yet shipped as a pre-built binary. Build from source for now.

## Path 2 — Build from source

Source builds give you the TUI binary, the GUI dev server, and the full toolchain for running tests or hacking on the runtime.

### Toolchain prerequisites

You need three toolchains and one task runner:

```bash
# Rust (stable). rust-toolchain.toml pins the version automatically.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js 22+ (use a version manager — nvm, mise, asdf, fnm — or your OS package manager).

# Bun 1.3+
curl -fsSL https://bun.sh/install | bash

# just (task runner)
cargo install just
# or
brew install just
```

### Platform prerequisites

The Rust runtime and TUI build with nothing extra. The Tauri 2 desktop GUI needs platform libraries to produce a window:

#### macOS

```bash
xcode-select --install
```

That installs the Command Line Tools, which include the Apple SDK headers and `clang`. No other system packages are needed.

#### Linux (Debian / Ubuntu)

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

The full canonical list is in [`ci.yml`](https://github.com/Z-Only/kairox/blob/main/.github/workflows/ci.yml). If your distro uses a different package manager or webkit version, match by purpose: WebKitGTK 4.1, libxdo, libssl, librsvg, build essentials.

#### Linux (Fedora / RHEL)

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

Install the WebView2 runtime (most modern Windows 10/11 machines already have it) and the Visual Studio C++ build tools. The simplest path:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
winget install Microsoft.EdgeWebView2Runtime
```

Inside the Visual Studio Build Tools installer, select the "Desktop development with C++" workload.

### Clone and bootstrap

```bash
git clone https://github.com/Z-Only/kairox.git
cd kairox
bun install
```

`bun install` installs JS dependencies and wires Husky pre-commit hooks. Always run it after a fresh clone or after creating a worktree.

### Run the workspace gates

```bash
just check
```

This runs format checks, lints, and the full Rust test suite. On a fresh checkout against `origin/main` it should be green. If it fails, the first failure tells you what is missing — almost always a platform prerequisite.

### Build the TUI

```bash
just tui
# or
cargo run -p agent-tui
```

This compiles the TUI in debug mode and runs it. For a release binary:

```bash
cargo build --release -p agent-tui
./target/release/kairox
```

### Build the GUI (dev mode)

```bash
just tauri-dev
```

This starts the Vite frontend dev server and the native Tauri window together with hot reload. The first build is slow because Tauri downloads platform SDKs and crates; subsequent builds are incremental.

For frontend-only iteration (e.g., styling a Vue component without rebuilding Rust):

```bash
just gui-dev
```

### Build the GUI (packaged binary)

```bash
just tauri-build
```

The output bundle lands under `apps/agent-gui/src-tauri/target/release/bundle/`. The exact filename depends on the host OS (`.dmg`, `.AppImage`, `.deb`, `.rpm`, `.msi`).

A faster, unoptimized variant useful for smoke-testing the packaging itself:

```bash
just tauri-build-fast
```

## Worktrees (for contributors)

Kairox uses git worktrees for isolated development. Create one and the helper runs `bun install` for you:

```bash
just worktree feat/my-feature
```

This creates `.worktrees/feat-my-feature/` branched from `origin/main` and wires Husky. The base checkout stays clean for rebases and emergency fixes. Contributing flow lives in [Contributing](../community/contributing).

## Configuration

Kairox reads its config from one TOML file:

```bash
mkdir -p .kairox
cp kairox.toml.example .kairox/config.toml
cp .env.example .env
```

Edit `.kairox/config.toml` to define model profiles, MCP servers, and the `[context]` budgeting section. Edit `.env` to set API keys (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.). The full schema is in [Configuration](../reference/configuration).

Discovery order: the runtime walks up to 5 parent directories from your current working directory looking for `.kairox/config.toml`. If none is found, it falls back to `~/.kairox/config.toml`, then to built-in defaults.

## Common install errors

::: details `webkit2gtk` not found (Linux)
You are missing the WebKitGTK 4.1 development headers. Install per the distro instructions above. On Ubuntu 24.04+ the package is `libwebkit2gtk-4.1-dev`; on older releases it may be `-4.0-dev` — check `apt search webkit2gtk`.
:::

::: details `link.exe not found` or MSVC errors (Windows)
You are missing the Visual Studio C++ Build Tools. Install via `winget install Microsoft.VisualStudio.2022.BuildTools` and ensure the "Desktop development with C++" workload is selected.
:::

::: details `xcrun: error: invalid active developer path` (macOS)
You are missing the Xcode Command Line Tools. Run `xcode-select --install` and retry.
:::

::: details `bun: command not found` after install
Bun installs to `~/.bun/bin/bun`. Add it to your PATH: `export PATH="$HOME/.bun/bin:$PATH"`. Add to your shell rc to persist.
:::

::: details `cargo build` hangs at "Compiling agent-gui-tauri"
The first Tauri build downloads platform SDKs and compiles many crates. On a slow connection this takes 10–20 minutes. Subsequent builds are seconds. If it has been longer than 30 minutes, check for an actual stall in the cargo output rather than slow I/O.
:::

::: details Husky pre-commit hooks not firing
You ran `git commit` without first running `bun install`. Run `bun install` and try again.
:::

::: details `linker 'cc' not found` (Linux)
Install build essentials: `sudo apt install build-essential` or your distro's equivalent.
:::

For anything not listed here, see [Troubleshooting & FAQ](./troubleshooting) or open a [discussion](https://github.com/Z-Only/kairox/discussions).

## Verifying the install

After install, three commands prove the workspace is healthy:

```bash
just check         # format + lint + test
just tui           # opens the TUI; press Ctrl+C to exit
just tauri-dev     # builds and opens the desktop GUI
```

If all three succeed, you are ready for [First Session](./first-session).

## What this page does not cover

This page is the install reference. It does not cover the conceptual model behind the runtime ([Architecture](../concepts/architecture)), the full TOML configuration schema ([Configuration](../reference/configuration)), or how to use Kairox interactively ([First Session](./first-session)).
