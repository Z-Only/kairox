#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const coveragePaths =
  process.argv.length > 2 ? process.argv.slice(2) : ["target/coverage/rust.lcov"];
const workspaceRoot = process.cwd();
const allowPartial = process.env.KAIROX_COVERAGE_ALLOW_PARTIAL === "1";

const metrics = ["branches", "functions", "lines"];

const groups = [
  // Rust branch coverage currently depends on nightly LLVM coverage support.
  // Keep thresholds on the reportable LCOV subset and file-count floors to catch
  // obvious report truncation while upstream branch crashes are still open.
  {
    name: "Rust workspace",
    include: [/^(crates|apps\/agent-gui\/src-tauri\/src)\//],
    minFiles: 45,
    thresholds: {
      branches: 70,
      functions: 30,
      lines: 70
    }
  },
  {
    name: "Rust core business",
    include: [/^crates\/agent-(core|runtime|memory|store|config)\//],
    minFiles: 24,
    thresholds: {
      branches: 60,
      functions: 26,
      lines: 72
    }
  },
  {
    name: "Rust tools and integrations",
    include: [/^crates\/agent-(tools|mcp|models|skills|plugins)\//],
    minFiles: 7,
    thresholds: {
      branches: 86,
      functions: 52,
      lines: 88
    }
  },
  {
    name: "Rust UI shells and eval",
    include: [
      /^crates\/agent-tui\//,
      /^crates\/agent-eval\//,
      /^apps\/agent-gui\/src-tauri\/src\//
    ],
    minFiles: 16,
    thresholds: {
      branches: 62,
      functions: 0,
      lines: 0
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
  return metrics.map((metric) => `${metric}>=${thresholds[metric]}%`).join(", ");
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
  const matchingFiles = files.filter((file) =>
    group.include.some((pattern) => pattern.test(file.relativePath))
  );

  if (matchingFiles.length === 0) {
    const message = `${group.name}: no files matched (${formatThresholds(group.thresholds)})`;
    if (allowPartial) {
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

  for (const metric of metrics) {
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
