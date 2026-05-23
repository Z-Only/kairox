#!/usr/bin/env bash
set -euo pipefail

coverage_dir="target/coverage"
profile_dir="target/llvm-cov-target"
grcov_version="${KAIROX_GRCOV_VERSION:-v0.10.7}"
toolchain="${KAIROX_COVERAGE_TOOLCHAIN:-nightly}"
grcov_threads="${KAIROX_GRCOV_THREADS:-1}"

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

run_coverage_group() {
  local name="$1"
  shift

  local output_path="${coverage_dir}/rust-${name}.lcov"

  # cargo-llvm-cov remains the Rust test/instrumentation driver recommended by
  # Rust projects. We intentionally let grcov generate LCOV from profraw files
  # because `cargo llvm-cov --branch --lcov` calls llvm-cov export, which is
  # currently crash-prone for this workspace's async Rust coverage maps.
  cargo "+${toolchain}" llvm-cov clean --workspace
  cargo "+${toolchain}" llvm-cov \
    "$@" \
    --all-targets \
    --branch \
    --no-report

  local profraw_count
  profraw_count="$(find "$profile_dir" -name '*.profraw' | wc -l | tr -d ' ')"
  echo "Generating Rust ${name} LCOV from ${profraw_count} profraw files with $("$grcov" --version)"

  run_grcov "$grcov" "$profile_dir" \
    --binary-path "${profile_dir}/debug/deps" \
    --source-dir . \
    --output-types lcov \
    --branch \
    --threads "$grcov_threads" \
    --llvm-path "$llvm_bin" \
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
    --output-path "$output_path"

  local source_count
  source_count="$(grep -c '^SF:' "$output_path" || true)"
  echo "Generated ${output_path} with ${source_count} source files"
}

grcov="$(install_grcov)"

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

bun scripts/check-rust-coverage.mjs "${coverage_dir}/rust.lcov"
