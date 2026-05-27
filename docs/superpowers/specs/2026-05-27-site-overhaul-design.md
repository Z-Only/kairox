# Website overhaul (VitePress + GitHub Pages)

Status: approved 2026-05-27
Scope: documentation site under `site/`, GitHub Pages workflow, supporting build scripts
Owners: docs
Non-goals: Rust changes, GUI/IPC changes, runtime behavior changes

## Problem

The current VitePress site (commit `65ac71ec`) ships three thin content pages per
locale: Home, Architecture (~22 lines), Getting Started (~30 lines). The site
underuses the source material that already exists in the repo (`AGENTS.md` —
619 lines, `README.md` — 263 lines, `ROADMAP.md`, `CONTRIBUTING.md`,
`docs/releasing.md`, `docs/dev/local-development.md`, 12 crates with rich
public APIs) and underuses common VitePress + GitHub Pages capabilities
(Mermaid, edit-on-GitHub, footer, llms.txt, custom 404, build-time release
metadata).

The result is a landing page that promises a workbench but cannot answer
basic questions: how do I install for my OS, what is in each crate, how do
permissions work, how do I configure a model, what does the runtime actually
do on each turn.

This spec describes a single-PR overhaul that expands the site to roughly
16 content pages per locale (32 markdown files total), deepens the
Architecture page in particular, and adds best-practice GitHub Pages
features that future contributors expect.

## Goals

1. Provide depth that matches the repo: per-page substance of 200-600 lines
   instead of the current ~22.
2. Full English + Simplified Chinese parity for every new page in the same
   PR — no half-localized navigation.
3. Add the VitePress / GitHub Pages features that are standard for project
   sites of this category: Mermaid, edit-on-GitHub, footer, custom 404,
   llms.txt for AI ingestion, small inline feedback widget, build-time
   release banner.
4. Source-of-truth discipline for content that already lives elsewhere in
   the repo (`ROADMAP.md`, `CONTRIBUTING.md`, `SECURITY.md`, release docs)
   — curate highlights on the site, link out for canonical text.
5. Ship as one squash auto-merge PR with no Rust touched.

## Information architecture

Top-level navigation, sidebar, and 16 content pages per locale:

```
Home (existing, polish)

Guide  (sidebar group — onboarding)
├── Getting Started      (exists, deepen)
├── Installation         (new — per-OS deps, Tauri prereqs, bun, just)
├── First Session        (new — TUI + GUI walkthrough)
└── Troubleshooting & FAQ (new)

Concepts  (sidebar group — understanding)
├── Architecture                (full rewrite — Mermaid diagrams)
├── Runtime & Sessions          (agent loop, events, DAG, multi-agent)
├── Memory & Context            (<memory> protocol, assembly, compaction)
├── Permissions & Tools         (5 modes, built-in tools, MCP adapter)
└── Extensibility: MCP / Skills / Plugins  (combined; three sub-sections)

Reference  (sidebar group — lookup)
├── Configuration         (TOML schema, profiles, .kairox/, env vars)
├── Crate Index           (every crate with purpose, key types, repo link)
└── CLI & Keyboard        (just recipes + TUI keymap + GUI shortcuts)

Community  (sidebar group — hybrid, curated + link-out)
├── Roadmap               (curated highlights → ROADMAP.md on GitHub)
├── Contributing          (workflow summary → CONTRIBUTING.md on GitHub)
└── Releases & Security   (release model + security policy + changelog links)

External nav: GitHub · Releases · Discussions
```

Removed: `site/guide/architecture.md` and `site/zh/guide/architecture.md`
are deleted; their content is rewritten and moved under `concepts/`.

Folds taken to stay under 16 pages: no standalone per-crate pages (rolled
into Crate Index), no standalone Hooks / Instructions / Workspaces pages
(rolled into Configuration + Extensibility).

## Best-practice features

1. **Mermaid** — add `vitepress-plugin-mermaid` + `mermaid` as devDeps,
   wrap `defineConfig` with `withMermaid(...)`. Used for architecture
   layers, dependency graph, agent loop sequence, prompt assembly pipeline,
   permission decision flow.
2. **Edit-on-GitHub** — `themeConfig.editLink` with pattern
   `https://github.com/Z-Only/kairox/edit/main/site/:path`. Locale-aware
   label.
3. **Footer** — `themeConfig.footer` with license + copyright. Localized.
4. **llms.txt + llms-full.txt** — Node post-build script
   `scripts/generate-llms-txt.mjs` walks `site/**/*.md` (EN canonical),
   emits `dist/llms.txt` (page index with title + 1-line summary) and
   `dist/llms-full.txt` (concatenated cleaned markdown). Hooked into
   `site:build`.
5. **Custom 404** — `site/404.md` and `site/zh/404.md` with `layout: page`,
   local search component, locale-aware links back to Home / Getting
   Started / GitHub.
6. **Feedback widget** — small `FeedbackBlock.vue` registered via
   `theme/index.ts` `enhanceApp` and injected into `Layout.vue` `doc-after`
   slot. Two buttons opening prefilled GitHub Discussion URLs:
   `helpful` and `needs-improvement`. No client-side state, no analytics.
7. **Release banner** — `pages.yml` adds a step calling
   `gh release view --json tagName,publishedAt,assets` and writing
   `site/.vitepress/cache/release.json`. A `ReleaseBanner.vue` component
   imports the JSON at build time and renders version + per-OS download
   links at the top of Getting Started + Installation. Falls back to
   "Latest release →" link when JSON is absent (local dev).

Trade-off accepted: release banner is build-time only. Refreshes only
when CI rebuilds the site. Keeps the site fully static and offline-safe.

Trade-off accepted: feedback widget points at GitHub Discussions
`site-feedback` category; if the category does not exist yet, the URL
still lands users on the new-discussion page where they can choose a
category.

## Content depth per page

Targets are guidelines, not hard limits. Substance over length.

### Guide

- **Getting Started** (~300 lines). 5-minute path. Release banner at top.
  Quickstart, prerequisites, install, first TUI session, first GUI
  session, what to read next.
- **Installation** (~400 lines). Per-OS prerequisites (macOS, Linux,
  Windows), Rust + Node + Bun + just, build from source for TUI and GUI,
  troubleshooting common install errors. Release banner.
- **First Session** (~500 lines). Step-by-step walkthrough with
  screenshots: configure a model profile, run TUI, run GUI, observe
  permission flow, switch model mid-session, trigger compaction. Mermaid
  sequence diagram for end-to-end turn.
- **Troubleshooting & FAQ** (~400 lines). Curated sections: model errors,
  MCP server start failures, permission denied, GUI launch failures, where
  data lives, how to reset memory, how to enable verbose logging.

### Concepts (the depth core)

- **Architecture** (~600 lines, 4-5x current). Mermaid layered
  architecture diagram, per-layer responsibilities table (mirroring
  AGENTS.md), dependency direction rule, trait boundaries and rationale,
  event-sourced state model, crate dependency graph (Mermaid), decision
  log (why facade, why event store, why split runtime modules).
- **Runtime & Sessions** (~500 lines). Agent loop, session lifecycle, event
  payload taxonomy table, task graph + DAG executor, multi-agent strategies
  (Planner / Worker / Reviewer), model switching with budget guards, MCP
  lifecycle. Mermaid sequence diagram for a single user turn.
- **Memory & Context** (~400 lines). `<memory>` marker protocol with
  scopes and approval semantics, memory store, context assembler with
  tiktoken budgets, manual + automatic compaction, busy-state guards.
  Mermaid prompt assembly pipeline.
- **Permissions & Tools** (~400 lines). Five permission modes with a table
  of what each blocks / prompts / allows, built-in tools table with risk
  classification, Mermaid permission decision flow, MCP tool adapter
  explanation.
- **Extensibility: MCP / Skills / Plugins** (~600 lines). Three
  sub-sections: MCP (client architecture, transports, lifecycle,
  marketplace, example server config), Skills (SkillDef, frontmatter
  reference, scopes, discovery, SkillHub install), Plugins (manifest,
  inventory, settings, plugin-namespaced skills).

### Reference

- **Configuration** (~500 lines). Annotated TOML schema for
  `~/.kairox/config.toml`, model profile examples for each provider
  (OpenAI, Anthropic, Ollama, Fake), `.kairox/` project discovery, env
  vars, hooks settings, instructions settings, permission defaults,
  workspace settings. Cookbook style.
- **Crate Index** (~400 lines). Table of all 12 crates with purpose,
  public traits, key types, depended-on-by, repo path link. Dependency
  direction rule restated. Mermaid dependency graph.
- **CLI & Keyboard** (~400 lines). Every `just` recipe with one-line
  description and when to use; every `bun` script; TUI keymap reference;
  GUI keyboard shortcuts and command palette commands.

### Community (hybrid pages)

- **Roadmap** (~150 lines). Curated highlights from `ROADMAP.md`.
  Prominent "Source of truth: `ROADMAP.md` on GitHub" callout.
- **Contributing** (~200 lines). Workflow summary (worktree → commit →
  PR), conventional-commit scopes table, quality gates, where to ask for
  help. Link to canonical `CONTRIBUTING.md`.
- **Releases & Security** (~250 lines). Release model (semver, what
  triggers a release, what's in an artifact), how to verify checksums,
  auto-update behavior, supported versions, security disclosure flow.
  Links to `docs/releasing.md`, `SECURITY.md`, latest GitHub Release.

## File inventory

```
.github/workflows/pages.yml                    (modify — add release fetch)
package.json                                   (modify — mermaid deps; site:build)
scripts/generate-llms-txt.mjs                  (new)

site/.vitepress/config.ts                      (rewrite — nav/sidebar/edit/footer/withMermaid)
site/.vitepress/theme/index.ts                 (modify — register components, doc-after slot)
site/.vitepress/theme/custom.css               (extend — feedback, mermaid, release banner)
site/.vitepress/theme/components/FeedbackBlock.vue    (new)
site/.vitepress/theme/components/ReleaseBanner.vue    (new)

site/index.md                                  (polish)
site/404.md                                    (new)

site/guide/getting-started.md                  (rewrite)
site/guide/installation.md                     (new)
site/guide/first-session.md                    (new)
site/guide/troubleshooting.md                  (new)

site/concepts/architecture.md                  (new — replaces site/guide/architecture.md)
site/concepts/runtime-and-sessions.md          (new)
site/concepts/memory-and-context.md            (new)
site/concepts/permissions-and-tools.md         (new)
site/concepts/extensibility.md                 (new)

site/reference/configuration.md                (new)
site/reference/crate-index.md                  (new)
site/reference/cli-and-keyboard.md             (new)

site/community/roadmap.md                      (new)
site/community/contributing.md                 (new)
site/community/releases-and-security.md        (new)

site/public/screenshots/                       (extend — only if a new shot is strictly needed)

# ZH mirror — same paths under site/zh/
site/zh/404.md
site/zh/guide/{getting-started,installation,first-session,troubleshooting}.md
site/zh/concepts/{architecture,runtime-and-sessions,memory-and-context,permissions-and-tools,extensibility}.md
site/zh/reference/{configuration,crate-index,cli-and-keyboard}.md
site/zh/community/{roadmap,contributing,releases-and-security}.md

# Deleted
site/guide/architecture.md
site/zh/guide/architecture.md
```

Total: 32 new/rewritten markdown files, 2 new Vue components, 1 new Node
script, 3 modified config files, 2 deletions.

## Implementation ordering

Single PR, four logical stages applied in order, all committed under
branch `docs/site-overhaul`:

1. **Foundation** — install Mermaid deps, rewrite `config.ts`, add theme
   components, extend `custom.css`, add 404 pages, add llms.txt generator,
   update `package.json` `site:build`, update `pages.yml` to fetch release
   JSON. Verify with `bun run site:dev` and a Mermaid smoke block.
2. **EN Concepts + Reference** — depth core. Architecture rewrite first,
   then Runtime, Memory, Permissions, Extensibility, Configuration, Crate
   Index, CLI & Keyboard. Delete old `site/guide/architecture.md`.
3. **EN Guide + Community + Home polish** — Getting Started rewrite,
   Installation, First Session, Troubleshooting, Roadmap, Contributing,
   Releases & Security, home page tightening.
4. **ZH mirror** — translate every new/rewritten page, idiomatic ZH not
   literal. Reuse Mermaid blocks verbatim. Delete old
   `site/zh/guide/architecture.md`. Verify ZH sidebar/nav coverage matches
   EN.

## Verification matrix

Docs-only PR. No Rust, no IPC, no GUI behavior change.

| Check                   | Command                                                                                                      | When          |
| ----------------------- | ------------------------------------------------------------------------------------------------------------ | ------------- |
| Format                  | `bun run format:check`                                                                                       | Before commit |
| Lint                    | `bun run lint`                                                                                               | Before commit |
| VitePress build (EN+ZH) | `bun run site:build`                                                                                         | Before commit |
| llms.txt generated      | check `site/.vitepress/dist/llms.txt` + `llms-full.txt` exist                                                | After build   |
| Dev server smoke        | `bun run site:dev`, manually visit Home / Architecture / Crate Index / 404 / a ZH page, expand every sidebar | Before commit |
| Mermaid renders         | Visual check on Architecture + Runtime pages                                                                 | During dev    |
| Edit-link path          | Click on 3 pages and confirm it points to the correct `site/...` path on GitHub                              | During dev    |
| Release banner fallback | Local dev with no `release.json` shows "Latest release →" link                                               | During dev    |
| Feedback widget URLs    | Click on 2 pages and confirm prefilled Discussion URL                                                        | During dev    |
| Internal link sanity    | Manual nav-by-nav                                                                                            | Before commit |

Explicitly **skipped**: `cargo test --workspace --all-targets` (no Rust
touched), `just gen-types` (no IPC touched), `tauri-pilot` (no GUI app
behavior touched), Vitest (no Vue app components touched), Playwright
(no GUI E2E surface touched).

## Risks

- Mermaid plugin pulls a meaningful devDep tree (~1MB). Acceptable.
- `llms-full.txt` may be 200-500KB. Linked as artifact only, not in nav.
- `gh release view` is available on `ubuntu-latest` by default. No new
  workflow permissions required.
- `site-feedback` Discussion category may not exist; URL still lands on
  the new-discussion page. Tolerable.
- Screenshots for First Session — prefer reusing the existing
  `workbench.png` and `settings.png`. Only add new captures if strictly
  needed, to keep PR scope tight.
- Build-time release banner: stale until next CI rebuild. Acceptable
  trade-off for static-site simplicity.

## Out of scope

- No standalone per-crate pages (rolled into Crate Index)
- No standalone Hooks / Instructions / Workspaces pages
- No interactive playground, live model demo, analytics, comment system,
  newsletter capture
- No slash-command or skill content beyond what is in Extensibility
- ZH content matches structure 1:1 but uses idiomatic ZH, not literal
  translation

## Completion signal

Squash-merged PR against `main` containing all 32 new/rewritten markdown
files plus configuration, theme, and CI changes. GitHub Pages workflow
deploys cleanly. Local `bun run site:dev` walks every nav item without
404s. Build emits `llms.txt` and `llms-full.txt`.
