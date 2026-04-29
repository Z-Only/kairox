#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <version-without-v>"
  exit 1
fi

VERSION="$1"
TAG="v${VERSION}"
ROOT="/Users/chanyu/AIProjects/kairox"

cd "$ROOT"

echo "[1/6] Running checks"
npm run format:check
npm run lint
cargo test --workspace --all-targets

echo "[2/6] Verifying GUI build"
cd "$ROOT/apps/agent-gui"
npm run build
npm run tauri:build
cd "$ROOT"

echo "[3/6] Creating or updating tag ${TAG}"
git tag -fa "$TAG" -m "$TAG"

echo "[4/6] Pushing main"
git push origin main

echo "[5/6] Pushing tag ${TAG}"
git push origin "$TAG" -f

echo "[6/6] Done. Monitor GitHub Actions for CI / Release workflows."
