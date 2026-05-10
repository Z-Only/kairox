# Build and Package Optimization Design

## Context

Kairox is a local-first AI agent workbench with a shared Rust workspace, a ratatui TUI, and a Tauri 2 + Vue 3 desktop GUI. Current build and release paths cover Rust tests, GUI web assets, type generation, Playwright E2E, tauri-pilot desktop E2E, TUI binaries, and full Tauri desktop bundles.

This design optimizes compile/package speed and package size while preserving formal release completeness.

## Goals

- Reduce local and CI validation time for common development paths.
- Reduce release binary and frontend asset size where the change is measurable and safe.
- Keep official Tauri release artifacts complete: `bundle.targets = "all"` and `bundle.createUpdaterArtifacts = true` remain unchanged.
- Separate development/verification build paths from formal release packaging.
- Make each optimization independently testable and reversible.

## Non-goals

- Do not remove any formal Tauri installer target from release builds.
- Do not disable updater artifact generation for tagged releases.
- Do not redesign the product architecture or IPC model.
- Do not replace the existing package manager, test framework, or release workflow wholesale.
- Do not introduce broad unrelated refactors.

## Chosen approach

Use an aggressive but staged optimization approach:

1. Add lightweight local and CI build entries based on `tauri build --no-bundle`.
2. Optimize Rust release profiles for final binaries.
3. Remove test-only features from production defaults.
4. Split type-generation-only dependencies from normal Tauri runtime builds where feasible.
5. Replace full `highlight.js` imports with explicit language registration.
6. Reduce duplicated Vite/Vitest plugin configuration.
7. Add targeted CI caching and remove low-value setup work from jobs that do not need it.

This approach preserves release compatibility while making the common validation path much cheaper.

## Design principles

- Formal release stays complete; local and CI validation become lighter.
- Production dependency graphs should not include test helpers by default.
- Type generation should be explicit instead of implicitly coupled to every runtime build where possible.
- Frontend bundle optimization should preserve readable output and avoid runtime crashes for unknown languages.
- CI changes should reduce repeated heavy setup without weakening required gates.
- Risky optimizations must have clear rollback order.

## Components and build flow

### Rust and Tauri backend

#### Workspace release profile

Add a root-level release profile in `Cargo.toml` for final binary size optimization.

Initial release profile candidates:

- `strip = "symbols"`
- `lto = "thin"`
- `codegen-units = 1`

`panic = "abort"` is intentionally excluded from the first implementation batch. It may reduce binary size, but it also changes panic behavior and can make diagnostics worse.

Affected paths:

- `cargo build --release`
- `cargo build -p agent-tui --release`
- `cargo build -p agent-gui-tauri --release`
- `pnpm --filter agent-gui exec -- tauri build`
- release workflow builds

Debug builds, Tauri dev mode, and pilot debug E2E are not affected by release profile changes.

#### `agent-runtime/test-helpers` default feature

Change `crates/agent-runtime/Cargo.toml` so `test-helpers` is no longer enabled by default.

Target state:

- `default = []`
- `test-helpers` remains available.
- Tests and fixtures that require helpers enable `features = ["test-helpers"]` explicitly.
- Production crates such as `agent-gui-tauri` and `agent-tui` do not receive test helpers through defaults.

This makes production feature selection explicit and reduces accidental test-only dependency propagation.

#### Specta and type generation split

Separate ordinary Tauri app builds from TypeScript binding generation as much as the current `tauri-specta` integration allows.

Target shape:

- Add a `typegen` feature to `apps/agent-gui/src-tauri/Cargo.toml`.
- Run export binaries with `--features typegen`.
- Move type-export-only dependencies behind `typegen` where feasible.
- Keep runtime-required `tauri-specta` pieces if command registration or event registration requires them.

The split is best-effort. If a dependency is needed for runtime command collection, it remains in the runtime graph and the implementation documents that reason.

#### Tauri release bundle

Keep official release packaging unchanged:

- `apps/agent-gui/src-tauri/tauri.conf.json` keeps `bundle.targets = "all"`.
- `apps/agent-gui/src-tauri/tauri.conf.json` keeps `bundle.createUpdaterArtifacts = true`.
- Release workflows still produce full desktop bundles and updater artifacts.

Add lightweight verification paths using `tauri build --no-bundle` for local development and ordinary CI compile checks.

### Vue and Vite frontend

#### Markdown and syntax highlighting

Replace the full `highlight.js` entry import with `highlight.js/lib/core` and explicit language registration.

Initial registered languages:

- Rust
- TypeScript
- JavaScript
- JSON
- Bash
- TOML
- YAML
- Markdown

Unknown languages render as safe plain code blocks instead of throwing. This preserves chat readability while reducing bundled highlighter code.

#### Vite production build strategy

Make production build behavior explicit in `apps/agent-gui/vite.config.ts`:

- `build.sourcemap = false`
- Add manual chunking only if measurement shows a remaining large chunk after highlighter optimization.

Manual chunking is not the first optimization because this is a local Tauri app; browser network cache benefits are less important than total asset size and build simplicity.

#### Vite and Vitest plugin configuration

Extract shared plugin creation for Vite and Vitest if the existing duplicated AutoImport/Components setup drifts.

Target shape:

- A small helper such as `apps/agent-gui/build/vitePlugins.ts`.
- It creates the same AutoImport and Components plugins for both Vite and Vitest.
- It preserves current project rules: auto-import transforms `.vue` files only, and plain `.ts` modules keep explicit imports.

### Local command flow

Add lightweight commands without changing existing formal build semantics.

Proposed commands:

- `just tauri-build-fast`: run type generation and `tauri build --no-bundle`.
- `just gui-size`: build GUI web assets and print total `dist` size plus largest files.
- Optional `just rust-size`: print release binary sizes for `agent-tui` and `agent-gui-tauri` when present.

Existing complete build commands keep their current meaning.

### CI and release flow

#### PR and main CI

Keep required gates:

- format check
- lint
- Rust tests
- TUI build
- GUI web build
- type-sync
- Playwright E2E
- tauri-pilot desktop E2E
- live model smoke tests

Optimize only setup and compile verification paths:

- Use `--no-bundle` for any ordinary Tauri compile verification.
- Cache Playwright browser downloads.
- Cache or reuse `tauri-pilot` CLI installation when possible.
- Remove Tauri/Linux GUI native dependency installation from jobs that only build the TUI if verification confirms it is unnecessary.

#### Manual verify and tagged release

Keep full packaging behavior:

- `verify-build.yml` remains the place for full manual package verification.
- `release-build.yml` keeps full Tauri `targets=all` packaging.
- Updater artifacts and checksums remain part of tagged releases.

Release asset post-processing can be optimized later if it is independent and measurable.

## Implementation phases

### Phase 1: Baseline and lightweight build entries

Files likely changed:

- `justfile`
- `apps/agent-gui/package.json` if package scripts are useful

Acceptance criteria:

- `just tauri-build-fast` reaches `tauri build --no-bundle`.
- `just gui-size` prints `apps/agent-gui/dist` size and largest files.
- Existing complete Tauri build command semantics remain unchanged.
- `tauri.conf.json` release settings remain unchanged.

### Phase 2: Rust release profile optimization

Files likely changed:

- `Cargo.toml`

Acceptance criteria:

- `cargo test --workspace --all-targets` passes.
- `cargo build -p agent-tui --release` passes.
- `pnpm --filter agent-gui exec -- tauri build --no-bundle` passes where local dependencies allow it.
- Binary sizes are recorded before and after the profile change.

Rollback order:

1. Remove `lto = "thin"`.
2. Restore default `codegen-units`.
3. Remove `strip = "symbols"` only if necessary.

### Phase 3: Production feature cleanup for `agent-runtime`

Files likely changed:

- `crates/agent-runtime/Cargo.toml`
- dependent crate manifests or test manifests that need `test-helpers`

Acceptance criteria:

- `cargo test --workspace --all-targets` passes.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
- Production `agent-gui-tauri` release feature tree no longer enables `agent-runtime/test-helpers` by default.

### Phase 4: Specta/typegen feature split

Files likely changed:

- `apps/agent-gui/src-tauri/Cargo.toml`
- `justfile`
- possibly Tauri export binary configuration

Acceptance criteria:

- `just gen-types` passes.
- `just check-types` passes.
- `cargo build -p agent-gui-tauri --release` passes.
- `pnpm --filter agent-gui exec -- tauri build --no-bundle` passes where local dependencies allow it.
- Type-generation-only dependencies are outside the normal release graph where feasible, or retained reasons are documented in code comments or commit notes.

### Phase 5: Frontend highlighter optimization

Files likely changed:

- `apps/agent-gui/src/utils/markdown.ts`
- `apps/agent-gui/src/main.ts` only if style imports need adjustment
- existing or new Markdown rendering tests

Acceptance criteria:

- Registered languages render highlighted code.
- Unknown languages render safe plain code.
- Invalid Markdown or malformed code fences do not crash rendering.
- `pnpm --filter agent-gui run test` passes.
- `pnpm --filter agent-gui run build` passes.
- `just gui-size` shows no asset size regression.

### Phase 6: Vite/Vitest config cleanup

Files likely changed:

- `apps/agent-gui/vite.config.ts`
- `apps/agent-gui/vitest.config.ts`
- optional `apps/agent-gui/build/vitePlugins.ts`

Acceptance criteria:

- `pnpm --filter agent-gui run build` passes.
- `pnpm --filter agent-gui run test` passes.
- `pnpm run lint:web` passes if available.
- Production `dist` does not contain source maps unless explicitly requested by a debug build.

### Phase 7: CI caching and lightweight validation

Files likely changed:

- `.github/workflows/ci.yml`
- `.github/workflows/verify-build.yml` only if documenting or adding a lightweight manual path
- `.github/workflows/release-build.yml` only for independent, low-risk release post-processing cleanup

Acceptance criteria:

- PR/main CI still covers all required quality gates.
- Formal release workflow still builds full Tauri targets and updater artifacts.
- Playwright or pilot CLI setup benefits from caching.
- TUI-only jobs do not install unrelated GUI native dependencies if not needed.

## Final verification matrix

Required local verification where dependencies and platform support are available:

- `pnpm run format:check`
- `pnpm run lint`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo build -p agent-tui --release`
- `cargo build -p agent-gui-tauri --release`
- `just check-types`
- `pnpm --filter agent-gui run test`
- `pnpm --filter agent-gui run build`
- `pnpm --filter agent-gui exec -- tauri build --no-bundle`

Optional heavy verification:

- `pnpm --filter agent-gui exec -- tauri build`
- `just test-e2e`
- `just test-pilot`

If optional heavy verification cannot run locally because of missing display, missing `tauri-pilot-cli`, or platform package constraints, the implementation must record the reason and rely on CI or manual verify jobs for that path.

## Measurement plan

Collect before and after values for:

- `target/release/agent-tui`
- `target/release/agent-gui-tauri`
- `apps/agent-gui/dist` total size
- largest frontend files in `apps/agent-gui/dist`
- `cargo tree -p agent-gui-tauri --release -e features`
- `cargo tree -p agent-gui-tauri --release -i tempfile`
- `cargo tree -p agent-gui-tauri --release -i specta`
- relevant GitHub Actions job durations after CI changes merge

## Risks and mitigations

### Release profile slows builds too much

Mitigation: keep changes staged and rollback in order: `lto`, then `codegen-units`, then `strip`.

### Feature split breaks tests

Mitigation: explicitly enable `test-helpers` in test-only consumers instead of restoring production defaults.

### Specta split cannot fully remove runtime dependencies

Mitigation: preserve runtime-required dependencies and move only export-only dependencies. The success condition is a cleaner graph, not forced removal of necessary runtime integration.

### Highlighter language list misses user content

Mitigation: unknown languages fall back to safe plain text. Add more explicit languages if users report common missing cases.

### CI cache keys become stale or too broad

Mitigation: bind Playwright cache keys to `pnpm-lock.yaml` or Playwright version. Keep cargo cache changes minimal and avoid over-fragmenting keys.

## Success criteria

The optimization is successful when:

- Official Tauri release targets and updater artifacts are preserved.
- A lightweight Tauri `--no-bundle` validation path exists locally and in CI where appropriate.
- At least one heavy CI setup path is cached or removed where unnecessary.
- Rust release profile optimizations are applied and measured.
- `agent-runtime/test-helpers` is no longer a production default.
- `highlight.js` is no longer imported through the full package entry.
- Type-generation-only dependencies are isolated where feasible.
- All required verification commands pass or have documented environment-specific skip reasons.
