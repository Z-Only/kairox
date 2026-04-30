#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <version-without-v>"
  exit 1
fi

VERSION="$1"
TAG="v${VERSION}"
ROOT="$(git rev-parse --show-toplevel)"

cd "$ROOT"

echo "[1/7] Running checks"
npm run format:check
npm run lint
cargo test --workspace --all-targets

echo "[2/7] Verifying GUI build"
cd "$ROOT/apps/agent-gui"
npm run build
npm run tauri:build
cd "$ROOT"

echo "[3/7] Generating CHANGELOG.md with git-cliff"
if command -v git-cliff &>/dev/null; then
  git cliff --tag "$TAG" -o CHANGELOG.md
else
  echo "⚠️  git-cliff not found. Install it: cargo install git-cliff"
  echo "   Skipping CHANGELOG.md generation. CI will still generate Release Notes."
fi

echo "[4/7] Committing CHANGELOG.md"
git add CHANGELOG.md
git diff --cached --quiet CHANGELOG.md || git commit -m "chore(release): update CHANGELOG for $TAG"

echo "[5/7] Creating or updating tag ${TAG}"
git tag -fa "$TAG" -m "$TAG"

echo "[6/7] Pushing main and tag ${TAG}"
git push origin main
git push origin "$TAG" -f

echo "[7/7] Done. Monitor GitHub Actions for CI / Release workflows."
