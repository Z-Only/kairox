# Brand Assets Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh Kairox's logo, banner, README top matter, and Tauri app icons so the project has one coherent "local-first AI agent workbench" brand system.

**Architecture:** Use `docs/assets/logo.svg` as the canonical source mark. Use `docs/assets/banner.svg` as the README's single primary brand surface. Regenerate all Tauri icon rasters from the updated SVG source via `pnpm exec tauri icon`.

**Tech Stack:** SVG, Markdown, Tauri CLI 2.x, pnpm, macOS `sips`/`file` for icon verification.

---

## File Structure

- Modify `docs/assets/logo.svg`: canonical app-icon-safe monogram source.
- Modify `docs/assets/banner.svg`: README/release banner integrating the refreshed logo mark and workbench geometry.
- Modify `README.md`: remove the standalone logo image immediately after the intro paragraph.
- Modify `apps/agent-gui/src-tauri/icons/**`: generated Tauri icon assets from the refreshed logo.
- No Rust, Vue, TypeScript generated bindings, or package metadata changes.

## Task 1: Refresh Logo SVG

**Files:**

- Modify: `docs/assets/logo.svg`

- [ ] **Step 1: Replace the logo source**

Replace the full contents of `docs/assets/logo.svg` with:

```svg
<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc">
  <title id="title">Kairox logo</title>
  <desc id="desc">A compact geometric K monogram on a dark rounded app tile for the Kairox local-first AI agent workbench.</desc>
  <defs>
    <linearGradient id="tile" x1="32" y1="24" x2="224" y2="232" gradientUnits="userSpaceOnUse">
      <stop stop-color="#07111D"/>
      <stop offset="0.52" stop-color="#0F172A"/>
      <stop offset="1" stop-color="#111827"/>
    </linearGradient>
    <linearGradient id="mark" x1="68" y1="52" x2="198" y2="206" gradientUnits="userSpaceOnUse">
      <stop stop-color="#2DD4BF"/>
      <stop offset="0.48" stop-color="#38BDF8"/>
      <stop offset="1" stop-color="#8B5CF6"/>
    </linearGradient>
    <linearGradient id="edge" x1="32" y1="28" x2="224" y2="228" gradientUnits="userSpaceOnUse">
      <stop stop-color="#475569"/>
      <stop offset="1" stop-color="#1E293B"/>
    </linearGradient>
  </defs>
  <rect x="16" y="16" width="224" height="224" rx="52" fill="url(#tile)"/>
  <rect x="17" y="17" width="222" height="222" rx="51" stroke="url(#edge)" stroke-opacity="0.78" stroke-width="2"/>
  <path d="M54 88H202" stroke="#1E293B" stroke-width="6" stroke-linecap="round" opacity="0.48"/>
  <path d="M54 128H202" stroke="#1E293B" stroke-width="6" stroke-linecap="round" opacity="0.6"/>
  <path d="M54 168H202" stroke="#1E293B" stroke-width="6" stroke-linecap="round" opacity="0.38"/>
  <path d="M74 62C74 57.5817 77.5817 54 82 54H104C108.418 54 112 57.5817 112 62V112.4L159.2 58.2C161.48 55.5805 164.784 54 168.256 54H197.4C204.229 54 207.862 62.0713 203.34 67.186L147.63 130.19L205.715 188.411C210.736 193.444 207.171 202 200.061 202H171.802C168.65 202 165.681 200.516 163.788 197.996L112 129.02V194C112 198.418 108.418 202 104 202H82C77.5817 202 74 198.418 74 194V62Z" fill="url(#mark)"/>
  <path d="M158 62L112 114V128L176 54H168.256C164.784 54 161.48 55.5805 159.2 58.2L158 62Z" fill="#E0F2FE" opacity="0.42"/>
</svg>
```

- [ ] **Step 2: Inspect SVG text for stale values**

Run:

```bash
rg -n "v[0-9]|release|TODO|TBD" docs/assets/logo.svg
```

Expected: no output.

## Task 2: Refresh Banner SVG

**Files:**

- Modify: `docs/assets/banner.svg`

- [ ] **Step 1: Replace the banner source**

Replace the full contents of `docs/assets/banner.svg` with:

```svg
<svg width="1280" height="640" viewBox="0 0 1280 640" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc">
  <title id="title">Kairox banner</title>
  <desc id="desc">A restrained dark banner for Kairox showing a compact logo mark, local-first AI agent workbench positioning, and abstract task, trace, memory, and tool panels.</desc>
  <defs>
    <linearGradient id="bannerBg" x1="0" y1="0" x2="1280" y2="640" gradientUnits="userSpaceOnUse">
      <stop stop-color="#06111F"/>
      <stop offset="0.55" stop-color="#0F172A"/>
      <stop offset="1" stop-color="#142018"/>
    </linearGradient>
    <linearGradient id="tile" x1="88" y1="72" x2="198" y2="182" gradientUnits="userSpaceOnUse">
      <stop stop-color="#07111D"/>
      <stop offset="0.52" stop-color="#0F172A"/>
      <stop offset="1" stop-color="#111827"/>
    </linearGradient>
    <linearGradient id="mark" x1="111" y1="88" x2="176" y2="170" gradientUnits="userSpaceOnUse">
      <stop stop-color="#2DD4BF"/>
      <stop offset="0.48" stop-color="#38BDF8"/>
      <stop offset="1" stop-color="#8B5CF6"/>
    </linearGradient>
    <linearGradient id="lineAccent" x1="670" y1="128" x2="1110" y2="468" gradientUnits="userSpaceOnUse">
      <stop stop-color="#2DD4BF"/>
      <stop offset="0.52" stop-color="#38BDF8"/>
      <stop offset="1" stop-color="#A78BFA"/>
    </linearGradient>
  </defs>

  <rect width="1280" height="640" rx="32" fill="url(#bannerBg)"/>
  <rect x="40" y="40" width="1200" height="560" rx="28" fill="#020617" fill-opacity="0.22" stroke="#334155" stroke-opacity="0.5"/>

  <g opacity="0.16" stroke="#64748B" stroke-width="1">
    <path d="M88 112H1192"/>
    <path d="M88 224H1192"/>
    <path d="M88 336H1192"/>
    <path d="M88 448H1192"/>
    <path d="M224 72V568"/>
    <path d="M416 72V568"/>
    <path d="M608 72V568"/>
    <path d="M800 72V568"/>
    <path d="M992 72V568"/>
  </g>

  <g aria-label="Kairox mark">
    <rect x="84" y="78" width="118" height="118" rx="30" fill="url(#tile)"/>
    <rect x="85" y="79" width="116" height="116" rx="29" stroke="#475569" stroke-opacity="0.78" stroke-width="2"/>
    <path d="M104 116H182" stroke="#1E293B" stroke-width="4" stroke-linecap="round" opacity="0.52"/>
    <path d="M104 137H182" stroke="#1E293B" stroke-width="4" stroke-linecap="round" opacity="0.62"/>
    <path d="M104 158H182" stroke="#1E293B" stroke-width="4" stroke-linecap="round" opacity="0.4"/>
    <path d="M116 103C116 100.791 117.791 99 120 99H131C133.209 99 135 100.791 135 103V128.2L158.6 101.1C159.74 99.7903 161.392 99 163.128 99H177.7C181.114 99 182.931 103.036 180.67 105.593L152.815 137.095L181.858 166.206C184.368 168.722 182.585 173 179.03 173H164.901C163.325 173 161.841 172.258 160.894 170.998L135 136.51V169C135 171.209 133.209 173 131 173H120C117.791 173 116 171.209 116 169V103Z" fill="url(#mark)"/>
  </g>

  <g aria-label="Kairox title">
    <text x="84" y="286" fill="#F8FAFC" font-size="82" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-weight="760">Kairox</text>
    <text x="88" y="340" fill="#A7F3D0" font-size="28" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-weight="520">Local-first AI agent workbench</text>
    <text x="88" y="394" fill="#CBD5E1" font-size="23" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif">Rust core · TUI · Tauri GUI · MCP tools</text>
  </g>

  <g aria-label="Workbench preview">
    <rect x="672" y="104" width="466" height="340" rx="24" fill="#0B1220" stroke="#334155"/>
    <rect x="672" y="104" width="466" height="54" rx="24" fill="#111827"/>
    <path d="M672 158H1138" stroke="#334155"/>
    <circle cx="706" cy="131" r="7" fill="#2DD4BF"/>
    <circle cx="730" cy="131" r="7" fill="#F59E0B"/>
    <circle cx="754" cy="131" r="7" fill="#A78BFA"/>
    <rect x="795" y="124" width="166" height="14" rx="7" fill="#334155"/>
    <rect x="700" y="188" width="124" height="210" rx="18" fill="#0F172A" stroke="#334155"/>
    <rect x="724" y="216" width="58" height="10" rx="5" fill="#2DD4BF"/>
    <rect x="724" y="248" width="76" height="9" rx="4.5" fill="#475569"/>
    <rect x="724" y="278" width="62" height="9" rx="4.5" fill="#475569"/>
    <rect x="724" y="338" width="78" height="22" rx="11" fill="#13251F" stroke="#14532D"/>
    <rect x="856" y="188" width="248" height="88" rx="18" fill="#0F172A" stroke="#334155"/>
    <rect x="884" y="216" width="84" height="10" rx="5" fill="#38BDF8"/>
    <rect x="884" y="244" width="174" height="9" rx="4.5" fill="#475569"/>
    <rect x="856" y="310" width="248" height="88" rx="18" fill="#0F172A" stroke="#334155"/>
    <rect x="884" y="338" width="64" height="10" rx="5" fill="#A78BFA"/>
    <rect x="884" y="366" width="188" height="9" rx="4.5" fill="#475569"/>
    <path d="M824 294C866 294 866 252 856 252" stroke="url(#lineAccent)" stroke-width="3" stroke-linecap="round"/>
    <path d="M824 306C866 306 866 352 856 352" stroke="url(#lineAccent)" stroke-width="3" stroke-linecap="round" opacity="0.72"/>
    <circle cx="824" cy="300" r="8" fill="#2DD4BF"/>
    <circle cx="856" cy="252" r="7" fill="#38BDF8"/>
    <circle cx="856" cy="352" r="7" fill="#A78BFA"/>
  </g>

  <g aria-label="Capability labels">
    <rect x="84" y="474" width="154" height="46" rx="14" fill="#0B1220" stroke="#334155"/>
    <text x="112" y="504" fill="#E2E8F0" font-size="18" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif">Rust core</text>
    <rect x="258" y="474" width="148" height="46" rx="14" fill="#0B1220" stroke="#334155"/>
    <text x="286" y="504" fill="#E2E8F0" font-size="18" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif">Memory</text>
    <rect x="426" y="474" width="156" height="46" rx="14" fill="#0B1220" stroke="#334155"/>
    <text x="454" y="504" fill="#E2E8F0" font-size="18" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif">Tools</text>
    <rect x="602" y="474" width="164" height="46" rx="14" fill="#0B1220" stroke="#334155"/>
    <text x="630" y="504" fill="#E2E8F0" font-size="18" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif">Permissions</text>
  </g>
</svg>
```

- [ ] **Step 2: Inspect banner for stale release text**

Run:

```bash
rg -n "Current release|v0|release ready|TODO|TBD" docs/assets/banner.svg
```

Expected: no output.

## Task 3: Simplify README Top Matter

**Files:**

- Modify: `README.md`

- [ ] **Step 1: Remove the immediate standalone logo image**

Remove this block from the top of `README.md`:

```markdown
![Kairox logo](https://github.com/Z-Only/kairox/blob/main/docs/assets/logo.svg)
```

The top matter should flow as:

```markdown
# Kairox

![Kairox banner](https://github.com/Z-Only/kairox/blob/main/docs/assets/banner.svg)

[![CI](https://github.com/Z-Only/kairox/actions/workflows/ci.yml/badge.svg)](https://github.com/Z-Only/kairox/actions/workflows/ci.yml)
[![Release Build](https://github.com/Z-Only/kairox/actions/workflows/release-build.yml/badge.svg)](https://github.com/Z-Only/kairox/actions/workflows/release-build.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://github.com/Z-Only/kairox/blob/main/LICENSE)
[![Release](https://img.shields.io/github/v/release/Z-Only/kairox)](https://github.com/Z-Only/kairox/releases)

Kairox is a local-first AI agent workbench built with a shared Rust core, a terminal UI, and a Tauri + Vue desktop GUI.

## Quick links
```

- [ ] **Step 2: Verify the README no longer stacks logo after banner**

Run:

```bash
rg -n "Kairox logo|docs/assets/logo.svg" README.md
```

Expected: no output.

## Task 4: Regenerate Tauri Icons

**Files:**

- Modify: `apps/agent-gui/src-tauri/icons/**`

- [ ] **Step 1: Generate icons from the refreshed logo SVG**

Run:

```bash
cd apps/agent-gui
pnpm exec tauri icon ../../docs/assets/logo.svg --output src-tauri/icons
cd ../..
```

Expected: command exits 0 and rewrites files under `apps/agent-gui/src-tauri/icons/`.

- [ ] **Step 2: Verify representative icon dimensions**

Run:

```bash
sips -g pixelWidth -g pixelHeight apps/agent-gui/src-tauri/icons/32x32.png apps/agent-gui/src-tauri/icons/128x128.png apps/agent-gui/src-tauri/icons/icon.png apps/agent-gui/src-tauri/icons/ios/AppIcon-512@2x.png
```

Expected output includes:

```text
pixelWidth: 32
pixelHeight: 32
pixelWidth: 128
pixelHeight: 128
pixelWidth: 512
pixelHeight: 512
pixelWidth: 1024
pixelHeight: 1024
```

- [ ] **Step 3: Verify bundled icon formats exist**

Run:

```bash
file apps/agent-gui/src-tauri/icons/icon.ico apps/agent-gui/src-tauri/icons/icon.icns
```

Expected: `icon.ico` is a Windows icon file and `icon.icns` is a macOS icon file.

## Task 5: Format And Review

**Files:**

- Modify: `README.md`
- Modify: `docs/assets/logo.svg`
- Modify: `docs/assets/banner.svg`
- Modify: `apps/agent-gui/src-tauri/icons/**`

- [ ] **Step 1: Run targeted formatting check**

Run:

```bash
pnpm prettier --check README.md docs/superpowers/specs/2026-05-06-brand-assets-refresh-design.md docs/superpowers/plans/2026-05-06-brand-assets-refresh.md
```

Expected: all matched files use Prettier style.

- [ ] **Step 2: Review changed files**

Run:

```bash
git status --short
git diff -- README.md docs/assets/logo.svg docs/assets/banner.svg docs/superpowers/plans/2026-05-06-brand-assets-refresh.md
```

Expected: diff shows only the brand asset, README, generated icon, and plan changes.

- [ ] **Step 3: Commit implementation**

Run:

```bash
git add README.md docs/assets/logo.svg docs/assets/banner.svg apps/agent-gui/src-tauri/icons docs/superpowers/plans/2026-05-06-brand-assets-refresh.md
git commit -m "style: refresh brand assets"
```

Expected: commit succeeds.
