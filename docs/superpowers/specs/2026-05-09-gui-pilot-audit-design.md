# GUI Pilot Audit — Design Spec

**Date:** 2026-05-09
**Status:** Draft (waiting for user review)
**Scope:** Spec A: run a full tauri-pilot-driven GUI UI/UX audit, fix all P0/P1/P2 findings, and convert P3 improvements into GitHub issues
**Target branch:** `feat/gui-pilot-audit-fixes` (single PR)
**Related work:** Spec B will extract the reusable workflow after this spec is complete

---

## 1. Purpose and Context

`apps/agent-gui` already has a real desktop E2E stack through [tauri-plugin-pilot](https://github.com/mpiton/tauri-pilot) and `just test-pilot`. However, the current three `apps/agent-gui/e2e-pilot/*.toml` scenarios are shallow and mostly verify that the sidebar renders. The GUI now includes Workbench, Marketplace, Settings, and more than ten core components, but it has not yet been systematically audited with a real desktop driver.

This spec has four goals:

1. **Drive:** Use the `tauri-pilot` CLI to inspect, interact with, and debug a running Tauri GUI in real time using a debug build with `--features pilot`.
2. **Discover:** Exhaustively identify all P0/P1/P2 UI/UX issues with a structured rubric covering UX, performance/jank, reduced motion, and severe accessibility.
3. **Fix:** Fix every P0/P1/P2 issue in the same branch. Convert P3 nice-to-have findings into GitHub issues.
4. **Prevent regression:** Encode discovered issues as assertions in new `audit-*.toml` scenarios and include them in CI.

**Out of scope:** This spec does not extract the methodology into reusable documentation (that belongs to Spec B), redesign the design system, introduce a third-party UI library, fix upstream `tauri-pilot` bugs, or modify Rust crates. If a GUI issue is clearly rooted in Rust-side behavior, such as a missing event payload field, create an issue instead.

---

## 2. Audit Scope

### 2.1 Five Core Scenarios, Existing Selectors, and Missing Selectors

The table below is based on `grep -rn "data-test" apps/agent-gui/src/`. The **Existing anchors** column lists selectors that have been verified to exist. The **Anchors to add** column lists selectors that must be added in the first implementation commit.

| #   | Scenario                    | Components                                                                                                                                                             | Existing anchors verified by grep                                                                                                                                                                                                                                                                                                                          | Anchors to add in PR commit 1                                                                                                                                                                                                                         | TOML scenario                                            |
| --- | --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| S1  | Bootstrap                   | `AppLayout` + `WorkbenchView` + `SettingsView`                                                                                                                         | `[data-test='app-shell']`, `[data-test='app-nav']`, `[data-test='nav-workbench']`, `[data-test='nav-settings']`, `[data-test='sessions-sidebar']`, `[data-test='status-bar']`                                                                                                                                                                              | `view-workbench`, `view-marketplace`, `view-settings` as view-root anchors; `theme-toggle` for the Settings theme toggle                                                                                                                              | `audit-bootstrap.toml` upgrading `app-bootstrap.toml`    |
| S2  | Sessions Lifecycle          | `SessionsSidebar` + `ConfirmDialog`                                                                                                                                    | `new-session-btn`, `create-session-btn`, `session-item`, `session-delete-btn`, `sessions-empty`, `confirm-cancel`, `confirm-ok`                                                                                                                                                                                                                            | `new-session-dialog`, `session-rename-btn`, `session-rename-input`, `session-rename-confirm`                                                                                                                                                          | `audit-sessions.toml` upgrading `session-lifecycle.toml` |
| S3  | Chat Streaming              | `ChatPanel` + `TraceTimeline` + `TaskSteps` + `TaskNode` + `PermissionPrompt`                                                                                          | `message-input`, `send-button`, `cancel-button`, `cancelled-marker`, `trust-server-checkbox`                                                                                                                                                                                                                                                               | `chat-message` with `data-role="user\|assistant"`, `chat-empty-state`, `stream-indicator`, `error-banner`, `trace-timeline`, `trace-entry`, `task-steps`, `task-node`, `task-node-status`, `permission-prompt`, `permission-allow`, `permission-deny` | `audit-chat.toml` upgrading `chat-flow.toml`             |
| S4  | MCP Server Manager          | `McpServerManager` + `McpStatusIndicator`                                                                                                                              | None                                                                                                                                                                                                                                                                                                                                                       | `mcp-manager`, `mcp-empty-state`, `mcp-server-item`, `mcp-server-name`, `mcp-server-status`, `mcp-server-error`, `mcp-start-btn`, `mcp-stop-btn`, `mcp-trust-btn`, `mcp-revoke-btn`, `mcp-close-btn`, `mcp-status-indicator`                          | `audit-mcp.toml` as a new scenario                       |
| S5  | Marketplace + MemoryBrowser | `MarketplaceView` + `MarketplacePane` + `CatalogList` + `CatalogCard` + `CatalogDetail` + `InstallProgress` + `InstalledList` + `RuntimeMissingHint` + `MemoryBrowser` | `tab-browse`, `tab-installed`, `source-chip-*`, `src-warn-*`, `catalog-source-settings`, `catalog-source-settings-drawer`, `catalog-search`, `catalog-trust`, `catalog-refresh`, `catalog-card`, `catalog-detail`, `env-*`, `catalog-install`, `installed-list`, `uninstall-*`, `install-progress`, `install-close`, `runtime-hint`, `memory-scope-select` | `memory-browser`, `memory-list`, `memory-item`, `memory-empty-state`, `memory-refresh-btn`, `memory-delete-btn`                                                                                                                                       | `audit-marketplace-memory.toml` as a new scenario        |

**Convention:** Adding `data-test` attributes must only add selectors. It must not change logic, props, or rendering. Vitest should be able to query the same anchors with `wrapper.find('[data-test=...]')`, and existing selectors in the current Vitest files must remain compatible. Commit message: `test(gui): add data-test anchors required by pilot audit scenarios`.

### 2.2 Workaround for the `ChatPanel` `<textarea>` Input Bug

In `tauri-pilot` v0.5.0, the `fill`/`type` handlers use the `HTMLInputElement.value` setter on `<textarea>`, which throws. The existing `chat-flow.toml` includes a note about this. The workaround is to set the textarea value directly and dispatch an input event.

Implement the workaround in `scripts/audit-helpers.sh`. The interface contract below is fixed and is not a placeholder:

```bash
# scripts/audit-helpers.sh — helper function library for manual audit scripts
# Usage: source scripts/audit-helpers.sh
# Prerequisite: tauri-pilot is connected to a running GUI. Either run
# scripts/run-pilot-tests.sh manually, or run ./target/debug/agent-gui-tauri &
# and verify with tauri-pilot ping.
# This script is not called by CI. CI still uses scripts/run-pilot-tests.sh + audit-*.toml.

# pilot_fill_textarea <css-selector> <text>
#   Work around the tauri-pilot v0.5.0 textarea fill bug.
#   Exit code: 0 on success; non-zero when the selector is not found or dispatch throws.
#   Error details go to stderr.
pilot_fill_textarea() {
  local selector="$1" text="$2"
  tauri-pilot eval - <<EOF || return 1
const el = document.querySelector(${selector@Q});
if (!el) throw new Error('selector not found: ' + ${selector@Q});
const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set;
setter.call(el, ${text@Q});
el.dispatchEvent(new Event('input', { bubbles: true }));
'ok'
EOF
}

# pilot_set_reduced_motion <on|off>
#   Inject or remove <style id="audit-reduced-motion">. This disables every
#   transition / animation and simulates system-level prefers-reduced-motion,
#   which tauri-pilot cannot toggle directly.
pilot_set_reduced_motion() {
  local mode="$1"
  if [[ "$mode" == "on" ]]; then
    tauri-pilot eval - <<'EOF'
let s = document.getElementById('audit-reduced-motion');
if (!s) {
  s = document.createElement('style');
  s.id = 'audit-reduced-motion';
  s.textContent = '*,*::before,*::after{transition:none!important;animation:none!important}';
  document.head.appendChild(s);
}
'on'
EOF
  else
    tauri-pilot eval - <<'EOF'
const s = document.getElementById('audit-reduced-motion');
if (s) s.remove();
'off'
EOF
  fi
}

# pilot_measure_fps <duration-ms>
#   Measure average FPS with a requestAnimationFrame counter.
#   Prints only the numeric value to stdout.
pilot_measure_fps() {
  local duration="$1"
  tauri-pilot eval - <<EOF
new Promise(resolve => {
  let frames = 0;
  const start = performance.now();
  function tick() {
    frames++;
    if (performance.now() - start < ${duration}) requestAnimationFrame(tick);
    else resolve((frames * 1000 / (performance.now() - start)).toFixed(1));
  }
  requestAnimationFrame(tick);
})
EOF
}

# pilot_collect_evidence <scenario-id>
#   Writes snapshot/screenshots/logs/network/axe under
#   audit-runs/<scenario>-<UTC-ts>/.
#   Depends on the unified screenshot dimensions in §3.2.
#
#   Correct theme switching: useUiStore uses useStorage('kairox.color-mode').
#   Directly toggling html.dark would be overwritten on the next reactive store
#   update. Instead, write the localStorage key and dispatch a storage event to
#   trigger useStorage synchronization, then restore the original value after
#   screenshots to avoid polluting the user's persistent preference.
pilot_collect_evidence() {
  local scenario="$1"
  local ts; ts="$(date -u +%Y%m%dT%H%M%SZ)"
  local dir="audit-runs/${scenario}-${ts}"
  mkdir -p "${dir}/screenshots"
  tauri-pilot snapshot -i --json > "${dir}/snapshot.json"
  tauri-pilot logs --level error > "${dir}/logs.txt" || true
  tauri-pilot network --failed > "${dir}/network.json" || true

  # 1) Back up the previous theme and force a light-mode screenshot.
  # tauri-pilot eval returns a JSON-RPC result; string values are JSON-encoded
  # with outer quotes. Use jq -r to strip them; fall back to sed when jq is not available.
  local prev_theme_raw prev_theme
  prev_theme_raw="$(tauri-pilot eval - <<'EOF'
const v = localStorage.getItem('kairox.color-mode'); v ?? 'auto'
EOF
)"
  if command -v jq >/dev/null 2>&1; then
    prev_theme="$(printf '%s' "$prev_theme_raw" | jq -r '.')"
  else
    prev_theme="$(printf '%s' "$prev_theme_raw" | sed -e 's/^"//' -e 's/"$//')"
  fi
  case "$prev_theme" in auto|light|dark) ;; *) prev_theme="auto" ;; esac
  tauri-pilot eval - <<'EOF' >/dev/null
localStorage.setItem('kairox.color-mode', 'light');
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: 'light' }));
'light'
EOF
  # Wait two frames for Vue reactivity and class binding to settle.
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  tauri-pilot screenshot "${dir}/screenshots/light.png"

  # 2) Force a dark-mode screenshot.
  tauri-pilot eval - <<'EOF' >/dev/null
localStorage.setItem('kairox.color-mode', 'dark');
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: 'dark' }));
'dark'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  tauri-pilot screenshot "${dir}/screenshots/dark.png"

  # 3) Restore the original theme, then capture reduced-motion on the restored real theme.
  tauri-pilot eval - <<EOF >/dev/null
localStorage.setItem('kairox.color-mode', ${prev_theme@Q});
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: ${prev_theme@Q} }));
'restored'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  pilot_set_reduced_motion on
  tauri-pilot screenshot "${dir}/screenshots/reduced-motion.png"
  pilot_set_reduced_motion off

  # 4) Full-page axe-core scan.
  pilot_run_axe > "${dir}/axe.json" || true

  echo "${dir}"
}
```

**Do not modify `scripts/run-pilot-tests.sh`.** It remains the CI entry point and runs every `*.toml` scenario. `audit-helpers.sh` is a separate helper library for the manual + AI exploration phase.

---

## 3. Evaluation Rubric

This rubric is fixed so findings do not depend on subjective judgment.

| Priority | Trigger conditions; classify into this level if any condition applies                                                                                                                                                                                                                                                                                           | Handling                |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| **P0**   | (a) App crash / blank screen / `tauri-pilot ping` failure<br>(b) Core path blocked: cannot send a message, create/switch session fails, permission prompt gets stuck<br>(c) Severe a11y: keyboard cannot reach a core control, text contrast < 3:1, or focus is completely invisible                                                                            | Must fix                |
| **P1**   | (a) Error state has no visible user feedback, such as missing toast or inline error<br>(b) Long operation has no loading state or button disabled state<br>(c) Main-flow contrast is between 3:1 and 4.5:1<br>(d) Streaming output jitters or long lists reflow, sustained FPS < 30<br>(e) Missing bilingual i18n keys for `common.*`, `nav.*`, or `settings.*` | Must fix                |
| **P2**   | (a) Visual consistency issue: spacing, border radius, or shadow token mismatch<br>(b) Missing or inconsistent hover/focus micro-interaction<br>(c) `prefers-reduced-motion` is not respected<br>(d) Empty state or first screen lacks a skeleton<br>(e) Non-core-path a11y issue, such as missing `aria-label`                                                  | Must fix                |
| **P3**   | (a) Style suggestion, such as more modern visuals or more refined motion<br>(b) Incremental feature, such as search/filtering or keyboard shortcuts<br>(c) Refactor suggestion, such as extracting design tokens or splitting components                                                                                                                        | Convert to GitHub issue |

### 3.1 Unified Evidence Checklist per Scenario

Each scenario must produce the following artifacts in one run through `pilot_collect_evidence <scenario-id>` from §2.2:

- ✅ `audit-runs/<scenario>-<UTC-ts>/snapshot.json` — `tauri-pilot snapshot -i --json` to verify the accessibility tree
- ✅ `audit-runs/<scenario>-<UTC-ts>/screenshots/light.png` — default light-mode screenshot
- ✅ `audit-runs/<scenario>-<UTC-ts>/screenshots/dark.png` — screenshot after switching to dark mode
- ✅ `audit-runs/<scenario>-<UTC-ts>/screenshots/reduced-motion.png` — screenshot after injecting the `<style id="audit-reduced-motion">` from §2.2
- ✅ `audit-runs/<scenario>-<UTC-ts>/logs.txt` — `tauri-pilot logs --level error`
- ✅ `audit-runs/<scenario>-<UTC-ts>/network.json` — `tauri-pilot network --failed`
- ✅ `audit-runs/<scenario>-<UTC-ts>/axe.json` — full-page a11y scan result from `pilot_run_axe`
- ✅ Before / during / after screenshots for key interactions. Capture these manually during the audit phase and name them `${dir}/screenshots/<step>-<phase>.png`, where `${dir}` is the path echoed by `pilot_collect_evidence`. Do not write these timestamped paths into `audit-*.toml`; TOML is the stable CI regression assertion layer.

**About 320px narrow screens:** Tauri desktop apps have a minimum usable width defined by the window size rather than the viewport, and `tauri-pilot` cannot resize the window. This spec does not require a 320px narrow-screen audit. It only requires checking naturally narrow containers, such as the model/profile list in Settings, against `light.png` for truncation and wrapping. Treat discovered narrow-column text breakage as P2.

### 3.2 Quantitative Measurement Methods

These methods are fixed to reduce subjectivity.

| Measurement                        | Implementation                                                                                          | Trigger                                                                                                                            | Threshold                                                                                                                                                                                                               |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Average FPS**                    | `pilot_measure_fps <ms>` from §2.2                                                                      | Run during streaming output, with a 5000ms window                                                                                  | Average < 30 → P1 (d)                                                                                                                                                                                                   |
| **Text contrast / full-page a11y** | `pilot_run_axe` from §3.2.1 with offline axe-core                                                       | Run once in the evidence collection phase for every scenario and write JSON to `axe.json`                                          | `color-contrast` violation with `impact ∈ {critical, serious}` and ratio < 3 → P0 (c); ratio 3–4.5 → P1 (c); other a11y violation with `impact=critical` → P0 (c), `serious` → P1, `moderate` → P2                      |
| **Keyboard reachability**          | `pilot_probe_tab_order <N>` from §3.2.2                                                                 | Sequentially dispatch or simulate `Tab` focus navigation N times, default 30; output each step's `document.activeElement` selector | Every interactive control listed in the existing and missing anchor columns of §2.1 must appear at least once in the sequence. Missing any P0-critical control → P0 (c)                                                 |
| **Focus visibility**               | `pilot_probe_focus_ring <selector>` from §3.2.3                                                         | For each interactive selector: screenshot unfocused state → focus with `eval el.focus()` → screenshot focused state → pixel diff   | Pixel difference < 0.1%, meaning the focus ring is completely invisible → P0 (c)                                                                                                                                        |
| **Reduced-motion compliance**      | Rerun the streaming scenario after `pilot_set_reduced_motion on`, then measure with `pilot_measure_fps` | Compare FPS and screenshots between on/off states                                                                                  | With reduced motion on, average FPS fluctuation `(max - min) > 5 FPS`, or `pilot_probe_focus_ring` pixel difference between the two screenshots > 0.5%, indicates transform/opacity animation is still running → P2 (c) |

#### 3.2.1 axe-core Injection Strategy

Use the offline strategy only; this is not a choice between CDN and offline.

`apps/agent-gui/src-tauri/tauri.conf.json` currently has `app.security` as an empty object, so no CSP is currently configured and CDN injection would theoretically work. This spec still requires the offline strategy because:

1. It works in CI on Linux + macOS and in offline development environments.
2. It will not silently break if CSP is tightened later.
3. `axe-core` is about 470 KB, which is acceptable.

**Implementation steps** as part of commit §5.1.C3:

1. Add npm dev dependency: `pnpm --filter agent-gui add -D axe-core@^4`
2. Create `apps/agent-gui/public/audit/axe.min.js` by running `cp node_modules/axe-core/axe.min.js apps/agent-gui/public/audit/axe.min.js`
3. Keep the directory with `apps/agent-gui/public/audit/.gitkeep`; commit `axe.min.js` to git. The `apps/agent-gui/public/audit/` path is not covered by the root-anchored `/audit-runs/` ignore rule from §4.2, so there is no conflict.
4. Implement the helper function with `fetch('/audit/axe.min.js')`:

```bash
# Add to scripts/audit-helpers.sh
pilot_run_axe() {
  tauri-pilot eval - <<'EOF'
(async () => {
  if (!window.axe) {
    const code = await (await fetch('/audit/axe.min.js')).text();
    new Function(code)();   // Register window.axe in the global scope.
  }
  const r = await window.axe.run({ resultTypes: ['violations'] });
  return JSON.stringify({
    violations: r.violations.map(v => ({
      id: v.id, impact: v.impact, help: v.help,
      nodes: v.nodes.map(n => ({ target: n.target, html: n.html.slice(0, 200) }))
    }))
  });
})()
EOF
}
```

#### 3.2.2 Keyboard Tab Order Probe

```bash
# Add to scripts/audit-helpers.sh
# pilot_probe_tab_order <count>  → stdout: JSON array of {step, selector}
pilot_probe_tab_order() {
  local count="${1:-30}"
  tauri-pilot eval - <<EOF
(async () => {
  const trail = [];
  // Start from body to make the initial focus deterministic.
  document.body.focus();
  for (let i = 0; i < ${count}; i++) {
    // A real Tab cannot be simulated with dispatchEvent because it does not
    // move focus. Use sequential focus navigation instead: find the next
    // tabbable element and call focus().
    const focusables = Array.from(document.querySelectorAll(
      'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )).filter(el => {
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
    });
    const cur = document.activeElement;
    const idx = focusables.indexOf(cur);
    const next = focusables[(idx + 1) % focusables.length];
    if (next) next.focus();
    const a = document.activeElement;
    const sel = a.dataset?.test ? '[data-test=' + JSON.stringify(a.dataset.test) + ']'
              : a.id ? '#' + a.id
              : a.tagName.toLowerCase() + (a.className ? '.' + String(a.className).split(/\s+/).join('.') : '');
    trail.push({ step: i, selector: sel });
  }
  return JSON.stringify(trail);
})()
EOF
}
```

> Note: `tauri-pilot press Tab` dispatches `keydown` inside the webview but does not trigger the browser's sequential focus navigation. This is a general Web platform limitation. The polyfill above uses `querySelectorAll` to simulate the same tab sequence. For the audit goal of verifying that critical controls can receive focus, the polyfill is equivalent to real Tab navigation.

#### 3.2.3 Focus Visibility Pixel Diff

```bash
# Add to scripts/audit-helpers.sh
# pilot_probe_focus_ring <selector> <out-dir>
#   Produces <out-dir>/focus-{blur,focus,diff}.png.
#   Prints the diff percentage as a numeric value to stdout.
pilot_probe_focus_ring() {
  local selector="$1" out="$2"
  mkdir -p "$out"
  tauri-pilot eval - <<EOF >/dev/null
const el = document.querySelector(${selector@Q});
if (!el) throw new Error('not found: ' + ${selector@Q});
el.blur(); 'blurred'
EOF
  tauri-pilot screenshot "$out/focus-blur.png" --selector "$selector"
  tauri-pilot eval - <<EOF >/dev/null
document.querySelector(${selector@Q}).focus(); 'focused'
EOF
  tauri-pilot screenshot "$out/focus-focus.png" --selector "$selector"
  # Use ImageMagick compare to calculate the changed-pixel percentage.
  # If ImageMagick is not available, fall back to pixelmatch through npx.
  if command -v compare >/dev/null 2>&1; then
    compare -metric AE "$out/focus-blur.png" "$out/focus-focus.png" "$out/focus-diff.png" 2>&1 \
      | awk -v total="$(identify -format '%[fx:w*h]' "$out/focus-blur.png")" '{ printf("%.4f\n", $1*100/total) }'
  else
    npx -y pixelmatch "$out/focus-blur.png" "$out/focus-focus.png" "$out/focus-diff.png" \
      | awk '/different pixels:/ { gsub(/[(%)]/,""); print $NF }'
  fi
}
```

> Dependency: `compare`/`identify` from ImageMagick or `npx pixelmatch`. If neither is available during the audit phase, fall back to manual visual inspection of the blur/focus screenshots and note the fallback in `REPORT.md`.

---

## 4. Architecture and Artifact Layers

### 4.1 Three Artifact Categories

```text
1. Audit artifacts (local workspace artifacts, not committed)
   ├── audit-runs/{scenario}-{UTC-ts}/
   │   ├── snapshot.json   (tauri-pilot snapshot -i --json)
   │   ├── screenshots/    (light/dark/reduced-motion)
   │   ├── logs.txt        (tauri-pilot logs --level error)
   │   └── network.json    (tauri-pilot network --failed)
   ├── audit-runs/REPORT.md     (summary: list + severity + root cause)
   └── audit-runs/p3-issues.md  (P3 issue drafts for gh CLI)

2. Fix artifacts (committed to main as the PR body)
   ├── apps/agent-gui/src/components/*.vue   (code fixes + new data-test anchors)
   ├── apps/agent-gui/src/styles/{theme,components}.css
   ├── apps/agent-gui/src/locales/{en,zh-CN}.json
   └── apps/agent-gui/src/**/*.test.ts       (Vitest regressions)

3. Verification / tooling artifacts (committed for regression prevention and reuse)
   ├── apps/agent-gui/e2e-pilot/audit-*.toml (5 scenarios)
   └── scripts/audit-helpers.sh              (helpers from §2.2)
```

### 4.2 Git Handling for `audit-runs/`

- **Location:** repository-root `audit-runs/`, at the same level as `pilot-results/` and `tauri-pilot-failures/`. Do not place it inside a worktree so it can persist across branches.
- **`.gitignore` change:** append this block to the repository-root `.gitignore`:

  ```gitignore
  # =============================================================================
  # Audit-driven UI/UX work (Spec 2026-05-09-gui-pilot-audit-design.md)
  # =============================================================================
  /audit-runs/
  ```

  This change is the second PR commit: `chore: ignore audit-runs/ artifacts directory`.

### 4.3 `REPORT.md` Commit Strategy

This resolves the potential contradiction between §1 and §9.

| Item                                                                                            | Commit to git | Where it appears                                                                            |
| ----------------------------------------------------------------------------------------------- | ------------- | ------------------------------------------------------------------------------------------- |
| Full `audit-runs/REPORT.md`, including screenshot references and long logs                      | ❌ No         | Local workspace + uploaded to the PR description inside a GitHub `<details>` fold-out block |
| **REPORT summary table** with issue ID / severity / description / commit hash / regression link | ✅ Yes        | Appended to §11 of this spec before PR closure, and copied to the top of the PR description |
| `audit-runs/p3-issues.md`                                                                       | ❌ No         | Local input list for `gh issue create`; the file can be deleted after issues are created    |

---

## 5. Fix Orchestration

Use a scenario-driven loop.

### 5.1 Commit Orchestration from Worktree Creation to PR Closure

```text
PR prelude before any scenario:
  C0. Create the worktree: just worktree feat/gui-pilot-audit-fixes
  C1. test(gui): add data-test anchors required by pilot audit scenarios
      Add every anchor from the "Anchors to add" column in §2.1 to the relevant .vue files,
      and add matching Vitest assertions.
  C2. chore: ignore audit-runs/ artifacts directory
      Add the .gitignore change from §4.2.
  C3. chore(gui): add scripts/audit-helpers.sh + offline axe-core
      Add all helpers from §2.2 and §3.2: pilot_fill_textarea,
      pilot_set_reduced_motion, pilot_measure_fps, pilot_run_axe,
      pilot_probe_tab_order, pilot_probe_focus_ring, and pilot_collect_evidence.
      Also run pnpm add -D axe-core and copy node_modules/axe-core/axe.min.js
      to apps/agent-gui/public/audit/axe.min.js.

For each S1..S5 scenario, in order:
  L1. test(gui): add audit-<scenario>.toml
      Or upgrade the existing TOML. Assertions must describe the expected correct behavior.
  L2. Run just test-pilot and expect the current scenario to fail red,
      exposing the difference between the current GUI and the expected behavior.
  L3. Run source scripts/audit-helpers.sh && pilot_collect_evidence <scenario>
      Use the evidence checklist in §3.1, including axe-core, FPS, and reduced-motion measurements.
  L4. Classify findings with the rubric using manual review + the ui-ux-pro-max skill,
      then write findings into audit-runs/REPORT.md.
  L5. Fix P0/P1 immediately using the §5.2 flow. Stage P2 findings in a
      "batch later" section at the end of REPORT.md.
  L6. After each P0/P1 fix, rerun the current scenario; commit after fmt + lint pass.

After all 5 scenarios have run:
  G1. Run a global root-cause scan and group P2 findings by shared root cause.
      Example: three components missing cursor-pointer → extract an .interactive class in components.css.
  G2. Batch-fix grouped P2 findings. Use style(gui) for the commit scope.
  G3. Rerun all 5 audit-*.toml scenarios to verify no regressions.
  G4. Convert P3 findings to GitHub issues:
      write audit-runs/p3-issues.md → create issues in batch with gh CLI.
      Every issue must include the `gui` label and then add labels by nature:
        - Lightweight interaction / visual improvement → `enhancement`
        - Script / audit tooling / dev tooling only → `tooling`
        - Introduces a new capability → `feature`
        - Documentation follow-up → `documentation`
      Do not apply `bug` to P3 findings; the rubric already classifies every bug-like
      item as P0/P1/P2. Do not create new labels. Only choose from existing project
      labels documented in AGENTS.md:
        gui, enhancement, tooling, feature, documentation, dependencies, ci.
      Do not use `bug`, `tui`, `runtime`, `core`, `models`, `tools`, `memory`, or `store`
      because they are unrelated to this spec or conflict with the P3 definition.
  G5. Append the REPORT summary table to §11 of this spec as an evolving spec artifact,
      and copy it to the top of the PR description.
  G6. Run final full verification, described in §7.
```

### 5.2 Inner Loop for Each P0/P1 Fix

This loop is mandatory and follows the `test-driven-development` skill.

Every independent issue fix must follow this sequence:

1. **Write the failing Vitest case first:** In the relevant `<Component>.test.ts`, add `it('reproduces audit issue: <issue-id>', ...)`, render with `mountWithPlugins`, and assert the expected behavior. Run `pnpm --filter agent-gui run test -- <Component>.test.ts` and expect red.
2. **Write the failing pilot assertion:** Add the corresponding `assert visible`, `assert text`, or `assert hidden` step to the scenario's `audit-*.toml`. Run `just test-pilot` for that scenario and expect red. This gives two layers of evidence that the issue is real.
3. **Fix the implementation:** Modify `.vue`, `.css`, or `.json` files until both layers are green.
4. **Commit:** Use `fix(gui): <one-sentence description>`. Reference the issue ID and the relevant `audit-runs` path in the commit body.

**Vitest conventions, aligned with `AGENTS.md`:**

- Test file path: same directory as the component, named `<Component>.test.ts`, for example `apps/agent-gui/src/components/ChatPanel.test.ts`.
- Use `mountWithPlugins` from `@/test-utils/mount` so the component gets Pinia, i18n, and router.
- Query selectors with `wrapper.find('[data-test="..."]')`, matching the pilot TOML selectors exactly for cross-reference.
- Name tests as `it('<P0|P1|P2>-<issue-id>: <human readable>')` so they are easy to grep later.

### 5.3 Commit Naming

- `test(gui): <description>` — C1 data-test anchors, L1 TOML additions/upgrades, and failing tests from §5.2
- `chore: <description>` — C2 `.gitignore`
- `chore(gui): <description>` — C3 `audit-helpers.sh`
- `fix(gui): <specific issue>` — §5.2 step 4 and all individual P0/P1/P2 fixes
- `style(gui): unify <token> across <components>` — G2 grouped fixes
- `docs(gui): append audit report summary to spec` — G5

---

## 6. Tool and Skill Orchestration

| Phase              | Tool / Skill                                                             | Purpose                                                                   |
| ------------------ | ------------------------------------------------------------------------ | ------------------------------------------------------------------------- |
| Run GUI            | `just test-pilot`                                                        | Start the debug build with the pilot feature                              |
| Real-time audit    | `tauri-pilot snapshot/screenshot/logs/network/eval/assert`               | Collect evidence                                                          |
| Design judgment    | `ui-ux-pro-max` skill with `--design-system` + `--domain ux/style/color` | Decide whether the UI follows UX best practices and propose fix direction |
| Fix implementation | `test-driven-development` skill                                          | Write failing Vitest cases before each P0/P1 fix                          |
| Complex debugging  | `systematic-debugging` skill                                             | Use when the root cause is unclear                                        |
| Final acceptance   | `verification-before-completion` skill                                   | Require evidence-driven verification before closure                       |
| Wrap-up            | `finishing-a-development-branch` skill                                   | Organize the PR description and integration path                          |

---

## 7. Testing and Verification Strategy

### 7.1 Required Closure Gates

All gates are required.

| Command                                | Expected result                      | Notes                                                                     |
| -------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------- |
| `pnpm format:check`                    | 0 errors                             | Existing CI gate                                                          |
| `pnpm lint`                            | 0 warnings                           | clippy + oxlint + stylelint                                               |
| `cargo test --workspace --all-targets` | All green                            | Rust tests only; type sync is verified separately with `just check-types` |
| `just test-gui`                        | All green + new Vitest regressions   | At least one regression test per P0/P1                                    |
| `just test-pilot`                      | All 5 `audit-*.toml` scenarios green | Critical final regression wall                                            |
| `just check-types`                     | Clean                                | Prevents unsynchronized Rust type changes                                 |

### 7.2 Two-Layer Regression Structure

- **Vitest unit layer:** Component-level regression. Example: `ChatPanel.test.ts` adds "shows error toast when send fails".
- **tauri-pilot TOML E2E layer:** Actual GUI behavior. Example: `audit-chat.toml` adds `assert visible [data-test="error-toast"]`.

Both layers are required. Unit tests cover logic, while pilot covers real rendering; CSS jank and contrast issues can only be found in the pilot layer.

### 7.3 Cross-Platform Verification

- **Local:** macOS can run `just test-pilot` directly.
- **CI:** The existing `tauri-pilot-e2e` job already covers `ubuntu-latest` with `xvfb-run` and `macos-latest`. New scenarios are included naturally.
- **Windows pilot:** Not required because the current CI matrix does not include it.

---

## 8. Risks, Assumptions, and Out of Scope

### 8.1 Risks and Mitigations

| Risk                                                                                    | Impact                                            | Mitigation                                                                                                                                           |
| --------------------------------------------------------------------------------------- | ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| **R1** `tauri-pilot` textarea bug                                                       | The chat path cannot use standard `fill` / `type` | Use the `pilot_fill_textarea` helper from §2.2                                                                                                       |
| **R2** More than 50 issues are discovered at once, making a single PR hard to review    | PR stalls or reviewers reject the size            | The user has authorized no fixed upper limit, but reassess progress after every 10 fix commits and keep the fallback option of splitting by scenario |
| **R3** Local `ui-ux-pro-max` Python script is not installed                             | Cannot compare against the design system          | Run `python3 --version`; if missing, install according to the brew command in `SKILL.md`                                                             |
| **R4** First `just test-pilot` debug build is slow, around 5 minutes                    | Initial feedback is slow                          | Keep `target/` after the first build in the worktree so later incremental builds are much faster                                                     |
| **R5** Fixing dark mode breaks light mode, or vice versa                                | One theme regresses while fixing the other        | Every scenario evidence checklist requires both light and dark screenshots                                                                           |
| **R6** A fix unintentionally modifies specta types                                      | type-sync CI fails                                | Run `just check-types` before submission                                                                                                             |
| **R7** Grouped root-cause fixes introduce broad CSS refactors and secondary visual bugs | Visual regression                                 | Rerun all 5 pilot scenarios after grouped fixes                                                                                                      |

### 8.2 Assumptions

- A1: It is acceptable for Spec A to span multiple weeks; the user selected the no-fixed-limit option.
- A2: `tauri-pilot` v0.5.0 is not upgraded in this spec.
- A3: This spec does not modify Rust crates. If a GUI issue is rooted in Rust, such as a missing event payload field, create an issue.
- A4: This spec does not add product features. It only fixes defects and usability issues.

### 8.3 Explicitly Out of Scope

- ❌ Redesigning the design system; no Tailwind, shadcn, or other UI library migration
- ❌ Rewriting component structure
- ❌ Changing the i18n framework; only add missing keys
- ❌ Changing the vue-router structure
- ❌ Fixing upstream `tauri-pilot` bugs
- ❌ Doing Spec B; that is a separate spec

---

## 9. Definition of Done

This spec is complete if and only if:

1. ✅ All 5 `audit-*.toml` scenarios exist and `just test-pilot` is fully green.
2. ✅ `audit-runs/REPORT.md` lists every finding, and each P0/P1/P2 has a corresponding commit hash.
3. ✅ `audit-runs/p3-issues.md` has been used to batch-create GitHub issues with `gh issue create`; issue URLs are written back to `REPORT.md`, and labels are limited to the reusable labels listed in §5.1.G4.
4. ✅ Every hard verification gate in §7.1 passes.
5. ✅ The `feat/gui-pilot-audit-fixes` branch has been pushed and a PR has been created; the REPORT summary table has been appended to §11 of this spec and copied to the top of the PR description.
6. ✅ The user approves the PR.

---

## 10. Follow-Up Work

- **Spec B**, as a separate spec, should extract this spec's methodology into:
  - `docs/superpowers/skills/gui-pilot-audit/SKILL.md` as a reusable skill
  - An updated "Tauri pilot E2E" section in `AGENTS.md`, including audit TOML authoring, helper scripts, and the rubric
  - An evaluation of upgrading `tauri-pilot` to a version that fixes the textarea bug

---

## 11. Execution Results

This section is intentionally a placeholder for the implementation phase.

> This section will be filled by the §5.1.G5 commit `docs(gui): append audit report summary to spec` before PR closure.
> At the spec stage, this is a placeholder heading and **is not considered a lingering TODO** because the implementation phase has not produced data yet.
> Table columns: `Issue ID | Severity | One-sentence description | Fix commit | Regression test (Vitest file + TOML step)`
