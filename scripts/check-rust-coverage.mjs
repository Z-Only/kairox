#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const coveragePaths =
  process.argv.length > 2 ? process.argv.slice(2) : ["target/coverage/rust.lcov"];
const workspaceRoot = process.cwd();
const allowPartial = process.env.KAIROX_COVERAGE_ALLOW_PARTIAL === "1";

// LCOV records still carry BRF/BRH lines if grcov is asked for them, but the
// pipeline runs without `--branch` to dodge the llvm-cov SIGSEGV described in
// `scripts/run-rust-coverage.sh` (upstream LLVM bug
// https://github.com/llvm/llvm-project/issues/189169). Drop branches from the
// gated metrics so the gate doesn't pretend to enforce something we no longer
// measure.
const metrics = ["functions", "lines"];

// Coverage thresholds are organised by risk tier rather than by codebase area.
// Each file may be evaluated by multiple groups (for example, a workspace-wide
// floor and a tier-specific gate). Stricter tiers must pass first; relaxed
// tiers act as a safety net against report truncation.
const groups = [
  // Tier 1 — Critical: permission engine, persistence, domain types,
  // configuration loader. Defects here affect audit, security, recoverability.
  {
    name: "T1 critical core",
    include: [
      /^crates\/agent-tools\/src\/(permission|registry)\.rs$/,
      /^crates\/agent-store\/src\//,
      /^crates\/agent-core\/src\//,
      /^crates\/agent-config\/src\//
    ],
    minFiles: 32,
    thresholds: {
      // Latest CI baseline (40 files): functions 36.19%, lines 86.93%.
      // Floors set to floor(actual − 1) for nightly wobble.
      //
      // Extracting inline `#[cfg(test)] mod tests` into `#[path]` sibling
      // *_tests.rs files (registry.rs, projection.rs) removes those test
      // functions from the T1 src-file function counts that previously
      // inflated the ratio. *_tests.rs files are excluded from coverage
      // (see isSourceFile), so the ratio drops to an honest 34.96% with no
      // production-code regression. Floor lowered 35 → 33 to absorb the
      // measurement shift (mirrors the T3 74 → 72 adjustment below).
      functions: 33,
      lines: 85
    }
  },
  // Tier 2A — High-risk runtime hot path.
  {
    name: "T2 high-risk runtime",
    include: [
      /^crates\/agent-runtime\/src\//,
      /^crates\/agent-memory\/src\//,
      /^crates\/agent-models\/src\//,
      /^crates\/agent-mcp\/src\//
    ],
    minFiles: 120,
    thresholds: {
      // Latest CI baseline (138 files): functions 30.64%, lines 81.34%.
      // Floors set to floor(actual − 1); raise as runtime-side tests land.
      functions: 29,
      lines: 80
    }
  },
  // Tier 2B — Tauri IPC boundary. Latest CI baseline: functions 3.17%,
  // lines 18.98% across 19 files. The functions floor is intentionally
  // near-zero — every #[tauri::command] is a thin adapter and
  // unit-testing them requires a real AppHandle (#502 PR discussion).
  // lines is gated to keep regressions visible.
  {
    name: "T2 Tauri IPC",
    include: [
      /^apps\/agent-gui\/src-tauri\/src\/(lib|app_state|event_forwarder|commands)\.rs$/,
      /^apps\/agent-gui\/src-tauri\/src\/commands\//
    ],
    // specta.rs is a Specta registration glue file dominated by macro output.
    exclude: [/^apps\/agent-gui\/src-tauri\/src\/specta\.rs$/],
    minFiles: 13,
    thresholds: {
      functions: 2,
      lines: 18
    }
  },
  // Tier 3 — Medium-risk adapters: built-in tools (shell/fs/patch/search),
  // Skills registry/state, Plugins manifest parsing.
  {
    name: "T3 adapters and skills",
    include: [
      /^crates\/agent-tools\/src\//,
      /^crates\/agent-skills\/src\//,
      /^crates\/agent-plugins\/src\//
    ],
    // T1 covers these with stricter thresholds; do not double-count them here.
    exclude: [/^crates\/agent-tools\/src\/(permission|registry)\.rs$/],
    minFiles: 30,
    thresholds: {
      // Latest CI baseline (33 files): functions 75.06%, lines 93.06%.
      // Floors set to floor(actual − 1).
      // Extracting inline `#[cfg(test)] mod tests` into `#[path]` sibling
      // *_tests.rs files (e.g. agent-skills registry_tests.rs) adds those
      // files to this tier, shifting the function ratio to 73.32% with no
      // production-code regression. Floor lowered 74 → 72 to absorb the
      // measurement shift.
      functions: 72,
      lines: 92
    }
  },
  // Tier 4 — Floor: rendering shells and evaluation CLI. Post-#509 finally
  // sees the agent-tui src tree (82 files), so meaningful floors apply.
  {
    name: "T4 UI shells and eval",
    include: [/^crates\/agent-tui\/src\//, /^crates\/agent-eval\/src\//],
    minFiles: 75,
    thresholds: {
      // Latest CI baseline (99 files): functions 36.40%, lines 63.91%.
      // Floors already tight at floor(actual − 1).
      functions: 35,
      lines: 62
    }
  },
  // Workspace overall — anti-truncation backstop covering every counted file.
  {
    name: "Rust workspace overall",
    include: [/^(crates|apps\/agent-gui\/src-tauri\/src)\//],
    minFiles: 280,
    thresholds: {
      // Latest CI baseline (331 files): functions 32.42%, lines 72.84%.
      // Floors set to floor(actual − 1).
      functions: 31,
      lines: 71
    }
  }
];

function readFile(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`coverage report does not exist: ${filePath}`);
  }

  return fs.readFileSync(filePath, "utf8");
}

function emptySummary() {
  return Object.fromEntries(metrics.map((metric) => [metric, { count: 0, covered: 0 }]));
}

function readLcov(filePath) {
  const files = [];
  let current = null;

  function finishRecord() {
    if (!current) return;
    files.push(current);
    current = null;
  }

  for (const line of readFile(filePath).split(/\r?\n/)) {
    if (line.startsWith("SF:")) {
      finishRecord();
      current = {
        filename: line.slice(3),
        summary: emptySummary()
      };
      continue;
    }

    if (!current) continue;

    // BRF/BRH (branch coverage) records are intentionally not parsed — see
    // the metrics constant above for context (LLVM #189169).
    if (line.startsWith("FNF:")) {
      current.summary.functions.count = Number(line.slice(4));
    } else if (line.startsWith("FNH:")) {
      current.summary.functions.covered = Number(line.slice(4));
    } else if (line.startsWith("LF:")) {
      current.summary.lines.count = Number(line.slice(3));
    } else if (line.startsWith("LH:")) {
      current.summary.lines.covered = Number(line.slice(3));
    } else if (line === "end_of_record") {
      finishRecord();
    }
  }

  finishRecord();
  return files;
}

function readCoverage(filePath) {
  if (filePath.endsWith(".json")) {
    const report = JSON.parse(readFile(filePath));
    return (report.data ?? []).flatMap((entry) => entry.files ?? []);
  }

  return readLcov(filePath);
}

function relativeCoveragePath(filename) {
  if (!path.isAbsolute(filename)) {
    return filename.replace(/^\.\//, "").split(path.sep).join("/");
  }

  const relative = path.relative(workspaceRoot, filename);
  return relative.startsWith("..") ? filename : relative.split(path.sep).join("/");
}

function isSourceFile(file) {
  return (
    file.endsWith(".rs") &&
    !file.includes("/tests/") &&
    !file.includes("/examples/") &&
    !file.includes("/benches/") &&
    !file.endsWith("/tests.rs") &&
    !/[_-]tests\.rs$/.test(file) &&
    !file.endsWith("/build.rs") &&
    !file.includes("/src/bin/")
  );
}

function percentage(covered, count) {
  return count === 0 ? 100 : (covered / count) * 100;
}

function aggregate(files) {
  const totals = Object.fromEntries(metrics.map((metric) => [metric, { count: 0, covered: 0 }]));

  for (const file of files) {
    for (const metric of metrics) {
      const summary = file.summary?.[metric];
      if (!summary) continue;
      totals[metric].count += summary.count ?? 0;
      totals[metric].covered += summary.covered ?? 0;
    }
  }

  return Object.fromEntries(
    metrics.map((metric) => {
      const total = totals[metric];
      return [
        metric,
        {
          ...total,
          percent: percentage(total.covered, total.count)
        }
      ];
    })
  );
}

function mergeDuplicateFiles(files) {
  const byFilename = new Map();

  for (const file of files) {
    const existing = byFilename.get(file.relativePath);
    if (!existing) {
      byFilename.set(file.relativePath, file);
      continue;
    }

    for (const metric of metrics) {
      const currentMetric = file.summary?.[metric];
      const existingMetric = existing.summary?.[metric];
      if (!currentMetric || !existingMetric) continue;

      existingMetric.count = Math.max(existingMetric.count ?? 0, currentMetric.count ?? 0);
      existingMetric.covered = Math.max(existingMetric.covered ?? 0, currentMetric.covered ?? 0);
    }
  }

  return [...byFilename.values()];
}

function formatPercent(value) {
  return `${value.toFixed(2)}%`;
}

function formatThresholds(thresholds) {
  return Object.entries(thresholds)
    .map(([metric, threshold]) => `${metric}>=${threshold}%`)
    .join(", ");
}

const files = mergeDuplicateFiles(
  coveragePaths
    .flatMap((coveragePath) => readCoverage(coveragePath))
    .map((file) => ({
      ...file,
      relativePath: relativeCoveragePath(file.filename)
    }))
    .filter((file) => isSourceFile(file.relativePath))
);

const failures = [];

console.log("Rust coverage thresholds");
console.log("=========================");

for (const group of groups) {
  const matchingFiles = files.filter((file) => {
    if (!group.include.some((pattern) => pattern.test(file.relativePath))) {
      return false;
    }
    if (group.exclude?.some((pattern) => pattern.test(file.relativePath))) {
      return false;
    }
    return true;
  });

  if (matchingFiles.length === 0) {
    const message = `${group.name}: no files matched (${formatThresholds(group.thresholds)})`;
    if (group.allowPartial || allowPartial) {
      console.warn(`WARN ${message}`);
      continue;
    }

    failures.push(message);
    continue;
  }

  const totals = aggregate(matchingFiles);
  const row = metrics
    .map((metric) => `${metric} ${formatPercent(totals[metric].percent)}`)
    .join(" | ");
  console.log(`${group.name}: ${row} (${matchingFiles.length} files)`);

  if (group.minFiles && matchingFiles.length < group.minFiles) {
    failures.push(`${group.name} files: ${matchingFiles.length} < ${group.minFiles}`);
  }

  for (const metric of Object.keys(group.thresholds)) {
    const actual = totals[metric].percent;
    const threshold = group.thresholds[metric];
    if (actual + Number.EPSILON < threshold) {
      failures.push(`${group.name} ${metric}: ${formatPercent(actual)} < ${threshold}%`);
    }
  }
}

if (failures.length > 0) {
  console.error("\nCoverage threshold failures:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("\nRust coverage thresholds satisfied.");
