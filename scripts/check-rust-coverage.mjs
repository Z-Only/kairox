#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const coveragePaths =
  process.argv.length > 2 ? process.argv.slice(2) : ["target/coverage/rust.lcov"];
const workspaceRoot = process.cwd();
const allowPartial = process.env.KAIROX_COVERAGE_ALLOW_PARTIAL === "1";

const metrics = ["branches", "functions", "lines"];

// Coverage thresholds are organised by risk tier rather than by codebase area.
// Each file may be evaluated by multiple groups (for example, a workspace-wide
// floor and a tier-specific gate). Stricter tiers must pass first; relaxed
// tiers act as a safety net against report truncation.
//
// Calibration note: the cargo-llvm-cov + grcov pipeline currently does not
// surface per-file LCOV records for several Rust crates (notably runtime, memory,
// models, mcp) and reports functions/lines as 0% for the Tauri IPC and TUI
// surfaces. Threshold floors below reflect what CI actually measures today and
// will be tightened as the LCOV pipeline is fixed in a follow-up. See AGENTS.md
// "Coverage gates".
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
    minFiles: 24,
    thresholds: {
      // Both branches and lines kept conservative: nightly llvm-cov coverage
      // numbers wobble by up to ~3pp between runs on this workspace; T1 lines
      // measured 74.26 / 73.87 / 72.75 across consecutive runs.
      branches: 60,
      functions: 33,
      lines: 71
    }
  },
  // Tier 2A — High-risk runtime hot path. CI LCOV currently does not report
  // these source files; the group is kept for documentation and to alert when
  // the LCOV pipeline starts surfacing them. allowPartial keeps it from
  // blocking CI in the meantime.
  {
    name: "T2 high-risk runtime",
    include: [
      /^crates\/agent-runtime\/src\//,
      /^crates\/agent-memory\/src\//,
      /^crates\/agent-models\/src\//,
      /^crates\/agent-mcp\/src\//
    ],
    allowPartial: true,
    thresholds: {
      branches: 70,
      functions: 55,
      lines: 75
    }
  },
  // Tier 2B — Tauri IPC boundary. Errors here block the desktop GUI. CI LCOV
  // reports functions/lines as 0% for these files, so only branches is gated
  // until the pipeline is fixed.
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
      branches: 99
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
    minFiles: 7,
    thresholds: {
      branches: 93,
      functions: 91,
      lines: 95
    }
  },
  // Tier 4 — Floor: rendering shells and evaluation CLI. CI LCOV reports
  // functions/lines as 0% for these surfaces, so only minFiles is gated.
  {
    name: "T4 UI shells and eval",
    include: [/^crates\/agent-tui\/src\//, /^crates\/agent-eval\/src\//],
    minFiles: 1,
    thresholds: {}
  },
  // Workspace overall — anti-truncation backstop covering every counted file.
  {
    name: "Rust workspace overall",
    include: [/^(crates|apps\/agent-gui\/src-tauri\/src)\//],
    minFiles: 50,
    thresholds: {
      // branches kept at 76 (~3pp below 79.69 baseline) for nightly wobble.
      branches: 76,
      functions: 37,
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

    if (line.startsWith("BRF:")) {
      current.summary.branches.count = Number(line.slice(4));
    } else if (line.startsWith("BRH:")) {
      current.summary.branches.covered = Number(line.slice(4));
    } else if (line.startsWith("FNF:")) {
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
