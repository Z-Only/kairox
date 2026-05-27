---
title: Releases & Security
description: 发布模型、产物校验、受支持的版本范围，以及如何负责任地报告漏洞。
outline: [2, 3]
---

# Releases & Security

本页讲的是共享同一条发布流水线的两件事：Kairox 如何发布，以及如何针对已发布版本提交安全问题。

::: tip 权威来源
权威文件分别是负责发布流程的 [`docs/releasing.md`](https://github.com/Z-Only/kairox/blob/main/docs/releasing.md) 和负责安全策略的 [`SECURITY.md`](https://github.com/Z-Only/kairox/blob/main/SECURITY.md)。如果本页与它们冲突，以仓库文件为准。
:::

## 发布模型

Kairox 遵循 [semantic versioning](https://semver.org/)，并适用以下 1.0 之前的额外说明：

| 版本号        | 触发条件                                            |
| ------------- | --------------------------------------------------- |
| Patch `0.X.Y` | 仅限 bug 修复与安全修复，不引入行为变更。           |
| Minor `0.X.0` | 新功能、行为变更、breaking change（1.0 之前允许）。 |
| Major `1.0.0` | runtime 契约稳定到可以做兼容性承诺时发布。          |

`main` 是集成分支。每次合入都会 squash 并打上一个 Conventional Commits 前缀，由 [git-cliff](https://github.com/orhun/git-cliff) 在下一次发布时用作 changelog 分组。

## 发布流程

发布是一个 PR，而不是对 `main` 的强推：

1. 维护者切出 `chore/release-vX.Y.Z`，运行 `just bump-version X.Y.Z`。这条配方会同步更新五个文件：
   - 根目录的 `Cargo.toml`（`[workspace.package].version`）
   - `Cargo.lock`
   - 根目录的 `package.json`
   - `apps/agent-gui/package.json`
   - `apps/agent-gui/src-tauri/tauri.conf.json`
2. 运行 `git cliff --tag vX.Y.Z -o CHANGELOG.md`，基于自上次 tag 以来的 Conventional Commits 重新生成 changelog。
3. 本地校验 —— `just check`、`just check-types`，必要时跑 `just tauri-build` —— 必须全部通过。
4. 发布 PR 提交，等待 `ci-success` 转绿后合入 `main`。
5. 维护者 checkout 合入后的 `main` commit，从那一处推送 `vX.Y.Z` tag。
6. [`release-build.yml`](https://github.com/Z-Only/kairox/blob/main/.github/workflows/release-build.yml) 在 tag 上触发，上传 macOS、Linux、Windows 的 TUI 二进制和 Tauri 桌面打包产物。

发布说明同样由 git-cliff 基于上述 Conventional Commits 自动生成，并附在对应 GitHub Release 页面上。

## 产物

每次发布会发布以下产物：

| 产物                             | 平台                                                                       | 来源 workflow                     |
| -------------------------------- | -------------------------------------------------------------------------- | --------------------------------- |
| TUI 二进制（`kairox`）           | macOS（Intel + Apple Silicon）、Linux x86_64、Windows x86_64               | `release-build.yml`               |
| TUI SHA256 校验文件（`.sha256`） | 与 TUI 二进制一致                                                          | `release-build.yml`               |
| Tauri 桌面打包产物               | macOS `.dmg`、Linux `.AppImage` / `.deb` / `.rpm`、Windows `.msi` / `.exe` | `release-build.yml`               |
| 发布说明（分类后的 changelog）   | n/a                                                                        | git-cliff via `release-build.yml` |

校验下载的 TUI 二进制时，将其 SHA256 与发布的 `.sha256` 文件比对即可：

```bash
shasum -a 256 -c kairox-aarch64-apple-darwin.sha256
```

Tauri bundle 在 macOS 上尚未做代码签名 —— 首次启动可能会看到 Gatekeeper 警告。可以右键 → 打开，或者去掉 quarantine 属性：

```bash
xattr -d com.apple.quarantine ~/Applications/Kairox.app
```

桌面 GUI 启动时会从 GitHub Releases 自动更新。新版本在后台下载，下次启动时生效。更新过程是非致命的 —— 网络失败不会阻塞当前 session。

## 受支持的版本

只有最新的 minor 发布线会得到安全修复。完整表格位于 [SECURITY.md](https://github.com/Z-Only/kairox/blob/main/SECURITY.md)，并以它为准。

如果你在不受支持的版本上发现安全问题，维护者通常会回复「请升级到受支持版本后重测」。我们不会向更老的 minor 回滚（backport）修复。

## 报告漏洞

::: warning 请勿为安全问题提交公开 issue。
请使用私有渠道。公开 issue 会在修复存在之前先给攻击者预警。
:::

请通过 Kairox 仓库上的 [**GitHub Security Advisories**](https://github.com/Z-Only/kairox/security/advisories/new) 私下报告。Advisory 流程允许维护者在不公开问题的情况下进行 triage、准备修复并协调披露。

如果 GitHub Security Advisories 不可用，可以直接联系仓库所有者：

- GitHub：[@Z-Only](https://github.com/Z-Only)

提交报告时，请包含：

- **受影响组件** —— 例如 `agent-tools` 的 shell 执行器、`agent-mcp` 的 SSE transport，或某个 Tauri 命令 handler。
- **复现步骤** —— 最小化配置加命令，或一段脚本。
- **影响评估** —— 攻击者能做什么、需要什么前提（本地访问？特定配置？恶意 MCP server？）。
- **建议的缓解方案** —— 哪怕只是粗略思路也有帮助。

维护者会尽快确认有效报告，并在发布修复版本前协商一个协调披露窗口。

## 隐私与遥测

Kairox 不带遥测。关于你的 session、prompt、tool 调用以及环境的任何信息，除你自己配置的模型 provider 之外，不会被发送到其他任何地方。

当配置了真实的 model client 或 shell tool 时，runtime 在生产环境默认采用 **minimal trace**。这是在代码中强制的（不在 TOML 里），因此误配置不会意外地在生产环境启用详细 trace。只有当配置的 provider 与 tool 在开发场景下被证明是安全的（例如使用 `fake` provider 且没有真实 shell）时，才允许使用详细 trace。详见 [Configuration](../reference/configuration#privacy-defaults)。

桌面端的自动更新器只会联系 GitHub Releases 检查新版本。Kairox 不运营任何自有的更新服务器。

## 可复现构建

从同一个 tag commit 出发的源码构建，在相同的工具链版本下（由 `rust-toolchain.toml` 钉住）应产出完全一致的 Rust 二进制。Tauri bundle 在不同 host 上会因为平台打包细节有小差异，但内嵌的 JavaScript 与 Rust 产物在相同矩阵的发布构建上是一致的。

如果你怀疑某个已发布的产物与源码不一致，请提交 security advisory —— 这是一个我们会严肃对待的可信度问题。

## 本页不涉及的内容

本页讲的是发布模型与安全策略，不涉及贡献流程（[Contributing](./contributing)）、即将发布内容的 roadmap（[Roadmap](./roadmap)），也不涉及在报告问题前你可能想了解的 runtime 架构（[Architecture](../concepts/architecture)）。
