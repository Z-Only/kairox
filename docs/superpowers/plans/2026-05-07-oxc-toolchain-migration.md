# Oxc Toolchain Migration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ESLint + Prettier with Oxlint + Oxfmt across the Kairox project (root + CI + justfile + lint-staged)

**Architecture:** This is a pure tooling migration — no production code. We swap npm deps in two package.json files, create 3 new config files, delete 3 old ones, update CI step names/commands, update justfile aliases, and update lint-staged hooks. Vite 8 (already in use) ships with Rolldown + Oxc — no bundler changes needed.

**Tech Stack:** pnpm (workspace root), oxlint, oxfmt, stylelint (unchanged), Vite 8 (unchanged), GitHub Actions CI

---

### Task 1: Create `.oxfmtrc.json` config

**Files:**

- Create: `.oxfmtrc.json`
- Create: `.oxfmtignore`

- [ ] **Step 1: Create `.oxfmtignore`**

```bash
touch .oxfmtignore
```

Content — exclude `.cjs` files (oxfmt compat gap) and Cargo.lock:

```
*.cjs
Cargo.lock
```

Write the file:

```bash
cat > .oxfmtignore << 'EOF'
*.cjs
Cargo.lock
EOF
```

- [ ] **Step 2: Create `.oxfmtrc.json`** (zero-config — same as oxfmt defaults)

oxfmt defaults match Prettier defaults. Current `.prettierrc.json` has `semi: true`, `singleQuote: false`, `trailingComma: "none"` — all default. Create a minimal file for explicit visibility:

```json
{
  "$schema": "https://oxc.rs/docs/guide/config/node-api.html",
  "semi": true,
  "singleQuote": false,
  "trailingComma": "none"
}
```

```bash
cat > .oxfmtrc.json << 'EOF'
{
  "$schema": "https://oxc.rs/docs/guide/config/node-api.html",
  "semi": true,
  "singleQuote": false,
  "trailingComma": "none"
}
EOF
```

- [ ] **Step 3: Commit**

```bash
git add .oxfmtrc.json .oxfmtignore
git commit -m "feat(ci): add oxfmt config files"
```

---

### Task 2: Create `.oxlintrc.json` config

**Files:**

- Create: `.oxlintrc.json`

- [ ] **Step 1: Create `.oxlintrc.json`**

Oxlint's built-in rules cover most ESLint rules. We turn off `vue/multi-word-component-names` (matches current ESLint config behavior). Oxlint uses rule names like `eslint/vue/multi-word-component-names`.

```json
{
  "rules": {
    "eslint/vue/multi-word-component-names": "off"
  }
}
```

```bash
cat > .oxlintrc.json << 'EOF'
{
  "rules": {
    "eslint/vue/multi-word-component-names": "off"
  }
}
EOF
```

- [ ] **Step 2: Commit**

```bash
git add .oxlintrc.json
git commit -m "feat(ci): add oxlint config file"
```

---

### Task 3: Update root `package.json` — deps

**Files:**

- Modify: `package.json` (root)

- [ ] **Step 1: Remove ESLint/Prettier devDependencies, add oxlint + oxfmt**

Remove these from `devDependencies`:

- `@eslint/js`
- `eslint`
- `eslint-config-prettier`
- `eslint-plugin-vue`
- `globals`
- `prettier`
- `typescript-eslint`
- `vue-eslint-parser`

Add:

- `oxlint` — `latest` (will install via `pnpm add`)
- `oxfmt` — `latest`

```bash
pnpm add -D oxlint oxfmt
pnpm remove \
  @eslint/js \
  eslint \
  eslint-config-prettier \
  eslint-plugin-vue \
  globals \
  prettier \
  typescript-eslint \
  vue-eslint-parser
```

- [ ] **Step 2: Verify devDependencies look correct**

```bash
node -e "const p=require('./package.json'); console.log(JSON.stringify(p.devDependencies, null, 2))" | cat
```

Expected: `oxlint` and `oxfmt` present, all ESLint/Prettier deps absent. `stylelint` should still be present.

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore(deps): replace eslint + prettier with oxlint + oxfmt"
```

---

### Task 4: Update root `package.json` — scripts + lint-staged

**Files:**

- Modify: `package.json` (root)

- [ ] **Step 1: Replace format:check:web and format:web scripts**

Change `format:check:web` from:

```
prettier --check "{apps,crates,docs,fixtures}/**/*.{ts,tsx,js,jsx,vue,css,scss,sass,less,json,md}" "*.{json,md}"
```

to:

```
npx oxfmt --check .
```

Change `format:web` from:

```
prettier --write "{apps,crates,docs,fixtures}/**/*.{ts,tsx,js,jsx,vue,css,scss,sass,less,json,md}" "*.{json,md}"
```

to:

```
npx oxfmt --write .
```

- [ ] **Step 2: Replace lint scripts**

Remove `lint:eslint`.
Change `lint:web` from `pnpm run lint:eslint && pnpm run lint:style` to:

```
npx oxlint && npx stylelint "apps/agent-gui/src/**/*.{vue,css,scss,sass,less}"
```

Add a convenience alias:

```json
"lint:oxlint": "npx oxlint"
```

- [ ] **Step 3: Replace lint-staged hooks**

Replace the entire `lint-staged` block:

```json
"lint-staged": {
  "*.rs": [
    "sh -c \"cargo fmt --all\""
  ],
  "*.{json,md}": [
    "oxfmt --write"
  ],
  "apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}": [
    "oxfmt --write",
    "oxlint --fix"
  ],
  "apps/agent-gui/src/**/*.{vue,css,scss,sass,less}": [
    "oxfmt --write",
    "stylelint --fix"
  ]
}
```

- [ ] **Step 4: Verify scripts**

```bash
pnpm run lint:web
```

Expected: Pass or show only pre-existing stylelint issues. No ESLint errors (ESLint is removed).

- [ ] **Step 5: Commit**

```bash
git add package.json
git commit -m "refactor: switch scripts and lint-staged to oxlint + oxfmt"
```

---

### Task 5: Remove old config files

**Files:**

- Delete: `eslint.config.js`
- Delete: `.prettierrc.json`
- Delete: `.prettierignore`

- [ ] **Step 1: Delete files**

```bash
rm eslint.config.js .prettierrc.json .prettierignore
```

- [ ] **Step 2: Commit**

```bash
git add eslint.config.js .prettierrc.json .prettierignore
git commit -m "refactor: remove eslint, prettier config files"
```

---

### Task 6: Clean `apps/agent-gui/package.json` — remove Rollup optionalDeps

**Files:**

- Modify: `apps/agent-gui/package.json`

- [ ] **Step 1: Remove the entire `optionalDependencies` block**

Vite 8 uses Rolldown, not Rollup. These `@rollup/rollup-*` optionalDeps are no longer needed.

```bash
pnpm remove --filter agent-gui @rollup/rollup-darwin-arm64 @rollup/rollup-linux-x64-gnu @rollup/rollup-win32-x64-msvc
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/package.json pnpm-lock.yaml
git commit -m "chore(gui): remove unused rollup optionalDependencies"
```

---

### Task 7: Update `.github/workflows/ci.yml`

**Files:**

- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Update format job — `Check formatting` step**

Change:

```yaml
- name: Check formatting
  run: pnpm run format:check
```

to:

```yaml
- name: Check formatting
  run: npx oxfmt --check .
```

(The `format:check` script now calls oxfmt internally via our scripts change, but being explicit avoids dependency on pnpm scripts chain.)

- [ ] **Step 2: Update lint-web job — replace ESLint step**

Change:

```yaml
- name: Run ESLint
  run: pnpm run lint:eslint
```

to:

```yaml
- name: Run Oxlint
  run: npx oxlint
```

The Stylelint step stays unchanged.

- [ ] **Step 3: Update `justfile` → `gen-types` recipe**

The `gen-types` recipe calls `npx prettier --write` on generated files. Replace with oxfmt:

Change:

```
npx prettier --write apps/agent-gui/src/generated/commands.ts apps/agent-gui/src/generated/events.ts
```

to:

```
npx oxfmt --write apps/agent-gui/src/generated/commands.ts apps/agent-gui/src/generated/events.ts
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml justfile
git commit -m "ci: switch CI lint and format checks to oxc toolchain"
```

---

### Task 8: Update justfile — top-level commands

**Files:**

- Modify: `justfile`

- [ ] **Step 1: Update the `lint` recipe**

Current:

```makefile
lint:
    pnpm run lint
```

Keep as is — `pnpm run lint` now calls oxlint+stylelint via our scripts change.

- [ ] **Step 2: Update `changelog` recipe**

Current:

```makefile
changelog tag:
    git cliff --tag {{ tag }} -o CHANGELOG.md && pnpm prettier --write CHANGELOG.md
```

Change to:

```makefile
changelog tag:
    git cliff --tag {{ tag }} -o CHANGELOG.md && npx oxfmt --write CHANGELOG.md
```

- [ ] **Step 3: Update `AGENTS.md` references**

AGENTS.md has Prettier/ESLint references in:

- `TypeScript / Vue` section: "Prettier + ESLint + Stylelint" → "Oxlint + Oxfmt + Stylelint"
- `lint-staged config` section: mentions `prettier --write` + `eslint --fix`
- `Local verification`: `pnpm run lint` (still valid)

- [ ] **Step 4: Commit**

```bash
git add justfile AGENTS.md
git commit -m "docs: update justfile and AGENTS.md for oxc toolchain"
```

---

### Task 9: Run full verification

- [ ] **Step 1: Run format check**

```bash
npx oxfmt --check .
```

Expected: All files pass format check (or show differences from Prettier → oxfmt switch).

- [ ] **Step 2: If format check fails, reformat the entire repo**

```bash
npx oxfmt --write .
```

- [ ] **Step 3: Run lint**

```bash
npx oxlint
npx stylelint "apps/agent-gui/src/**/*.{vue,css,scss,sass,less}"
```

Expected: No oxlint errors (or pre-existing issues), stylelint unchanged.

- [ ] **Step 4: Run full Rust check**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

- [ ] **Step 5: Commit reformat if any**

```bash
git add -A
git commit -m "style: apply oxfmt reformat across entire repo"
```

---

### Task 10: Final clean verification

- [ ] **Step 1: Full gate check**

```bash
npx oxfmt --check . && npx oxlint && npx stylelint "apps/agent-gui/src/**/*.{vue,css,scss,sass,less}" && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-targets
```

Expected: All pass.

- [ ] **Step 2: Verify no ESLint/Prettier remnants**

```bash
grep -r "prettier" package.json justfile .github/workflows/ci.yml AGENTS.md | cat
grep -r "eslint" package.json justfile .github/workflows/ci.yml AGENTS.md | cat
```

Expected: No matches for Prettier/ESLint (except in historical context sections of AGENTS.md like "Style: Prettier + ESLint + Stylelint" which we already updated).

- [ ] **Step 3: Final commit if any loose ends**

```bash
git status | cat
```

If clean, done. If any changes, commit them.
