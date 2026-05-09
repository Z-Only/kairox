# GUI Pilot Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run a full tauri-pilot-driven GUI UI/UX audit, fix every P0/P1/P2 finding, and turn P3 improvements into GitHub issues.

**Architecture:** Add stable pilot selectors and manual audit helpers first, then drive five scenario-specific audit loops through `tauri-pilot`, Vitest regressions, and `audit-*.toml` CI assertions. Keep local evidence under ignored `audit-runs/`, commit only regression scenarios, helper tooling, source fixes, tests, and the final report summary.

**Tech Stack:** Tauri 2, Vue 3 Composition API, TypeScript, Pinia, Vitest, Playwright-style TOML scenarios via `tauri-pilot`, shell helper scripts, offline `axe-core`, CSS custom properties.

**Spec:** `docs/superpowers/specs/2026-05-09-gui-pilot-audit-design.md`

---

## File Structure

### New files

| File                                                     | Responsibility                                                                                                                                                            |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scripts/audit-helpers.sh`                               | Manual pilot audit helper library: textarea workaround, reduced motion toggle, FPS measurement, offline axe scan, tab order probe, focus-ring probe, evidence collection. |
| `apps/agent-gui/public/audit/.gitkeep`                   | Keep the public audit asset directory in git.                                                                                                                             |
| `apps/agent-gui/public/audit/axe.min.js`                 | Offline axe-core browser bundle loaded by `pilot_run_axe`.                                                                                                                |
| `apps/agent-gui/e2e-pilot/audit-bootstrap.toml`          | Stable CI regression scenario for app shell, navigation, Settings theme control, and Workbench root.                                                                      |
| `apps/agent-gui/e2e-pilot/audit-sessions.toml`           | Stable CI regression scenario for session create, rename, delete, and confirmation dialog behavior.                                                                       |
| `apps/agent-gui/e2e-pilot/audit-chat.toml`               | Stable CI regression scenario for chat input, message rendering, streaming/cancel/error anchors, trace/task/permission anchors.                                           |
| `apps/agent-gui/e2e-pilot/audit-mcp.toml`                | Stable CI regression scenario for MCP manager empty/list/status/trust controls.                                                                                           |
| `apps/agent-gui/e2e-pilot/audit-marketplace-memory.toml` | Stable CI regression scenario for Marketplace and MemoryBrowser anchors.                                                                                                  |
| `audit-runs/REPORT.md`                                   | Local, ignored audit report with findings, severity, root cause, evidence, fix commit, and regression link.                                                               |
| `audit-runs/p3-issues.md`                                | Local, ignored issue draft list for P3 GitHub issue creation.                                                                                                             |

### Modified files

| File                                                                               | Changes                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `.gitignore`                                                                       | Ignore root `audit-runs/` artifacts.                                                                                                                                                             |
| `apps/agent-gui/package.json`                                                      | Add `axe-core` as an `agent-gui` dev dependency.                                                                                                                                                 |
| `pnpm-lock.yaml`                                                                   | Lock the `axe-core` dev dependency.                                                                                                                                                              |
| `apps/agent-gui/src/layouts/AppLayout.vue`                                         | Ensure `app-shell`, `app-nav`, `nav-workbench`, and `nav-settings` anchors remain stable.                                                                                                        |
| `apps/agent-gui/src/views/WorkbenchView.vue`                                       | Verify or add `data-test="view-workbench"`.                                                                                                                                                      |
| `apps/agent-gui/src/views/MarketplaceView.vue`                                     | Add `data-test="view-marketplace"` to the legacy view root, even though the current route redirects `/marketplace` to Settings.                                                                  |
| `apps/agent-gui/src/views/SettingsView.vue`                                        | Verify `view-settings`; add `theme-toggle` to the theme select/toggle control while preserving `settings-theme`; add `settings-tab-marketplace` to the Marketplace tab button for S5 navigation. |
| `apps/agent-gui/src/components/SessionsSidebar.vue`                                | Add `new-session-dialog`, `session-rename-btn`, `session-rename-input`, `session-rename-confirm`; keep existing session anchors.                                                                 |
| `apps/agent-gui/src/components/ChatPanel.vue`                                      | Add `chat-message`, `data-role`, `chat-empty-state`, `stream-indicator`, and `error-banner` anchors without changing logic.                                                                      |
| `apps/agent-gui/src/components/TraceTimeline.vue`                                  | Add `trace-timeline`, `trace-tab-memory`, and per-entry `trace-entry` anchors.                                                                                                                   |
| `apps/agent-gui/src/components/TaskSteps.vue`                                      | Add `task-steps`.                                                                                                                                                                                |
| `apps/agent-gui/src/components/TaskNode.vue`                                       | Add `task-node` and `task-node-status`.                                                                                                                                                          |
| `apps/agent-gui/src/components/PermissionPrompt.vue`                               | Add `permission-prompt`, `permission-allow`, `permission-deny`; keep `trust-server-checkbox`.                                                                                                    |
| `apps/agent-gui/src/components/McpServerManager.vue`                               | Add `mcp-manager`, empty/list/status/error/start/stop/trust/revoke/close anchors.                                                                                                                |
| `apps/agent-gui/src/components/McpStatusIndicator.vue`                             | Add `mcp-status-indicator`.                                                                                                                                                                      |
| `apps/agent-gui/src/components/MemoryBrowser.vue`                                  | Add `memory-browser`, `memory-list`, `memory-item`, `memory-empty-state`, `memory-refresh-btn`, `memory-delete-btn`.                                                                             |
| `apps/agent-gui/src/components/*.test.ts` and `apps/agent-gui/src/views/*.test.ts` | Add selector-preservation tests and P0/P1/P2 regression tests as issues are discovered.                                                                                                          |
| `docs/superpowers/specs/2026-05-09-gui-pilot-audit-design.md`                      | Append final report summary table in section 11 before PR closure.                                                                                                                               |

### Files that must not be modified

| File                                       | Reason                                                                                               |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `scripts/run-pilot-tests.sh`               | The spec explicitly keeps this as the CI entry point and forbids modification.                       |
| `apps/agent-gui/src/generated/commands.ts` | Generated by `just gen-types`; do not edit manually.                                                 |
| `apps/agent-gui/src/generated/events.ts`   | Generated by `just gen-types`; do not edit manually.                                                 |
| Rust crates under `crates/`                | Rust-side behavior is out of scope for this spec; create an issue if a GUI defect is rooted in Rust. |

---

## Task 1: Create the isolated audit worktree

**Files:**

- No source files changed in this task.

- [ ] **Step 1: Create the worktree**

Run from the repository root:

```bash
just worktree feat/gui-pilot-audit-fixes
```

Expected: a new worktree exists at `.worktrees/feat-gui-pilot-audit-fixes`, the branch is `feat/gui-pilot-audit-fixes`, and `pnpm install` has completed.

- [ ] **Step 2: Enter the worktree and verify branch**

Run:

```bash
cd .worktrees/feat-gui-pilot-audit-fixes
git branch --show-current | cat
```

Expected output:

```text
feat/gui-pilot-audit-fixes
```

- [ ] **Step 3: Verify pilot prerequisite**

Run:

```bash
command -v tauri-pilot && tauri-pilot --help | cat
```

Expected: prints the `tauri-pilot` executable path and help text. If missing, install it:

```bash
cargo install --git https://github.com/mpiton/tauri-pilot tauri-pilot-cli
```

---

## Task 2: Add selector-preservation tests for required audit anchors

**Files:**

- Modify: `apps/agent-gui/src/views/WorkbenchView.test.ts`
- Modify: `apps/agent-gui/src/views/SettingsView.test.ts`
- Modify: `apps/agent-gui/src/components/SessionsSidebar.test.ts`
- Modify: `apps/agent-gui/src/components/ChatPanel.test.ts`
- Modify: `apps/agent-gui/src/components/TraceTimeline.test.ts`
- Modify: `apps/agent-gui/src/components/TaskSteps.test.ts`
- Modify: `apps/agent-gui/src/components/TaskNode.test.ts`
- Modify: `apps/agent-gui/src/components/PermissionPrompt.test.ts`
- Modify: `apps/agent-gui/src/components/McpServerManager.test.ts`
- Modify: `apps/agent-gui/src/components/McpStatusIndicator.test.ts`
- Modify: `apps/agent-gui/src/components/MemoryBrowser.test.ts`
- Modify: `apps/agent-gui/src/components/marketplace/Marketplace.test.ts`

- [ ] **Step 1: Add failing tests for required missing anchors**

Add one selector contract test per affected component. Use the existing test setup in each file; query with exact selectors such as `wrapper.find('[data-test="chat-message"]')` and assert `.exists()`.

Add concrete selector checks to the existing component tests instead of creating a shared abstract helper. Use the component-specific mount helpers that already exist in each file. For example, add this concrete test to `apps/agent-gui/src/components/ChatPanel.test.ts`:

```typescript
it("audit anchors: exposes stable chat pilot selectors", async () => {
  const wrapper = mountChatPanel((session) => {
    session.projection.messages = [
      { role: "user", content: "Hello" },
      { role: "assistant", content: "Hi" }
    ];
    session.projection.token_stream = "Streaming";
    session.lastSendError = "network failed";
    session.isStreaming = true;
  });
  await flushPromises();

  expect(wrapper.find('[data-test="chat-message"][data-role="user"]').exists()).toBe(true);
  expect(wrapper.find('[data-test="chat-message"][data-role="assistant"]').exists()).toBe(true);
  expect(wrapper.find('[data-test="stream-indicator"]').exists()).toBe(true);
  expect(wrapper.find('[data-test="error-banner"]').exists()).toBe(true);
});
```

Use these exact expected selectors in the relevant component-specific tests:

```text
view-workbench
view-marketplace
view-settings
theme-toggle
settings-tab-marketplace
new-session-dialog
session-rename-btn
session-rename-input
session-rename-confirm
chat-message
chat-empty-state
stream-indicator
error-banner
trace-timeline
trace-tab-memory
trace-entry
task-steps
task-node
task-node-status
permission-prompt
permission-allow
permission-deny
mcp-manager
mcp-empty-state
mcp-server-item
mcp-server-name
mcp-server-status
mcp-server-error
mcp-start-btn
mcp-stop-btn
mcp-trust-btn
mcp-revoke-btn
mcp-close-btn
mcp-status-indicator
memory-browser
memory-list
memory-item
memory-empty-state
memory-refresh-btn
memory-delete-btn
```

For `ChatPanel.vue`, also assert user and assistant messages expose role metadata:

```typescript
expect(wrapper.find('[data-test="chat-message"][data-role="user"]').exists()).toBe(true);
expect(wrapper.find('[data-test="chat-message"][data-role="assistant"]').exists()).toBe(true);
```

- [ ] **Step 2: Run tests and verify they fail before implementation**

Run:

```bash
pnpm --filter agent-gui run test -- WorkbenchView.test.ts SettingsView.test.ts SessionsSidebar.test.ts ChatPanel.test.ts TraceTimeline.test.ts TaskSteps.test.ts TaskNode.test.ts PermissionPrompt.test.ts McpServerManager.test.ts McpStatusIndicator.test.ts MemoryBrowser.test.ts Marketplace.test.ts
```

Expected: fails only on missing `data-test` selectors. If a test fails because the component mount setup is invalid, fix the test harness first without changing component behavior.

---

## Task 3: Add required `data-test` anchors without behavior changes

**Files:**

- Modify: all component/view files listed in Task 2.

- [ ] **Step 1: Add the route view anchors**

Add or verify these root anchors:

```vue
<main class="workbench" data-test="view-workbench">
```

```vue
<section class="marketplace" data-test="view-marketplace">
```

```vue
<section class="settings" data-test="view-settings">
```

In `SettingsView.vue`, keep `data-test="settings-theme"` on the existing theme `<select>` and wrap it in a stable audit container:

```vue
<div data-test="theme-toggle">
  <select id="settings-theme" data-test="settings-theme">
  </select>
</div>
```

Add `data-test="settings-tab-marketplace"` to the Marketplace tab button:

```vue
<button data-test="settings-tab-marketplace" role="tab">
  {{ t("nav.marketplace") }}
</button>
```

- [ ] **Step 2: Add SessionsSidebar anchors**

Add these selectors to the existing dialog and rename controls:

```vue
<dialog data-test="new-session-dialog">
<input data-test="session-rename-input" />
<button data-test="session-rename-btn">Rename</button>
<button data-test="session-rename-confirm">Save</button>
```

Do not change create/delete/confirm behavior. Preserve existing `new-session-btn`, `create-session-btn`, `session-item`, `session-delete-btn`, `sessions-empty`, `confirm-cancel`, and `confirm-ok` selectors.

- [ ] **Step 3: Add ChatPanel anchors**

For message rows, use the message role as `data-role`:

```vue
<div
  v-for="message in messages"
  :key="message.id"
  class="message"
  data-test="chat-message"
  :data-role="message.role"
>
```

Add the empty, streaming, and error anchors to the existing UI states:

```vue
<div v-if="messages.length === 0" class="empty-state" data-test="chat-empty-state">
```

```vue
<span v-if="isStreaming" class="tag" data-test="stream-indicator">Streaming</span>
```

```vue
<div v-if="error" class="alert alert-error" data-test="error-banner">
```

- [ ] **Step 4: Add trace/task/permission anchors**

Add these stable selectors to the existing elements. In `TraceTimeline.vue`, put `trace-tab-memory` on the Memory tab button because S5 opens `MemoryBrowser` through this tab.

```vue
<section class="trace-timeline" data-test="trace-timeline">
<button data-test="trace-tab-memory">
<article class="trace-entry" data-test="trace-entry">
<section class="task-steps" data-test="task-steps">
<article class="task-node" data-test="task-node">
<span class="tag" data-test="task-node-status">
<section class="permission-prompt" data-test="permission-prompt">
<button data-test="permission-allow">
<button data-test="permission-deny">
```

- [ ] **Step 5: Add MCP and memory anchors**

Add these selectors to the existing MCP manager/status UI:

```vue
<section data-test="mcp-manager">
<div data-test="mcp-empty-state">
<article data-test="mcp-server-item">
<span data-test="mcp-server-name">
<span data-test="mcp-server-status">
<p data-test="mcp-server-error">
<button data-test="mcp-start-btn">
<button data-test="mcp-stop-btn">
<button data-test="mcp-trust-btn">
<button data-test="mcp-revoke-btn">
<button data-test="mcp-close-btn">
<span data-test="mcp-status-indicator">
```

Add these selectors to the existing memory UI:

```vue
<section data-test="memory-browser">
<ul data-test="memory-list">
<li data-test="memory-item">
<div data-test="memory-empty-state">
<button data-test="memory-refresh-btn">
<button data-test="memory-delete-btn">
```

- [ ] **Step 6: Verify selector tests pass**

Run the same command from Task 2:

```bash
pnpm --filter agent-gui run test -- WorkbenchView.test.ts SettingsView.test.ts SessionsSidebar.test.ts ChatPanel.test.ts TraceTimeline.test.ts TaskSteps.test.ts TaskNode.test.ts PermissionPrompt.test.ts McpServerManager.test.ts McpStatusIndicator.test.ts MemoryBrowser.test.ts Marketplace.test.ts
```

Expected: all selector-preservation tests pass.

- [ ] **Step 7: Commit selector anchors**

Run:

```bash
git add apps/agent-gui/src apps/agent-gui/src/**/*.test.ts
git commit -m "test(gui): add data-test anchors required by pilot audit scenarios"
```

---

## Task 4: Ignore local audit artifacts

**Files:**

- Modify: `.gitignore`

- [ ] **Step 1: Append the audit-runs ignore block**

Append exactly this block after the existing tauri-pilot artifact ignores:

```gitignore
# =============================================================================
# Audit-driven UI/UX work (Spec 2026-05-09-gui-pilot-audit-design.md)
# =============================================================================
/audit-runs/
```

- [ ] **Step 2: Verify ignore behavior**

Run:

```bash
mkdir -p audit-runs && touch audit-runs/.probe && git check-ignore -v audit-runs/.probe | cat
```

Expected: output references `.gitignore` and `/audit-runs/`.

- [ ] **Step 3: Clean the probe and commit**

Run:

```bash
rm audit-runs/.probe
git add .gitignore
git commit -m "chore: ignore audit-runs artifacts directory"
```

---

## Task 5: Add audit helper library and offline axe-core

**Files:**

- Create: `scripts/audit-helpers.sh`
- Create: `apps/agent-gui/public/audit/.gitkeep`
- Create: `apps/agent-gui/public/audit/axe.min.js`
- Modify: `apps/agent-gui/package.json`
- Modify: `pnpm-lock.yaml`

- [ ] **Step 1: Add `axe-core` dev dependency**

Run:

```bash
pnpm --filter agent-gui add -D axe-core@^4
```

Expected: `apps/agent-gui/package.json` and `pnpm-lock.yaml` are updated.

- [ ] **Step 2: Copy offline axe bundle into public assets**

Run:

```bash
mkdir -p apps/agent-gui/public/audit
cp node_modules/axe-core/axe.min.js apps/agent-gui/public/audit/axe.min.js
touch apps/agent-gui/public/audit/.gitkeep
```

Expected: `apps/agent-gui/public/audit/axe.min.js` exists and is not ignored.

- [ ] **Step 3: Create the helper script**

Create `scripts/audit-helpers.sh` with these exported functions:

```bash
#!/usr/bin/env bash
set -euo pipefail

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

pilot_run_axe() {
  tauri-pilot eval - <<'EOF'
(async () => {
  if (!window.axe) {
    const code = await (await fetch('/audit/axe.min.js')).text();
    new Function(code)();
  }
  const r = await window.axe.run({ resultTypes: ['violations'] });
  return JSON.stringify({
    violations: r.violations.map(v => ({
      id: v.id,
      impact: v.impact,
      help: v.help,
      nodes: v.nodes.map(n => ({ target: n.target, html: n.html.slice(0, 200) }))
    }))
  });
})()
EOF
}

pilot_probe_tab_order() {
  local count="${1:-30}"
  tauri-pilot eval - <<EOF
(async () => {
  const trail = [];
  document.body.focus();
  for (let i = 0; i < ${count}; i++) {
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
  if command -v compare >/dev/null 2>&1; then
    compare -metric AE "$out/focus-blur.png" "$out/focus-focus.png" "$out/focus-diff.png" 2>&1 \
      | awk -v total="$(identify -format '%[fx:w*h]' "$out/focus-blur.png")" '{ printf("%.4f\n", $1*100/total) }'
  else
    npx -y pixelmatch "$out/focus-blur.png" "$out/focus-focus.png" "$out/focus-diff.png" \
      | awk '/different pixels:/ { gsub(/[(%)]/,""); print $NF }'
  fi
}

pilot_collect_evidence() {
  local scenario="$1"
  local ts dir prev_theme_raw prev_theme
  ts="$(date -u +%Y%m%dT%H%M%SZ)"
  dir="audit-runs/${scenario}-${ts}"
  mkdir -p "${dir}/screenshots"
  tauri-pilot snapshot -i --json > "${dir}/snapshot.json"
  tauri-pilot logs --level error > "${dir}/logs.txt" || true
  tauri-pilot network --failed > "${dir}/network.json" || true

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
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  tauri-pilot screenshot "${dir}/screenshots/light.png"

  tauri-pilot eval - <<'EOF' >/dev/null
localStorage.setItem('kairox.color-mode', 'dark');
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: 'dark' }));
'dark'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  tauri-pilot screenshot "${dir}/screenshots/dark.png"

  tauri-pilot eval - <<EOF >/dev/null
localStorage.setItem('kairox.color-mode', ${prev_theme@Q});
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: ${prev_theme@Q} }));
'restored'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  pilot_set_reduced_motion on
  tauri-pilot screenshot "${dir}/screenshots/reduced-motion.png"
  pilot_set_reduced_motion off

  pilot_run_axe > "${dir}/axe.json" || true
  echo "${dir}"
}
```

- [ ] **Step 4: Make the script executable and syntax-check it**

Run:

```bash
chmod +x scripts/audit-helpers.sh
bash -n scripts/audit-helpers.sh
```

Expected: `bash -n` exits 0.

- [ ] **Step 5: Commit helper tooling**

Run:

```bash
git add scripts/audit-helpers.sh apps/agent-gui/public/audit/.gitkeep apps/agent-gui/public/audit/axe.min.js apps/agent-gui/package.json pnpm-lock.yaml
git commit -m "chore(gui): add pilot audit helpers and offline axe-core"
```

---

## Task 6: Add `audit-bootstrap.toml` and run S1 audit loop

**Files:**

- Create: `apps/agent-gui/e2e-pilot/audit-bootstrap.toml`
- Modify as findings require: `apps/agent-gui/src/layouts/AppLayout.vue`, `apps/agent-gui/src/views/WorkbenchView.vue`, `apps/agent-gui/src/views/SettingsView.vue`, related tests.

- [ ] **Step 1: Create the bootstrap scenario**

Create `apps/agent-gui/e2e-pilot/audit-bootstrap.toml`:

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Audit bootstrap"
fail_fast = true
global_timeout_ms = 45000

[[step]]
name = "wait for app shell"
action = "wait"
selector = "[data-test='app-shell']"
timeout_ms = 15000

[[step]]
name = "app nav visible"
action = "assert-visible"
target = "[data-test='app-nav']"

[[step]]
name = "workbench nav visible"
action = "assert-visible"
target = "[data-test='nav-workbench']"

[[step]]
name = "settings nav visible"
action = "assert-visible"
target = "[data-test='nav-settings']"

[[step]]
name = "sessions sidebar visible"
action = "assert-visible"
target = "[data-test='sessions-sidebar']"

[[step]]
name = "status bar visible"
action = "assert-visible"
target = "[data-test='status-bar']"

[[step]]
name = "workbench view visible"
action = "assert-visible"
target = "[data-test='view-workbench']"

[[step]]
name = "open settings"
action = "click"
target = "[data-test='nav-settings']"

[[step]]
name = "settings view visible"
action = "assert-visible"
target = "[data-test='view-settings']"

[[step]]
name = "theme toggle visible"
action = "assert-visible"
target = "[data-test='theme-toggle']"
```

- [ ] **Step 2: Run the scenario red/green check**

Run:

```bash
just test-pilot
```

Expected before fixes: any failure identifies missing anchors or UI defects. Expected after fixes: `audit-bootstrap` passes.

- [ ] **Step 3: Collect S1 evidence**

With the pilot app running, run:

```bash
source scripts/audit-helpers.sh
pilot_collect_evidence S1-bootstrap
pilot_probe_tab_order 30 > audit-runs/S1-tab-order.json
pilot_probe_focus_ring "[data-test='theme-toggle']" audit-runs/S1-focus-theme-toggle
```

Expected: evidence directory contains `snapshot.json`, `logs.txt`, `network.json`, `axe.json`, and light/dark/reduced-motion screenshots.

- [ ] **Step 4: Classify and fix S1 findings**

For every P0/P1/P2 finding:

1. Add a failing Vitest case named with a concrete issue ID, for example `P0-S1-keyboard-nav`, `P1-S1-theme-feedback`, or `P2-S1-focus-ring`.
2. Add or update the `audit-bootstrap.toml` assertion that captures the expected behavior.
3. Fix the Vue/CSS/i18n code.
4. Rerun the targeted Vitest file and `just test-pilot`.

- [ ] **Step 5: Commit S1 scenario and fixes**

Run:

```bash
git add apps/agent-gui/e2e-pilot/audit-bootstrap.toml apps/agent-gui/src docs/superpowers/specs/2026-05-09-gui-pilot-audit-design.md
git commit -m "test(gui): add bootstrap pilot audit scenario"
```

Use separate commits for any S1 P0/P1/P2 implementation fixes, with messages like `fix(gui): restore visible keyboard focus in settings` or `fix(gui): add missing settings loading feedback`.

---

## Task 7: Add `audit-sessions.toml` and run S2 audit loop

**Files:**

- Create: `apps/agent-gui/e2e-pilot/audit-sessions.toml`
- Modify as findings require: `apps/agent-gui/src/components/SessionsSidebar.vue`, `apps/agent-gui/src/components/ConfirmDialog.vue`, related tests.

- [ ] **Step 1: Create the sessions scenario**

Create `apps/agent-gui/e2e-pilot/audit-sessions.toml`:

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Audit sessions lifecycle"
fail_fast = true
global_timeout_ms = 60000

[[step]]
name = "wait for sidebar"
action = "wait"
selector = "[data-test='sessions-sidebar']"
timeout_ms = 15000

[[step]]
name = "new session button visible"
action = "assert-visible"
target = "[data-test='new-session-btn']"

[[step]]
name = "open new session dialog"
action = "click"
target = "[data-test='new-session-btn']"

[[step]]
name = "new session dialog visible"
action = "assert-visible"
target = "[data-test='new-session-dialog']"

[[step]]
name = "create session button visible"
action = "assert-visible"
target = "[data-test='create-session-btn']"

[[step]]
name = "create a session"
action = "click"
target = "[data-test='create-session-btn']"

[[step]]
name = "session item visible"
action = "assert-visible"
target = "[data-test='session-item']"

[[step]]
name = "rename button visible"
action = "assert-visible"
target = "[data-test='session-rename-btn']"

[[step]]
name = "delete button visible"
action = "assert-visible"
target = "[data-test='session-delete-btn']"

[[step]]
name = "open delete confirmation"
action = "click"
target = "[data-test='session-delete-btn']"

[[step]]
name = "confirm cancel visible"
action = "assert-visible"
target = "[data-test='confirm-cancel']"

[[step]]
name = "confirm ok visible"
action = "assert-visible"
target = "[data-test='confirm-ok']"
```

- [ ] **Step 2: Run, collect, classify, fix**

Run:

```bash
just test-pilot
source scripts/audit-helpers.sh
pilot_collect_evidence S2-sessions
pilot_probe_tab_order 40 > audit-runs/S2-tab-order.json
```

Expected: all S2 P0/P1/P2 findings are captured in `audit-runs/REPORT.md`, fixed with Vitest and TOML regressions, and committed.

- [ ] **Step 3: Commit S2 scenario**

Run:

```bash
git add apps/agent-gui/e2e-pilot/audit-sessions.toml apps/agent-gui/src/components/SessionsSidebar.vue apps/agent-gui/src/components/SessionsSidebar.test.ts
git commit -m "test(gui): add sessions pilot audit scenario"
```

Use separate commits for S2 P0/P1/P2 implementation fixes, with messages like `fix(gui): make session rename reachable by keyboard` or `fix(gui): show destructive confirmation focus state`.

---

## Task 8: Add `audit-chat.toml` and run S3 audit loop

**Files:**

- Create: `apps/agent-gui/e2e-pilot/audit-chat.toml`
- Modify as findings require: `ChatPanel.vue`, `TraceTimeline.vue`, `TaskSteps.vue`, `TaskNode.vue`, `PermissionPrompt.vue`, related tests.

- [ ] **Step 1: Create the chat scenario**

Create `apps/agent-gui/e2e-pilot/audit-chat.toml`:

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Audit chat streaming"
fail_fast = true
global_timeout_ms = 90000

[[step]]
name = "wait for chat input"
action = "wait"
selector = "textarea[data-test='message-input']"
timeout_ms = 15000

[[step]]
name = "empty state visible"
action = "assert-visible"
target = "[data-test='chat-empty-state']"

[[step]]
name = "send button visible"
action = "assert-visible"
target = "[data-test='send-button']"

[[step]]
name = "cancel button target exists"
action = "assert-visible"
target = "[data-test='cancel-button']"

[[step]]
name = "trace timeline visible"
action = "assert-visible"
target = "[data-test='trace-timeline']"

[[step]]
name = "task steps visible"
action = "assert-visible"
target = "[data-test='task-steps']"

[[step]]
name = "message list visible"
action = "assert-visible"
target = "[data-test='message-list']"
```

Do not use `fill` or `type` on the textarea in this TOML file until upstream `tauri-pilot` fixes the textarea setter bug. Use `pilot_fill_textarea` manually during exploration.

- [ ] **Step 2: Manually drive the textarea workaround during audit**

With the app running:

```bash
source scripts/audit-helpers.sh
pilot_fill_textarea "textarea[data-test='message-input']" "Audit chat streaming responsiveness"
tauri-pilot click "[data-test='send-button']"
tauri-pilot wait --selector "[data-test='chat-message']"
pilot_measure_fps 5000 > audit-runs/S3-streaming-fps.txt
pilot_collect_evidence S3-chat
```

Expected: the user message renders; any streaming, error, trace, task, permission, reduced-motion, FPS, or accessibility defects are recorded.

- [ ] **Step 3: Fix S3 P0/P1/P2 issues with two-layer regressions**

For the known chat error-feedback regression, add this concrete test to `apps/agent-gui/src/components/ChatPanel.test.ts`:

```typescript
it("P1-S3-send-error: shows a visible send error banner", async () => {
  const wrapper = mountChatPanel((session) => {
    session.lastSendError = "model unavailable";
  });
  await flushPromises();

  const errorBanner = wrapper.find('[data-test="error-banner"]');
  expect(errorBanner.exists()).toBe(true);
  expect(errorBanner.text()).toContain("model unavailable");
});
```

For S3 issues that depend on sending text through the textarea, keep the Vitest regression above and record the manual pilot command/evidence path in `audit-runs/REPORT.md` because `tauri-pilot` v0.5.0 cannot `fill` or `type` this `<textarea>` in TOML.

- [ ] **Step 4: Commit S3 scenario**

Run:

```bash
git add apps/agent-gui/e2e-pilot/audit-chat.toml apps/agent-gui/src/components/ChatPanel.vue apps/agent-gui/src/components/ChatPanel.test.ts
git commit -m "test(gui): add chat pilot audit scenario"
```

Use separate commits for S3 P0/P1/P2 implementation fixes, with messages like `fix(gui): show chat send error banner` or `fix(gui): respect reduced motion during streaming`.

---

## Task 9: Add `audit-mcp.toml` and run S4 audit loop

**Files:**

- Create: `apps/agent-gui/e2e-pilot/audit-mcp.toml`
- Modify as findings require: `apps/agent-gui/src/components/McpServerManager.vue`, `apps/agent-gui/src/components/McpStatusIndicator.vue`, `apps/agent-gui/src/stores/mcp.ts`, related tests.

- [ ] **Step 1: Create the MCP scenario**

Create `apps/agent-gui/e2e-pilot/audit-mcp.toml`:

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Audit MCP manager"
fail_fast = true
global_timeout_ms = 60000

[[step]]
name = "wait for app shell"
action = "wait"
selector = "[data-test='app-shell']"
timeout_ms = 15000

[[step]]
name = "mcp status indicator exists"
action = "assert-visible"
target = "[data-test='mcp-status-indicator']"

[[step]]
name = "mcp manager exists"
action = "assert-visible"
target = "[data-test='mcp-manager']"

[[step]]
name = "mcp empty state visible in the default fixture"
action = "assert-visible"
target = "[data-test='mcp-empty-state']"
```

- [ ] **Step 2: Run, collect, classify, fix**

Run:

```bash
just test-pilot
source scripts/audit-helpers.sh
pilot_collect_evidence S4-mcp
pilot_probe_tab_order 40 > audit-runs/S4-tab-order.json
```

Expected: every P0/P1/P2 MCP finding has a Vitest regression and, where visible in the desktop shell, a TOML assertion.

- [ ] **Step 3: Commit S4 scenario**

Run:

```bash
git add apps/agent-gui/e2e-pilot/audit-mcp.toml apps/agent-gui/src/components/McpServerManager.vue apps/agent-gui/src/components/McpStatusIndicator.vue apps/agent-gui/src/components/McpServerManager.test.ts apps/agent-gui/src/components/McpStatusIndicator.test.ts
git commit -m "test(gui): add mcp pilot audit scenario"
```

---

## Task 10: Add `audit-marketplace-memory.toml` and run S5 audit loop

**Files:**

- Create: `apps/agent-gui/e2e-pilot/audit-marketplace-memory.toml`
- Modify as findings require: `MarketplaceView.vue`, `MarketplacePane.vue`, `CatalogList.vue`, `CatalogCard.vue`, `CatalogDetail.vue`, `InstallProgress.vue`, `InstalledList.vue`, `RuntimeMissingHint.vue`, `MemoryBrowser.vue`, related tests.

- [ ] **Step 1: Create the Marketplace + Memory scenario**

Create `apps/agent-gui/e2e-pilot/audit-marketplace-memory.toml`:

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Audit marketplace and memory"
fail_fast = true
global_timeout_ms = 90000

[[step]]
name = "wait for app shell"
action = "wait"
selector = "[data-test='app-shell']"
timeout_ms = 15000

[[step]]
name = "open settings"
action = "click"
target = "[data-test='nav-settings']"

[[step]]
name = "settings view visible"
action = "assert-visible"
target = "[data-test='view-settings']"

[[step]]
name = "open marketplace settings tab"
action = "click"
target = "[data-test='settings-tab-marketplace']"

[[step]]
name = "browse tab visible"
action = "assert-visible"
target = "[data-test='tab-browse']"

[[step]]
name = "installed tab visible"
action = "assert-visible"
target = "[data-test='tab-installed']"

[[step]]
name = "catalog search visible"
action = "assert-visible"
target = "[data-test='catalog-search']"

[[step]]
name = "catalog refresh visible"
action = "assert-visible"
target = "[data-test='catalog-refresh']"

[[step]]
name = "return to workbench"
action = "click"
target = "[data-test='nav-workbench']"

[[step]]
name = "open memory tab in right panel"
action = "click"
target = "[data-test='trace-tab-memory']"

[[step]]
name = "memory browser visible"
action = "assert-visible"
target = "[data-test='memory-browser']"

[[step]]
name = "memory scope select visible"
action = "assert-visible"
target = "[data-test='memory-scope-select']"
```

- [ ] **Step 2: Run, collect, classify, fix**

Run:

```bash
just test-pilot
source scripts/audit-helpers.sh
pilot_collect_evidence S5-marketplace-memory
pilot_probe_tab_order 50 > audit-runs/S5-tab-order.json
```

Expected: every P0/P1/P2 Marketplace or MemoryBrowser finding has a Vitest regression and a TOML assertion where applicable.

- [ ] **Step 3: Commit S5 scenario**

Run:

```bash
git add apps/agent-gui/e2e-pilot/audit-marketplace-memory.toml apps/agent-gui/src/views/MarketplaceView.vue apps/agent-gui/src/components/MarketplacePane.vue apps/agent-gui/src/components/MemoryBrowser.vue apps/agent-gui/src/components/marketplace apps/agent-gui/src/components/*Memory*.test.ts apps/agent-gui/src/components/marketplace/*.test.ts
git commit -m "test(gui): add marketplace and memory pilot audit scenario"
```

---

## Task 11: Batch-fix grouped P2 findings and create P3 issues

**Files:**

- Modify as findings require: `apps/agent-gui/src/styles/theme.css`, `apps/agent-gui/src/styles/components.css`, component CSS blocks, locale files, tests.
- Local only: `audit-runs/REPORT.md`
- Local only: `audit-runs/p3-issues.md`

- [ ] **Step 1: Group P2 findings by shared root cause**

Edit `audit-runs/REPORT.md` with this structure:

```markdown
# GUI Pilot Audit Report

## Findings

| Issue ID | Severity | Scenario | Description | Root cause | Evidence | Fix commit | Regression |
| -------- | -------- | -------- | ----------- | ---------- | -------- | ---------- | ---------- |

Add rows only after the finding has a real evidence path. Do not add a row with an empty fix commit or regression column.

## P2 Groups

### P2-G1: Focus ring consistency

- Affected selectors: `[data-test='theme-toggle']`, `[data-test='send-button']`
- Fix files: `apps/agent-gui/src/styles/components.css`
- Verification: `pilot_probe_focus_ring` diff > 0.1% for each selector

## P3 Issue Drafts

See `audit-runs/p3-issues.md`.
```

- [ ] **Step 2: Fix each P2 group with tests**

For each grouped P2 issue:

1. Add or update a Vitest assertion for class/attribute/state behavior.
2. Update shared CSS or component code.
3. Run the affected test file.
4. Run the scenario that found the issue.
5. Commit with a concrete message such as `fix(gui): improve memory browser empty state contrast` or `style(gui): unify visible focus treatment across audit controls`.

Example commit:

```bash
git add apps/agent-gui/src/styles/components.css apps/agent-gui/src/components/StatusBar.test.ts
git commit -m "style(gui): unify visible focus treatment across audit controls"
```

- [ ] **Step 3: Draft P3 issues**

Create `audit-runs/p3-issues.md` locally. Every entry must include a concrete title, labels, evidence path, and suggested direction:

```markdown
# P3 Issue Drafts

## More refined chat streaming motion

Labels: gui, enhancement

Body:
The GUI pilot audit found an optional visual improvement in chat streaming motion.
This is P3 because the current behavior is usable and all P0/P1/P2 checks pass.
Evidence: audit-runs/S3-chat-20260509T120000Z/screenshots/streaming-after.png
Suggested direction: refine motion timing while preserving `prefers-reduced-motion` behavior.
```

- [ ] **Step 4: Create GitHub issues from P3 drafts**

For each P3 item, write the issue body to `/tmp/kairox-p3-issue-body.md`, then run a concrete command such as:

```bash
gh issue create --title "Refine chat streaming motion" --label gui --label enhancement --body-file /tmp/kairox-p3-issue-body.md
```

Use only these additional labels when applicable: `enhancement`, `tooling`, `feature`, `documentation`. Do not use `bug` for P3 findings.

---

## Task 12: Append final report summary to the spec and run closure gates

**Files:**

- Modify: `docs/superpowers/specs/2026-05-09-gui-pilot-audit-design.md`

- [ ] **Step 1: Append the report summary table to section 11**

Replace the existing section 11 implementation-phase note with a summary table copied from `audit-runs/REPORT.md`:

```markdown
## 11. Execution Results

| Issue ID         | Severity | One-sentence description                             | Fix commit | Regression test (Vitest file + TOML step)                          |
| ---------------- | -------- | ---------------------------------------------------- | ---------- | ------------------------------------------------------------------ |
| P1-S3-send-error | P1       | Chat send failure now shows visible inline feedback. | `abc1234`  | `ChatPanel.test.ts`; `audit-chat.toml` step `message list visible` |
```

- [ ] **Step 2: Run formatting and lint gates**

Run:

```bash
pnpm run format:check
pnpm run lint
```

Expected: both commands exit 0 with no warnings.

- [ ] **Step 3: Run Rust and GUI tests**

Run:

```bash
cargo test --workspace --all-targets
just test-gui
```

Expected: both commands exit 0.

- [ ] **Step 4: Run pilot and type-sync gates**

Run:

```bash
just test-pilot
just check-types
```

Expected: all five `audit-*.toml` scenarios pass, existing pilot scenarios still pass, and generated TypeScript bindings are clean.

- [ ] **Step 5: Verify no ignored audit artifacts are staged**

Run:

```bash
git status --short | cat
git check-ignore -v audit-runs/REPORT.md audit-runs/p3-issues.md | cat
```

Expected: no `audit-runs/` files appear in `git status`; `git check-ignore` confirms they are ignored.

- [ ] **Step 6: Commit spec summary**

Run:

```bash
git add docs/superpowers/specs/2026-05-09-gui-pilot-audit-design.md
git commit -m "docs(gui): append pilot audit report summary"
```

- [ ] **Step 7: Push and open PR**

Run:

```bash
git push origin feat/gui-pilot-audit-fixes
gh pr create --base main --head feat/gui-pilot-audit-fixes --title "test(gui): run pilot-driven GUI audit" --body-file /tmp/kairox-gui-pilot-audit-pr.md
```

The PR body must include:

```markdown
## Summary

- Added stable pilot audit scenarios for bootstrap, sessions, chat, MCP, marketplace, and memory.
- Fixed all P0/P1/P2 findings discovered by the audit.
- Created GitHub issues for all P3 follow-ups.

## Audit report summary

<copy section 11 table here>

<details>
<summary>Full local audit report</summary>

<paste audit-runs/REPORT.md contents here>

</details>

## Verification

- [ ] `pnpm run format:check`
- [ ] `pnpm run lint`
- [ ] `cargo test --workspace --all-targets`
- [ ] `just test-gui`
- [ ] `just test-pilot`
- [ ] `just check-types`
```

---

## Implementation Rules for Every P0/P1/P2 Fix

Use this loop for each independent issue:

1. Write a failing Vitest test first.
2. Add or update the `audit-*.toml` assertion when the behavior is observable through pilot.
3. Implement the smallest Vue/CSS/i18n fix.
4. Run the targeted Vitest file and the relevant pilot scenario.
5. Record evidence and the eventual commit hash in `audit-runs/REPORT.md`.
6. Commit with `fix(gui): <specific issue>`.

Do not change Rust crates, generated files, `scripts/run-pilot-tests.sh`, router architecture, i18n framework, or UI library choices for this spec.

## Severity Rubric Reference

| Priority | Must fix?    | Examples                                                                                                                                                    |
| -------- | ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0       | Yes          | Crash, blank screen, core path blocked, keyboard cannot reach core control, contrast < 3:1, invisible focus on core controls.                               |
| P1       | Yes          | Missing visible error, missing loading/disabled state, main-flow contrast 3:1-4.5:1, sustained FPS < 30, missing `common.*`, `nav.*`, or `settings.*` i18n. |
| P2       | Yes          | Token mismatch, inconsistent hover/focus, reduced-motion violation, empty state/skeleton gap, non-core a11y issue.                                          |
| P3       | GitHub issue | Nice-to-have style, incremental feature, refactor suggestion.                                                                                               |

## Final Self-Review Checklist

- [ ] Every required selector in spec section 2.1 appears in a Vue file and a selector-preservation test.
- [ ] `scripts/audit-helpers.sh` contains `pilot_fill_textarea`, `pilot_set_reduced_motion`, `pilot_measure_fps`, `pilot_run_axe`, `pilot_probe_tab_order`, `pilot_probe_focus_ring`, and `pilot_collect_evidence`.
- [ ] `apps/agent-gui/public/audit/axe.min.js` is committed and loaded by `pilot_run_axe`.
- [ ] Five `audit-*.toml` scenarios exist and are included automatically by `scripts/run-pilot-tests.sh`.
- [ ] `audit-runs/` is ignored and no local evidence files are staged.
- [ ] Every P0/P1/P2 finding has a fix commit and regression link.
- [ ] Every P3 finding has a GitHub issue URL in `audit-runs/REPORT.md`.
- [ ] Section 11 of `docs/superpowers/specs/2026-05-09-gui-pilot-audit-design.md` contains the final summary table.
- [ ] All closure gates pass before PR creation.
