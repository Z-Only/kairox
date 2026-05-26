#!/usr/bin/env bash
set -euo pipefail

coverage_dir="target/coverage"
toolchain="${KAIROX_COVERAGE_TOOLCHAIN:-nightly}"

if [ "$(uname -s)" = "Linux" ] && [ -z "${CC:-}" ] && command -v clang >/dev/null 2>&1; then
  export CC=clang
fi

if [ "$(uname -s)" = "Linux" ] && [ -z "${CXX:-}" ] && command -v clang++ >/dev/null 2>&1; then
  export CXX=clang++
fi

mkdir -p "$coverage_dir"
rm -f "$coverage_dir"/rust*.lcov

# Workspace files we never want to count toward coverage. See agent-tools/tests
# (integration tests), benches, examples, build.rs, src/bin (binary entry
# points), and the auto-generated Tauri gen/ tree. cargo-llvm-cov passes the
# expression to llvm-cov export's --ignore-filename-regex, which matches on
# the absolute source path that ends up in LCOV SF: records.
ignore_filename_regex='(/tests/|/benches/|/examples/|/src/bin/|apps/agent-gui/src-tauri/gen/|build\.rs$)'

# Diagnostic helper: group every SF: record in an LCOV file by its top-level
# workspace path (crate or app). Originally added in the grcov-based pipeline
# (#503) to investigate why agent-runtime/memory/models/mcp/tools/tui/config
# never appeared in LCOV; kept here so future regressions stay easy to spot.
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

# Generate LCOV for one logical workspace group (core / tools / ui). We rely
# on cargo-llvm-cov's built-in `--lcov` exporter, which goes through
# `llvm-cov export` against every .rlib/.o/binary in the instrumented build,
# so dependency crates within the workspace appear in the report. The earlier
# grcov-based pipeline only saw symbols whose debug info reached final test
# binaries, which dropped half the workspace (#503 diagnosis).
run_coverage_group() {
  local name="$1"
  shift

  local output_path="${coverage_dir}/rust-${name}.lcov"

  cargo "+${toolchain}" llvm-cov clean --workspace
  cargo "+${toolchain}" llvm-cov \
    "$@" \
    --all-targets \
    --branch \
    --ignore-filename-regex "$ignore_filename_regex" \
    --lcov \
    --output-path "$output_path"

  local source_count
  source_count="$(grep -c '^SF:' "$output_path" || true)"
  echo "Generated ${output_path} with ${source_count} source files"
  print_lcov_breakdown "$output_path"
}

run_coverage_group core \
  -p agent-core \
  -p agent-runtime \
  -p agent-memory \
  -p agent-store \
  -p agent-config

run_coverage_group tools \
  -p agent-tools \
  -p agent-mcp \
  -p agent-models \
  -p agent-skills \
  -p agent-plugins

run_coverage_group ui \
  -p agent-tui \
  -p agent-eval \
  -p agent-gui-tauri

cat \
  "${coverage_dir}/rust-core.lcov" \
  "${coverage_dir}/rust-tools.lcov" \
  "${coverage_dir}/rust-ui.lcov" \
  >"${coverage_dir}/rust.lcov"

print_lcov_breakdown "${coverage_dir}/rust.lcov"

bun scripts/check-rust-coverage.mjs "${coverage_dir}/rust.lcov"
