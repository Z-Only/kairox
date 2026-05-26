#!/usr/bin/env bash
set -euo pipefail

coverage_dir="target/coverage"
profile_dir="target/llvm-cov-target"
grcov_version="${KAIROX_GRCOV_VERSION:-v0.10.7}"
toolchain="${KAIROX_COVERAGE_TOOLCHAIN:-nightly}"
grcov_threads="${KAIROX_GRCOV_THREADS:-1}"
grcov_log_level="${KAIROX_GRCOV_LOG_LEVEL:-INFO}"

if [ "$(uname -s)" = "Linux" ] && [ -z "${CC:-}" ] && command -v clang >/dev/null 2>&1; then
  export CC=clang
fi

if [ "$(uname -s)" = "Linux" ] && [ -z "${CXX:-}" ] && command -v clang++ >/dev/null 2>&1; then
  export CXX=clang++
fi

mkdir -p "$coverage_dir"
rm -f "$coverage_dir"/rust*.lcov

host_triple="$(rustc "+${toolchain}" -vV | awk '/^host:/ { print $2 }')"
rust_sysroot="$(rustc "+${toolchain}" --print sysroot)"
llvm_bin="${rust_sysroot}/lib/rustlib/${host_triple}/bin"

install_grcov() {
  if command -v grcov >/dev/null 2>&1; then
    command -v grcov
    return
  fi

  local cached_grcov="${coverage_dir}/bin/grcov"
  if [ -x "$cached_grcov" ]; then
    printf '%s\n' "$cached_grcov"
    return
  fi

  local os arch asset tmpdir
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}:${arch}" in
    Darwin:arm64) asset="grcov-aarch64-apple-darwin.tar.bz2" ;;
    Darwin:x86_64) asset="grcov-x86_64-apple-darwin.tar.bz2" ;;
    Linux:aarch64) asset="grcov-aarch64-unknown-linux-gnu.tar.bz2" ;;
    Linux:x86_64) asset="grcov-x86_64-unknown-linux-gnu.tar.bz2" ;;
    *)
      echo "Unsupported platform for automatic grcov install: ${os}/${arch}" >&2
      exit 1
      ;;
  esac

  mkdir -p "${coverage_dir}/bin"
  tmpdir="$(mktemp -d)"
  curl -L --retry 3 --fail \
    "https://github.com/mozilla/grcov/releases/download/${grcov_version}/${asset}" \
    | tar -xjf - -C "$tmpdir"
  install -m 0755 "${tmpdir}/grcov" "$cached_grcov"
  rm -rf "$tmpdir"
  printf '%s\n' "$cached_grcov"
}

run_grcov() {
  local grcov="$1"
  shift

  if "$grcov" "$@"; then
    return
  fi

  local status=$?
  if [ "$status" -ne 137 ] && [ "$status" -ne 139 ]; then
    return "$status"
  fi

  echo "grcov exited with ${status}; waiting briefly before one retry" >&2
  sleep 5
  "$grcov" "$@"
}

# Diagnostic helper: group every SF: record in an LCOV file by its top-level
# workspace path (crate or app). Originally added (#503) to investigate why
# agent-runtime/memory/models/mcp/tools/tui/config never appeared in LCOV;
# kept here so future regressions stay easy to spot.
print_lcov_breakdown() {
  local lcov_path="$1"
  if [ ! -f "$lcov_path" ]; then
    return
  fi
  local sf_count
  sf_count="$(grep -c '^SF:' "$lcov_path" 2>/dev/null || echo 0)"
  echo "[diag] LCOV breakdown for ${lcov_path} (${sf_count} SF records):"
  if [ "${sf_count}" -eq 0 ]; then
    echo "[diag]   (empty)"
    return
  fi
  grep '^SF:' "$lcov_path" \
    | sed -E 's|.*/(crates/[^/]+)/.*|\1|; s|.*/(apps/agent-gui/src-tauri)/.*|\1|' \
    | sort | uniq -c | sort -rn \
    | sed 's/^/[diag]   /'
}

grcov="$(install_grcov)"

# Run instrumented tests for the entire workspace in one cargo invocation.
# Running every -p in one pass keeps every test binary on disk while grcov
# walks them, restoring source-map coverage for dep crates that the prior
# core/tools/ui split clobbered between groups.
#
# `--branch` is intentionally absent. Branch coverage on this workspace
# triggers SIGSEGV in `llvm::coverage::CoverageMapping::getInstantiationGroups`
# inside llvm-cov when async test binaries are processed — the upstream LLVM
# bug tracked at https://github.com/llvm/llvm-project/issues/189169 (still
# open). grcov fans `llvm-cov export` out per-binary, so each crash quietly
# drops one binary's source records; the cumulative result was the 52-file /
# 7-crates-missing LCOV diagnosed in #503/#506. Running without `--branch`
# routes around the crashing code path entirely. We still get function and
# line coverage; the workspace's branches gate is removed in
# `scripts/check-rust-coverage.mjs` to match.
#
# We also avoid `cargo llvm-cov --lcov` because that path calls llvm-cov
# export over every object at once and the SIGSEGV becomes terminal there
# (see PR #505 close comment). Profraw → grcov → lcov stays.
cargo "+${toolchain}" llvm-cov clean --workspace
cargo "+${toolchain}" llvm-cov \
  -p agent-core \
  -p agent-runtime \
  -p agent-memory \
  -p agent-store \
  -p agent-config \
  -p agent-tools \
  -p agent-mcp \
  -p agent-models \
  -p agent-skills \
  -p agent-plugins \
  -p agent-tui \
  -p agent-eval \
  -p agent-gui-tauri \
  --all-targets \
  --no-report

profraw_count="$(find "$profile_dir" -name '*.profraw' | wc -l | tr -d ' ')"
echo "Generating Rust LCOV from ${profraw_count} profraw files with $("$grcov" --version)"

run_grcov "$grcov" "$profile_dir" \
  --binary-path "${profile_dir}/debug/deps" \
  --source-dir . \
  --output-types lcov \
  --threads "$grcov_threads" \
  --llvm-path "$llvm_bin" \
  --log-level "$grcov_log_level" \
  --ignore-not-existing \
  --ignore 'target/*' \
  --ignore '*/tests/*' \
  --ignore '*/benches/*' \
  --ignore '*/examples/*' \
  --ignore '*/build.rs' \
  --ignore '*/src/bin/*' \
  --ignore 'apps/agent-gui/src-tauri/gen/*' \
  --keep-only 'crates/*/src/*' \
  --keep-only 'apps/agent-gui/src-tauri/src/*' \
  --output-path "${coverage_dir}/rust.lcov"

source_count="$(grep -c '^SF:' "${coverage_dir}/rust.lcov" || true)"
echo "Generated ${coverage_dir}/rust.lcov with ${source_count} source files"
print_lcov_breakdown "${coverage_dir}/rust.lcov"

bun scripts/check-rust-coverage.mjs "${coverage_dir}/rust.lcov"
