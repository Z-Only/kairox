# Build Package Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Kairox local and CI build validation faster while reducing release binary/frontend asset size without changing formal Tauri release artifacts.

**Architecture:** The work is split into independent, measurable layers: local command entries, Rust release profile, Cargo feature boundaries, Tauri type-generation boundaries, frontend bundling, Vite/Vitest config sharing, and CI caching. Official release packaging remains on the existing `tauri.conf.json` path with `targets=all` and updater artifacts enabled; lightweight validation uses explicit `--no-bundle` commands.

**Tech Stack:** Rust workspace, Cargo features/profiles, Tauri 2, Vue 3, Vite/Vitest, highlight.js, markdown-it, pnpm, just, GitHub Actions.

---

## File structure

- Modify `justfile`: add `tauri-build-fast`, `gui-size`, `rust-size`; later update `gen-types` to use `--features typegen`.
- Modify `Cargo.toml`: add workspace `[profile.release]` settings.
- Modify `crates/agent-runtime/Cargo.toml`: remove `test-helpers` from default features.
- Modify `crates/agent-tui/Cargo.toml`: enable `agent-runtime/test-helpers` only for dev-dependencies if tests need it.
- Modify `apps/agent-gui/src-tauri/Cargo.toml`: add `typegen` feature and gate export-only dependencies where feasible.
- Modify `apps/agent-gui/src/utils/markdown.ts`: switch from full `highlight.js` entry to `highlight.js/lib/core` with explicit language registration.
- Modify `apps/agent-gui/src/utils/markdown.test.ts`: cover registered language highlighting and unknown language fallback.
- Create `apps/agent-gui/build/vitePlugins.ts`: shared Vite/Vitest plugin factory.
- Modify `apps/agent-gui/vite.config.ts`: use shared plugins and set `build.sourcemap = false`.
- Modify `apps/agent-gui/vitest.config.ts`: use shared plugins.
- Modify `.github/workflows/ci.yml`: add Playwright cache, cache pilot CLI, and remove GUI native dependency installation from TUI-only build.

## Task 1: Add baseline measurement and lightweight local build commands

**Files:**

- Modify: `justfile`

- [ ] **Step 1: Edit `justfile` to add lightweight build and size recipes**

Insert these recipes immediately after the existing `tauri-build` recipe:

```make
# Build Tauri desktop app without generating installer bundles
tauri-build-fast: gen-types
    pnpm --filter agent-gui exec -- tauri build --no-bundle

# Build GUI web assets and print the largest generated files
gui-size: gui-build
    @du -sh apps/agent-gui/dist
    @find apps/agent-gui/dist -type f -exec ls -lh {} \; | sort -k5 -hr | head -30 | cat

# Print release binary sizes when the binaries have already been built
rust-size:
    @test -f target/release/agent-tui && ls -lh target/release/agent-tui || echo "target/release/agent-tui not built"
    @test -f target/release/agent-gui-tauri && ls -lh target/release/agent-gui-tauri || echo "target/release/agent-gui-tauri not built"
```

Keep the existing `tauri-build` recipe unchanged:

```make
# Build Tauri desktop app
tauri-build: gen-types
    pnpm --filter agent-gui run tauri:build
```

- [ ] **Step 2: Verify the new recipes are discoverable**

Run:

```bash
just --list | cat
```

Expected: output includes `tauri-build-fast`, `gui-size`, and `rust-size`.

- [ ] **Step 3: Verify GUI size measurement runs**

Run:

```bash
just gui-size
```

Expected: `pnpm --filter agent-gui run build` succeeds, then the command prints one `du -sh apps/agent-gui/dist` line and up to 30 generated files.

- [ ] **Step 4: Verify lightweight Tauri compile path**

Run:

```bash
just tauri-build-fast
```

Expected: `just gen-types` succeeds and Tauri runs `tauri build --no-bundle`. If platform system packages are missing, capture the exact missing dependency message and continue only after installing or documenting the environment limitation.

- [ ] **Step 5: Commit local command entries**

```bash
git add justfile
git commit -m "chore: add lightweight build measurement commands"
```

## Task 2: Add Rust release profile optimization

**Files:**

- Modify: `Cargo.toml`

- [ ] **Step 1: Record current release binary size when available**

Run:

```bash
just rust-size
```

Expected: prints existing binary sizes or explicit `not built` messages. Save the output in the task notes for comparison.

- [ ] **Step 2: Edit root `Cargo.toml` to add release profile settings**

Append this section after `[workspace.dependencies]`:

```toml
[profile.release]
strip = "symbols"
lto = "thin"
codegen-units = 1
```

Do not add `panic = "abort"` in this implementation pass.

- [ ] **Step 3: Verify Rust tests still pass**

Run:

```bash
cargo test --workspace --all-targets
```

Expected: all workspace tests pass.

- [ ] **Step 4: Verify TUI release build**

Run:

```bash
cargo build -p agent-tui --release
```

Expected: release build completes and produces `target/release/agent-tui`.

- [ ] **Step 5: Record optimized TUI binary size**

Run:

```bash
just rust-size
```

Expected: `target/release/agent-tui` size is printed. Compare it with the Step 1 baseline when a baseline existed.

- [ ] **Step 6: Commit release profile change**

```bash
git add Cargo.toml
git commit -m "perf: optimize release binary profile"
```

## Task 3: Remove `agent-runtime/test-helpers` from production defaults

**Files:**

- Modify: `crates/agent-runtime/Cargo.toml`
- Modify if needed: `crates/agent-tui/Cargo.toml`

- [ ] **Step 1: Edit `crates/agent-runtime/Cargo.toml` default feature**

Change:

```toml
[features]
default = ["test-helpers"]
test-helpers = []
```

To:

```toml
[features]
default = []
test-helpers = []
```

- [ ] **Step 2: Run the workspace tests to identify explicit feature needs**

Run:

```bash
cargo test --workspace --all-targets
```

Expected: either all tests pass, or failures identify test code that needs `agent-runtime/test-helpers` explicitly.

- [ ] **Step 3: If `agent-tui` tests need helpers, update only its dev-dependency**

If Step 2 reports missing test-helper symbols from `agent-tui` tests, change the `agent-runtime` dev-dependency in `crates/agent-tui/Cargo.toml` from:

```toml
agent-runtime = { path = "../agent-runtime" }
```

To:

```toml
agent-runtime = { path = "../agent-runtime", features = ["test-helpers"] }
```

Leave the normal `[dependencies]` entry unchanged:

```toml
agent-runtime = { path = "../agent-runtime" }
```

- [ ] **Step 4: Re-run tests after any manifest adjustment**

Run:

```bash
cargo test --workspace --all-targets
```

Expected: all workspace tests pass.

- [ ] **Step 5: Verify all-feature clippy still covers helper code**

Run:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: clippy passes with zero warnings.

- [ ] **Step 6: Inspect GUI production feature graph**

Run:

```bash
cargo tree -p agent-gui-tauri --release -e features | grep 'agent-runtime feature "test-helpers"' || true
```

Expected: no `agent-runtime feature "test-helpers"` line appears for the normal release graph.

- [ ] **Step 7: Commit production feature cleanup**

```bash
git add crates/agent-runtime/Cargo.toml crates/agent-tui/Cargo.toml
git commit -m "perf(runtime): remove test helpers from defaults"
```

## Task 4: Add explicit Tauri type generation feature path

**Files:**

- Modify: `apps/agent-gui/src-tauri/Cargo.toml`
- Modify: `justfile`

- [ ] **Step 1: Add a `typegen` feature in `apps/agent-gui/src-tauri/Cargo.toml`**

Change the feature section from:

```toml
[features]
pilot = ["dep:tauri-plugin-pilot"]
```

To:

```toml
[features]
pilot = ["dep:tauri-plugin-pilot"]
typegen = []
```

Keep `specta`, `specta-typescript`, and `tauri-specta` as normal dependencies in this pass because `src/specta.rs` and `src/lib.rs` currently use `tauri_specta::Builder` on the runtime path.

- [ ] **Step 2: Update `just gen-types` to use the explicit typegen feature**

Change the two `cargo run` lines in `justfile` from:

```make
    cargo run -p agent-gui-tauri --bin export-specta -- apps/agent-gui/src/generated/commands.ts
    cargo run -p agent-gui-tauri --bin export-events -- apps/agent-gui/src/generated/events.ts
```

To:

```make
    cargo run -p agent-gui-tauri --features typegen --bin export-specta -- apps/agent-gui/src/generated/commands.ts
    cargo run -p agent-gui-tauri --features typegen --bin export-events -- apps/agent-gui/src/generated/events.ts
```

- [ ] **Step 3: Verify type generation still works**

Run:

```bash
just gen-types
```

Expected: generated `commands.ts` and `events.ts` are written successfully.

- [ ] **Step 4: Verify generated files stay in sync**

Run:

```bash
just check-types
```

Expected: command exits successfully with `Generated types are in sync`.

- [ ] **Step 5: Verify normal GUI release build still compiles**

Run:

```bash
cargo build -p agent-gui-tauri --release
```

Expected: build completes. If platform dependencies are missing, capture the exact package error and verify via CI later.

- [ ] **Step 6: Commit explicit typegen path**

```bash
git add apps/agent-gui/src-tauri/Cargo.toml justfile apps/agent-gui/src/generated/commands.ts apps/agent-gui/src/generated/events.ts
git commit -m "chore(gui): make type generation feature explicit"
```

## Task 5: Optimize Markdown syntax highlighting bundle size

**Files:**

- Modify: `apps/agent-gui/src/utils/markdown.ts`
- Modify: `apps/agent-gui/src/utils/markdown.test.ts`

- [ ] **Step 1: Update Markdown tests for registered and unknown languages**

Replace the existing `apps/agent-gui/src/utils/markdown.test.ts` content with:

````ts
import { describe, it, expect } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders plain text as a paragraph", () => {
    const result = renderMarkdown("Hello world");
    expect(result).toContain("<p>");
    expect(result).toContain("Hello world");
  });

  it.each([
    ["rust", "fn main() {}", "hljs-keyword"],
    ["typescript", "const value: string = 'ok';", "hljs-keyword"],
    ["json", '{"ok": true}', "hljs-attr"],
    ["bash", "echo hello", "hljs-built_in"]
  ])("highlights registered %s code blocks", (language, code, expectedClass) => {
    const result = renderMarkdown(`\`\`\`${language}\n${code}\n\`\`\``);
    expect(result).toContain("hljs");
    expect(result).toContain(expectedClass);
  });

  it("falls back to escaped HTML for code blocks with unknown language", () => {
    const result = renderMarkdown("```foobar\n<script>bad()</script>\n```");
    expect(result).toContain("<pre");
    expect(result).toContain("&lt;script&gt;bad()&lt;/script&gt;");
    expect(result).not.toContain("language-foobar");
  });

  it("renders inline code", () => {
    const result = renderMarkdown("Use `cargo test` to run");
    expect(result).toContain("<code>");
    expect(result).toContain("cargo test");
  });

  it("escapes HTML to prevent XSS", () => {
    const result = renderMarkdown('<script>alert("xss")</script>');
    expect(result).not.toContain("<script>");
    expect(result).toContain("&lt;script&gt;");
  });
});
````

- [ ] **Step 2: Run the focused test and confirm it fails before implementation**

Run:

```bash
pnpm --filter agent-gui exec vitest run src/utils/markdown.test.ts
```

Expected: this may pass with current full `highlight.js`; the important assertion is that tests describe the required registered-language behavior and unknown-language fallback before the implementation is changed.

- [ ] **Step 3: Replace full highlighter import with explicit language registration**

Replace `apps/agent-gui/src/utils/markdown.ts` with:

```ts
import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import yaml from "highlight.js/lib/languages/yaml";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("shell", bash);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("md", markdown);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("rs", rust);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("yml", yaml);
hljs.registerLanguage("toml", rust);

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
  highlight(source: string, language: string): string {
    if (language && hljs.getLanguage(language)) {
      try {
        const highlightedCode = hljs.highlight(source, { language }).value;
        return `<pre class="hljs"><code>${highlightedCode}</code></pre>`;
      } catch {
        return renderPlainCodeBlock(source);
      }
    }

    return renderPlainCodeBlock(source);
  }
});

function renderPlainCodeBlock(source: string): string {
  return `<pre class="hljs"><code>${md.utils.escapeHtml(source)}</code></pre>`;
}

export function renderMarkdown(text: string): string {
  return md.render(text);
}
```

Note: if `highlight.js/lib/languages/toml` exists in the installed package, prefer importing that language and registering `toml` with it. If it does not exist, keep the fallback above and rely on plain safe rendering for TOML by removing the `toml` registration line.

- [ ] **Step 4: Run focused Markdown tests**

Run:

```bash
pnpm --filter agent-gui exec vitest run src/utils/markdown.test.ts
```

Expected: all Markdown tests pass.

- [ ] **Step 5: Run full GUI tests and build**

Run:

```bash
pnpm --filter agent-gui run test
pnpm --filter agent-gui run build
just gui-size
```

Expected: tests and build pass; `gui-size` prints asset sizes without increasing total `dist` size compared with the baseline collected earlier.

- [ ] **Step 6: Commit highlighter optimization**

```bash
git add apps/agent-gui/src/utils/markdown.ts apps/agent-gui/src/utils/markdown.test.ts
git commit -m "perf(gui): register highlight languages explicitly"
```

## Task 6: Share Vite and Vitest plugin configuration

**Files:**

- Create: `apps/agent-gui/build/vitePlugins.ts`
- Modify: `apps/agent-gui/vite.config.ts`
- Modify: `apps/agent-gui/vitest.config.ts`

- [ ] **Step 1: Create shared plugin factory**

Create `apps/agent-gui/build/vitePlugins.ts` with:

```ts
import vue from "@vitejs/plugin-vue";
import AutoImport from "unplugin-auto-import/vite";
import Components from "unplugin-vue-components/vite";

const vueUseImports = [
  "useDark",
  "useColorMode",
  "useStorage",
  "useEventListener",
  "tryOnScopeDispose",
  "useDebounceFn",
  "useThrottleFn",
  "useIntervalFn",
  "useTimeoutFn",
  "useClipboard",
  "useFocus"
];

export function createKairoxVitePlugins() {
  return [
    vue(),
    AutoImport({
      imports: [
        "vue",
        "vue-router",
        "pinia",
        "vue-i18n",
        {
          "@vueuse/core": vueUseImports
        }
      ],
      dts: "src/auto-imports.d.ts",
      dirs: [],
      vueTemplate: true
    }),
    Components({
      dirs: ["src/components"],
      extensions: ["vue"],
      deep: true,
      dts: "src/components.d.ts"
    })
  ];
}
```

- [ ] **Step 2: Update `apps/agent-gui/vite.config.ts`**

Replace direct plugin imports with:

```ts
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import { createKairoxVitePlugins } from "./build/vitePlugins";
```

Set plugins and build config like this:

```ts
export default defineConfig({
  plugins: createKairoxVitePlugins(),
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url))
    }
  },
  build: {
    sourcemap: false
  },
  clearScreen: false,
  server: { port: 1420, host: "0.0.0.0" }
});
```

- [ ] **Step 3: Update `apps/agent-gui/vitest.config.ts`**

Replace direct plugin imports with:

```ts
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";
import { createKairoxVitePlugins } from "./build/vitePlugins";
```

Use the shared plugin factory:

```ts
export default defineConfig({
  plugins: createKairoxVitePlugins(),
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url))
    }
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,vue}"],
      exclude: ["src/generated/**", "src/env.d.ts"]
    }
  }
});
```

- [ ] **Step 4: Verify frontend tests and build**

Run:

```bash
pnpm --filter agent-gui run test
pnpm --filter agent-gui run build
pnpm run lint:web
```

Expected: tests, build, and web lint pass.

- [ ] **Step 5: Verify production source maps are not emitted**

Run:

```bash
find apps/agent-gui/dist -name '*.map' -print | cat
```

Expected: no output.

- [ ] **Step 6: Commit shared Vite config**

```bash
git add apps/agent-gui/build/vitePlugins.ts apps/agent-gui/vite.config.ts apps/agent-gui/vitest.config.ts
git commit -m "refactor(gui): share vite plugin configuration"
```

## Task 7: Optimize CI setup and caching

**Files:**

- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add Playwright browser cache in `test-e2e` job**

Add this step after `Setup Node.js` and before `Install repo tooling deps` in the `test-e2e` job:

```yaml
- name: Cache Playwright browsers
  uses: actions/cache@v4
  with:
    path: ~/.cache/ms-playwright
    key: playwright-${{ runner.os }}-${{ hashFiles('pnpm-lock.yaml') }}
```

Keep the existing install step:

```yaml
- name: Install Playwright browsers
  working-directory: apps/agent-gui
  run: npx playwright install --with-deps chromium
```

Expected behavior: the install step remains idempotent and becomes faster on cache hits.

- [ ] **Step 2: Remove GUI native dependency installation from `build-tui` job**

Delete the entire `Install Linux system deps` step from the `build-tui` job only. Do not remove the Tauri dependency steps from `lint-rust`, `test`, `type-sync`, or `tauri-pilot-e2e`.

The `build-tui` job should go directly from cargo cache setup to:

```yaml
- name: Build TUI
  run: cargo build -p agent-tui
```

- [ ] **Step 3: Cache installed `tauri-pilot` binary**

Add this step after cargo cache setup in the `tauri-pilot-e2e` job:

```yaml
- name: Cache tauri-pilot CLI
  id: cache-tauri-pilot
  uses: actions/cache@v4
  with:
    path: ~/.cargo/bin/tauri-pilot
    key: tauri-pilot-${{ matrix.os }}-v0.5.1
```

Change the install step from:

```yaml
- name: Install tauri-pilot CLI
  run: cargo install --git https://github.com/mpiton/tauri-pilot --tag v0.5.1 tauri-pilot-cli --locked
```

To:

```yaml
- name: Install tauri-pilot CLI
  if: steps.cache-tauri-pilot.outputs.cache-hit != 'true'
  run: cargo install --git https://github.com/mpiton/tauri-pilot --tag v0.5.1 tauri-pilot-cli --locked
```

- [ ] **Step 4: Validate workflow syntax locally if tooling is available**

Run:

```bash
python3 - <<'PY'
import pathlib
import yaml
for path in [pathlib.Path('.github/workflows/ci.yml')]:
    yaml.safe_load(path.read_text())
    print(f'parsed {path}')
PY
```

Expected: prints `parsed .github/workflows/ci.yml`. If `PyYAML` is not installed, run `pnpm run format:check` instead and rely on GitHub Actions for workflow parsing.

- [ ] **Step 5: Commit CI optimization**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: cache heavy gui test dependencies"
```

## Task 8: Final verification and measurement

**Files:**

- No code files expected unless earlier verification reveals a concrete defect.

- [ ] **Step 1: Run formatting checks**

```bash
pnpm run format:check
```

Expected: formatting check passes.

- [ ] **Step 2: Run lint checks**

```bash
pnpm run lint
```

Expected: clippy, oxlint, and stylelint pass.

- [ ] **Step 3: Run Rust tests**

```bash
cargo test --workspace --all-targets
```

Expected: all Rust tests pass.

- [ ] **Step 4: Run GUI tests and build**

```bash
pnpm --filter agent-gui run test
pnpm --filter agent-gui run build
```

Expected: GUI tests and Vite production build pass.

- [ ] **Step 5: Run type sync check**

```bash
just check-types
```

Expected: generated TypeScript files are in sync.

- [ ] **Step 6: Run release compile checks**

```bash
cargo build -p agent-tui --release
cargo build -p agent-gui-tauri --release
```

Expected: both release builds pass. If `agent-gui-tauri` fails because local native Tauri packages are missing, record the exact package names and verify in CI or a prepared native build environment.

- [ ] **Step 7: Run lightweight Tauri no-bundle validation**

```bash
just tauri-build-fast
```

Expected: no-bundle Tauri build passes, or local native dependency limitations are documented with exact error text.

- [ ] **Step 8: Capture size and feature measurements**

Run:

```bash
just rust-size
just gui-size
cargo tree -p agent-gui-tauri --release -e features > /tmp/kairox-agent-gui-tauri-features.txt
cargo tree -p agent-gui-tauri --release -i tempfile | cat
cargo tree -p agent-gui-tauri --release -i specta | cat
```

Expected: size output is available for review, and the feature graph can be inspected from `/tmp/kairox-agent-gui-tauri-features.txt`.

- [ ] **Step 9: Inspect final diff**

```bash
git status --short | cat
git log --oneline --decorate -8 | cat
```

Expected: working tree is clean after all task commits, and recent commits show the staged optimization sequence.
