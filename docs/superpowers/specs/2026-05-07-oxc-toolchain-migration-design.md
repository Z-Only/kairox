# Oxc Toolchain Migration Design

**Date**: 2026-05-07
**Status**: Draft
**Author**: AI Copilot + 蝉雨
**Branch**: `feat/oxc-toolchain-migration`

## Overview

Replace ESLint + Prettier with Oxc toolchain (Oxlint + Oxfmt) for the Kairox project. Vite 8 already ships with Rolldown + Oxc built-in, so the bundler layer requires no changes.

## Decision Summary

| Decision       | Chosen Option                                                    |
| -------------- | ---------------------------------------------------------------- |
| Risk appetite  | 🔴 Aggressive: Oxlint + Oxfmt, full removal of ESLint + Prettier |
| Oxfmt scope    | Full repo including `.md` / `.json` / `.yaml`                    |
| Bundler change | **None needed** — Vite 8 already uses Rolldown                   |
| Stylelint      | **Kept** — no Oxc CSS linter available                           |
| Configuration  | `.oxfmtrc.json` + `.oxlintrc.json` at repo root                  |

## Replacement Map

| Removed                         | Replaced By      | Notes                                   |
| ------------------------------- | ---------------- | --------------------------------------- |
| `eslint` @10                    | `oxlint`         | Installed at repo root                  |
| `typescript-eslint` @8          | oxlint built-in  | —                                       |
| `eslint-plugin-vue` @10         | oxlint built-in  | —                                       |
| `eslint-config-prettier`        | Deleted          | oxlint has no formatting rule conflicts |
| `prettier` @3                   | `oxfmt`          | CLI compatible, `--check` / `--write`   |
| `@eslint/js` @10                | Deleted          | —                                       |
| `globals` @17                   | Deleted          | Only used by eslint config              |
| `vue-eslint-parser`             | Deleted          | oxlint has built-in Vue parsing         |
| `@rollup/rollup-*` optionalDeps | Deleted          | Vite 8 uses Rolldown, not Rollup        |
| `.prettierrc.json`              | `.oxfmtrc.json`  | —                                       |
| `eslint.config.js`              | `.oxlintrc.json` | —                                       |

| Unchanged               | Reason                                                 |
| ----------------------- | ------------------------------------------------------ |
| `vite` @8               | Vite 8 stable (2026-03-12) has Rolldown + Oxc built-in |
| `@vitejs/plugin-vue` @6 | Vite plugin API compatible with Rolldown               |
| `stylelint` @17         | No Oxc CSS linter                                      |
| `vitest` @4             | No Oxc replacement                                     |
| `playwright` @1.59      | E2E testing unchanged                                  |
| `vue-tsc`               | Type checking unchanged                                |

## Files Changed

### New Files

- `.oxfmtrc.json` — oxfmt configuration
- `.oxlintrc.json` — oxlint configuration
- `.oxfmtignore` — oxfmt ignore rules

### Deleted Files

- `eslint.config.js`
- `.prettierrc.json`
- `.prettierignore`

### Modified Files

- `package.json` (root) — deps + scripts + lint-staged
- `apps/agent-gui/package.json` — remove `@rollup/rollup-*` optionalDeps
- `justfile` — update lint/fmt commands
- `.github/workflows/ci.yml` — ESLint → Oxlint steps
- `.github/workflows/release-build.yml` — no change needed (commands unchanged)
- `.github/workflows/verify-build.yml` — no change needed

## Research Findings

1. **`rolldown-plugin-vue` does not exist** — `@vitejs/plugin-vue` works directly with Vite 8 + Rolldown
2. **Vite 8.0 stable** released 2026-03-12, already ships Rolldown + Oxc. Current project uses `vite@^8.0.10`
3. **oxfmt** (`npm: oxfmt`) supports JS/TS/JSX/TSX/JSON/YAML/TOML/HTML/Vue/CSS/SCSS/Less/Markdown/GraphQL, CLI compatible with Prettier
4. **oxlint** (`npm: oxlint`) replaces ESLint for JS/TS/JSX/TSX/Vue

## CI Workflow Changes

### ci.yml

**format job:**

- `Check formatting` step: `prettier --check` → `npx oxfmt --check .`

**lint-web job (rename to lint-oxlint):**

- `Run ESLint` step → `Run Oxlint`: `npx oxlint`
- `Run Stylelint` step unchanged

### Other workflows

- `release-build.yml` and `verify-build.yml` — no changes needed (build commands unchanged)

## npm Scripts Changes

### Root package.json

```json
"format:check:web": "npx oxfmt --check .",
"format:web": "npx oxfmt --write .",
"lint:web": "npx oxlint && npx stylelint \"apps/agent-gui/src/**/*.{vue,css,scss,sass,less}\"",
"lint:oxlint": "npx oxlint"
```

Remove: `lint:eslint`, `format:check:rust` stays unchanged.

### lint-staged hooks

```json
"*.{json,md}": ["oxfmt --write"],
"apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}": ["oxfmt --write", "oxlint --fix"],
"apps/agent-gui/src/**/*.{vue,css,scss,sass,less}": ["oxfmt --write", "stylelint --fix"]
```

## justfile Changes

```makefile
lint:
  cargo clippy ... && npx oxlint && npx stylelint ...
fmt-check:
  cargo fmt --all --check && npx oxfmt --check .
fmt:
  cargo fmt --all && npx oxfmt --write .
```

## Risks & Mitigations

| Risk                                                                    | Severity | Mitigation                                                    |
| ----------------------------------------------------------------------- | -------- | ------------------------------------------------------------- |
| oxfmt alpha — formatting differences from Prettier                      | Medium   | One-time reformat commit, clearly marked in PR                |
| oxlint fewer rules than eslint-plugin-vue                               | Low      | Accepted per aggressive approach                              |
| oxfmt on .md/.json/.yaml less mature                                    | Low      | Accepted per decision, .cjs files excluded via `.oxfmtignore` |
| .cjs files (`prepare.cjs`, `commitlint.config.js`) unsupported by oxfmt | Low      | Excluded in `.oxfmtignore`                                    |

## Rollback Strategy

If oxfmt/oxlint fails in CI:

1. Adjust `.oxfmtrc.json` / `.oxlintrc.json` config
2. Temporarily skip failing step in CI
3. Track issue for oxfmt/oxlint future release fix
