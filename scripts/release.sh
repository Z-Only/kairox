#!/usr/bin/env bash
set -euo pipefail

# ─── Defaults ───
DRY_RUN=false
SKIP_CHECKS=false
SKIP_BUILD=false
PRERELEASE=false

# ─── Parse args ───
while [[ $# -gt 0 ]]; do
  case $1 in
    --dry-run)      DRY_RUN=true; shift ;;
    --skip-checks)  SKIP_CHECKS=true; shift ;;
    --skip-build)   SKIP_BUILD=true; shift ;;
    --prerelease)   PRERELEASE=true; shift ;;
    -*)             echo "Unknown option: $1"; exit 1 ;;
    *)              VERSION="$1"; shift ;;
  esac
done

if [[ -z "${VERSION:-}" ]]; then
  echo "Usage: $0 <version> [--dry-run] [--skip-checks] [--skip-build] [--prerelease]"
  echo ""
  echo "Options:"
  echo "  --dry-run       Print commands without executing"
  echo "  --skip-checks   Skip format check, lint, and test"
  echo "  --skip-build    Skip GUI build verification"
  echo "  --prerelease    Mark as a prerelease (requires manual GitHub mark)"
  exit 1
fi

TAG="v${VERSION}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# ─── Helper ───
run() {
  if [[ "$DRY_RUN" == true ]]; then
    echo "[DRY RUN] $*"
  else
    "$@"
  fi
}

# ─── Step 1: Checks ───
if [[ "$SKIP_CHECKS" == false ]]; then
  echo "[1/7] Running checks"
  run pnpm run format:check
  run pnpm run lint
  run cargo test --workspace --all-targets
else
  echo "[1/7] Skipping checks (--skip-checks)"
fi

# ─── Step 2: GUI build verification ───
if [[ "$SKIP_BUILD" == false ]]; then
  echo "[2/7] Verifying GUI build"
  run bash -c 'cd "$ROOT/apps/agent-gui" && pnpm run build && pnpm run tauri:build'
else
  echo "[2/7] Skipping GUI build (--skip-build)"
fi

# ─── Step 3: Changelog ───
echo "[3/7] Generating CHANGELOG.md"
if command -v git-cliff &>/dev/null; then
  run git cliff --tag "$TAG" -o CHANGELOG.md
  run pnpm exec prettier --write CHANGELOG.md
else
  echo "⚠️  git-cliff not found. Install it: cargo install git-cliff"
  echo "   Skipping CHANGELOG.md generation. CI will still generate Release Notes."
fi

# ─── Step 4: Commit changelog ───
echo "[4/7] Committing CHANGELOG.md"
run git add CHANGELOG.md
if ! git diff --cached --quiet CHANGELOG.md; then
  run git commit -m "chore(release): update CHANGELOG for $TAG"
else
  echo "  No changelog changes to commit"
fi

# ─── Step 5: Tag ───
echo "[5/7] Creating or updating tag ${TAG}"
run git tag -fa "$TAG" -m "$TAG"

# ─── Step 6: Push ───
echo "[6/7] Pushing main and tag ${TAG}"
run git push origin main
run git push origin "$TAG" -f

# ─── Step 7: Done ───
echo "[7/7] Done. Monitor GitHub Actions for CI / Release workflows."
if [[ "$PRERELEASE" == true ]]; then
  echo "📌 This is a prerelease. Mark the GitHub Release as pre-release manually or via CI."
fi
