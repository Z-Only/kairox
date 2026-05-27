---
title: Releases & Security
description: The release model, artifact verification, supported versions, and how to report a vulnerability responsibly.
outline: [2, 3]
---

# Releases & Security

This page covers two things that share the same release pipeline: how Kairox ships, and how to report a security issue against a shipped version.

::: tip Source of truth
The canonical files are [`docs/releasing.md`](https://github.com/Z-Only/kairox/blob/main/docs/releasing.md) for the release process and [`SECURITY.md`](https://github.com/Z-Only/kairox/blob/main/SECURITY.md) for the security policy. If this page disagrees with either, the repository files win.
:::

## Release model

Kairox follows [semantic versioning](https://semver.org/). Pre-1.0 caveats apply:

| Bump          | Triggers                                                                  |
| ------------- | ------------------------------------------------------------------------- |
| Patch `0.X.Y` | Bug fixes and security fixes only. No behavior changes.                   |
| Minor `0.X.0` | New features, behavior changes, breaking changes (pre-1.0 allows this).   |
| Major `1.0.0` | When the runtime contract is stable enough for compatibility commitments. |

`main` is the integration branch. Every merge is squashed and tagged with a Conventional Commits prefix, which feeds [git-cliff](https://github.com/orhun/git-cliff) for the changelog grouping at the next release.

## Release flow

A release is a PR, not a force-push to `main`:

1. A maintainer cuts `chore/release-vX.Y.Z` and runs `just bump-version X.Y.Z`. That single recipe updates five files in sync:
   - root `Cargo.toml` (`[workspace.package].version`)
   - `Cargo.lock`
   - root `package.json`
   - `apps/agent-gui/package.json`
   - `apps/agent-gui/src-tauri/tauri.conf.json`
2. `git cliff --tag vX.Y.Z -o CHANGELOG.md` regenerates the changelog from Conventional Commits since the last tag.
3. Local verification — `just check`, `just check-types`, optionally `just tauri-build` — must pass.
4. The release PR opens, waits for `ci-success` to go green, and merges into `main`.
5. The maintainer checks out the merged `main` commit and pushes a `vX.Y.Z` tag from it.
6. [`release-build.yml`](https://github.com/Z-Only/kairox/blob/main/.github/workflows/release-build.yml) runs on the tag and uploads TUI binaries and Tauri desktop bundles for macOS, Linux, and Windows.

The release notes are generated automatically by git-cliff from the same Conventional Commits that produced the changelog, then attached to the GitHub Release page.

## Artifacts

Every release publishes:

| Artifact                              | Platforms                                                                  | Source workflow                   |
| ------------------------------------- | -------------------------------------------------------------------------- | --------------------------------- |
| TUI binary (`kairox`)                 | macOS (Intel + Apple Silicon), Linux x86_64, Windows x86_64                | `release-build.yml`               |
| TUI SHA256 checksum (`.sha256`)       | Same as TUI binary                                                         | `release-build.yml`               |
| Tauri desktop bundle                  | macOS `.dmg`, Linux `.AppImage` / `.deb` / `.rpm`, Windows `.msi` / `.exe` | `release-build.yml`               |
| Release notes (categorized changelog) | n/a                                                                        | git-cliff via `release-build.yml` |

Verify a downloaded TUI binary by comparing its SHA256 to the published `.sha256` file:

```bash
shasum -a 256 -c kairox-aarch64-apple-darwin.sha256
```

Tauri bundles are not yet code-signed on macOS — you may see a Gatekeeper warning on first launch. Right-click → Open the first time, or remove the quarantine attribute:

```bash
xattr -d com.apple.quarantine ~/Applications/Kairox.app
```

The desktop GUI auto-updates from GitHub Releases on launch. New versions download in the background and apply on the next launch. Updates are non-fatal — a network failure does not block the current session.

## Supported versions

Only the latest minor release line receives security fixes. The full table lives in [SECURITY.md](https://github.com/Z-Only/kairox/blob/main/SECURITY.md) and is the source of truth.

If you are running an unsupported version and find a security issue, the maintainer's response will typically be "please upgrade to the supported line and retest." We do not backport fixes to older minors.

## Reporting a vulnerability

::: warning Do not file a public issue for security problems.
Use private reporting. A public issue gives attackers a heads-up before a fix exists.
:::

Report privately via [**GitHub Security Advisories**](https://github.com/Z-Only/kairox/security/advisories/new) on the Kairox repository. The advisory flow lets the maintainer triage, prepare a fix, and coordinate disclosure without exposing the issue publicly.

If GitHub Security Advisories are unavailable, contact the repository owner directly:

- GitHub: [@Z-Only](https://github.com/Z-Only)

When you report, include:

- **affected component** — e.g., `agent-tools` shell executor, `agent-mcp` SSE transport, Tauri command handler.
- **reproduction steps** — minimal config + commands or a script.
- **impact assessment** — what an attacker can do, what they need (local access? specific config? a malicious MCP server?).
- **suggested mitigation** — even a rough sketch helps.

The maintainer will acknowledge valid reports as quickly as possible and coordinate a coordinated disclosure window before publishing a fixed release.

## Privacy and telemetry

Kairox has no telemetry. Nothing about your sessions, prompts, tool calls, or environment is sent anywhere except to the model provider you configure.

The runtime defaults to **minimal trace** in production when a real model client or shell tool is configured. This is enforced in code (not in TOML), so a misconfiguration cannot accidentally enable verbose tracing in a production deployment. Verbose tracing is allowed only when the configured providers and tools are demonstrably safe for development — e.g., the `fake` provider with no real shell. See [Configuration](../reference/configuration#privacy-defaults).

The desktop auto-updater only contacts GitHub Releases to check for new versions. There is no Kairox-operated update server.

## Reproducible builds

Source builds from a tagged commit should produce identical Rust binaries given the same toolchain version (pinned by `rust-toolchain.toml`). Tauri bundles differ slightly across hosts because of platform-bundle plumbing, but the embedded JavaScript and Rust artifacts are identical to a release build run on the same matrix.

If you suspect a published artifact differs from source, file a security advisory — that is a credibility issue we treat seriously.

## What this page does not cover

This page is the release model and the security policy. It does not cover the contribution workflow ([Contributing](./contributing)), the roadmap of what is shipping next ([Roadmap](./roadmap)), or the runtime architecture you may want to understand before reporting an issue ([Architecture](../concepts/architecture)).
