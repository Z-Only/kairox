import { execFile as execFileCallback } from "node:child_process";
import { existsSync } from "node:fs";
import { basename } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);
const GIT_BUFFER = 10 * 1024 * 1024;
const DIRTY_FILE_LIMIT = 5;

export const USAGE = `Usage: node scripts/audit-eval-worktrees.mjs [--json] [--summary] [--dirty-only|--clean-only] [--compare-ref <ref>]

Audits local eval worktrees without deleting worktrees or branches.

Selection:
  Includes worktrees whose branch starts with "eval/" or whose path basename
  starts with "eval-kairox-".

Options:
  --json        Print a stable JSON object.
  --summary     Only print summary counts.
  --dirty-only  Only show dirty, missing, or error worktrees.
  --clean-only  Only show clean worktrees.
  --compare-ref Compare dirty files with a ref and annotate matching content.
  --help, -h    Show this help.
`;

class UsageError extends Error {}

function normalizeBranch(ref) {
  if (!ref) {
    return null;
  }
  if (ref.startsWith("refs/heads/")) {
    return ref.slice("refs/heads/".length);
  }
  return ref;
}

function pushWorktree(worktrees, current) {
  if (!current?.path) {
    return;
  }
  worktrees.push({
    path: current.path,
    branch: current.branch ?? null,
    head: current.head ?? null
  });
}

function parseDirtyStatusPath(line) {
  const path = line.length > 3 ? line.slice(3) : line.trim();
  const renameSeparator = " -> ";
  if (path.includes(renameSeparator)) {
    return path.slice(path.lastIndexOf(renameSeparator) + renameSeparator.length).trim();
  }
  return path.trim();
}

function parseDirtyStatusPaths(output) {
  return output
    .split(/\r?\n/)
    .filter((line) => line.trim() !== "")
    .map(parseDirtyStatusPath);
}

function summarizeDirtyFiles(dirtyFiles) {
  return {
    dirty_file_count: dirtyFiles.length,
    dirty_files: dirtyFiles.slice(0, DIRTY_FILE_LIMIT)
  };
}

function summarizeDirtyStatus(output) {
  return summarizeDirtyFiles(parseDirtyStatusPaths(output));
}

export function parseWorktreePorcelain(output) {
  const worktrees = [];
  let current = null;

  for (const line of output.split(/\r?\n/)) {
    if (line === "") {
      pushWorktree(worktrees, current);
      current = null;
      continue;
    }

    if (line.startsWith("worktree ")) {
      pushWorktree(worktrees, current);
      current = {
        path: line.slice("worktree ".length),
        branch: null,
        head: null
      };
      continue;
    }

    if (!current) {
      continue;
    }
    if (line.startsWith("HEAD ")) {
      current.head = line.slice("HEAD ".length);
      continue;
    }
    if (line.startsWith("branch ")) {
      current.branch = normalizeBranch(line.slice("branch ".length));
      continue;
    }
    if (line === "detached") {
      current.branch = null;
    }
  }

  pushWorktree(worktrees, current);
  return worktrees;
}

export function filterEvalWorktrees(worktrees) {
  return worktrees.filter((worktree) => {
    return (
      worktree.branch?.startsWith("eval/") === true ||
      basename(worktree.path).startsWith("eval-kairox-")
    );
  });
}

async function compareDirtyFilesToRef(worktreePath, dirtyFiles, compareRef, { execFile, env }) {
  let checkedCount = 0;
  const matchingFiles = [];

  for (const dirtyFile of dirtyFiles) {
    try {
      const refResult = await execFile(
        "git",
        ["-C", worktreePath, "rev-parse", `${compareRef}:${dirtyFile}`],
        {
          env,
          maxBuffer: GIT_BUFFER
        }
      );
      const worktreeResult = await execFile(
        "git",
        ["-C", worktreePath, "hash-object", "--", dirtyFile],
        {
          env,
          maxBuffer: GIT_BUFFER
        }
      );
      checkedCount += 1;
      if (refResult.stdout.trim() === worktreeResult.stdout.trim()) {
        matchingFiles.push(dirtyFile);
      }
    } catch {
      // Untracked, deleted, or absent-in-ref files are not comparable.
    }
  }

  return {
    compare_ref: compareRef,
    compare_ref_checked_count: checkedCount,
    compare_ref_match_count: matchingFiles.length,
    compare_ref_matching_files: matchingFiles.slice(0, DIRTY_FILE_LIMIT)
  };
}

async function dirtyStatus(worktreePath, { execFile, env, pathExists, compareRef }) {
  const exists = pathExists(worktreePath);
  if (!exists) {
    return {
      dirty_status: "missing",
      path_exists: false,
      dirty_file_count: 0,
      dirty_files: []
    };
  }

  try {
    const result = await execFile("git", ["-C", worktreePath, "status", "--short"], {
      env,
      maxBuffer: GIT_BUFFER
    });
    const dirtyFiles = parseDirtyStatusPaths(result.stdout);
    const dirtySummary = summarizeDirtyFiles(dirtyFiles);
    const compareSummary =
      compareRef && dirtyFiles.length > 0
        ? await compareDirtyFilesToRef(worktreePath, dirtyFiles, compareRef, { execFile, env })
        : {};
    return {
      dirty_status: dirtySummary.dirty_file_count === 0 ? "clean" : "dirty",
      path_exists: true,
      ...dirtySummary,
      ...compareSummary
    };
  } catch {
    return {
      dirty_status: "error",
      path_exists: true,
      dirty_file_count: 0,
      dirty_files: []
    };
  }
}

export async function auditEvalWorktrees({
  execFile = execFileAsync,
  pathExists = existsSync,
  cwd = process.cwd(),
  env = process.env,
  compareRef = null
} = {}) {
  const result = await execFile("git", ["worktree", "list", "--porcelain"], {
    cwd,
    env,
    maxBuffer: GIT_BUFFER
  });
  const selected = filterEvalWorktrees(parseWorktreePorcelain(result.stdout));
  const audited = [];

  for (const worktree of selected) {
    audited.push({
      path: worktree.path,
      branch: worktree.branch,
      head: worktree.head,
      ...(await dirtyStatus(worktree.path, { execFile, env, pathExists, compareRef }))
    });
  }

  return audited;
}

export function summarizeAudit(worktrees) {
  const summary = {
    total: worktrees.length,
    clean: 0,
    dirty: 0,
    missing: 0,
    error: 0
  };

  for (const worktree of worktrees) {
    if (Object.hasOwn(summary, worktree.dirty_status)) {
      summary[worktree.dirty_status] += 1;
    }
  }

  return summary;
}

export function filterAuditResults(worktrees, { dirtyOnly = false, cleanOnly = false } = {}) {
  if (dirtyOnly) {
    return worktrees.filter((worktree) =>
      ["dirty", "missing", "error"].includes(worktree.dirty_status)
    );
  }
  if (cleanOnly) {
    return worktrees.filter((worktree) => worktree.dirty_status === "clean");
  }
  return worktrees;
}

function shortHead(head) {
  return head ? head.slice(0, 12) : "-";
}

function pad(value, width) {
  return String(value).padEnd(width, " ");
}

function formatDirtyFiles(worktree) {
  const dirtyFiles = Array.isArray(worktree.dirty_files) ? worktree.dirty_files : [];
  if (dirtyFiles.length === 0) {
    return "-";
  }

  const dirtyFileCount = worktree.dirty_file_count ?? dirtyFiles.length;
  const remaining = dirtyFileCount - dirtyFiles.length;
  const suffix = remaining > 0 ? `, +${remaining} more` : "";
  return `${dirtyFileCount}: ${dirtyFiles.join(", ")}${suffix}`;
}

function formatCompareRef(worktree) {
  if (!worktree.compare_ref) {
    return "-";
  }

  const matchingFiles = Array.isArray(worktree.compare_ref_matching_files)
    ? worktree.compare_ref_matching_files
    : [];
  const matchCount = worktree.compare_ref_match_count ?? matchingFiles.length;
  const checkedCount = worktree.compare_ref_checked_count ?? 0;
  const remaining = matchCount - matchingFiles.length;
  const suffix = remaining > 0 ? `, +${remaining} more` : "";
  const files = matchingFiles.length > 0 ? `: ${matchingFiles.join(", ")}${suffix}` : "";
  return `${worktree.compare_ref} ${matchCount}/${checkedCount}${files}`;
}

export function formatHumanTable(worktrees) {
  const summary = summarizeAudit(worktrees);
  const summaryLine = `${formatSummaryLine(summary)}\n`;

  if (worktrees.length === 0) {
    return `${summaryLine}No eval worktrees found.\n`;
  }

  const includeCompareRef = worktrees.some((worktree) => worktree.compare_ref);
  const headers = [
    "PATH",
    "BRANCH",
    "HEAD",
    "PATH_EXISTS",
    "DIRTY_STATUS",
    "DIRTY_FILES",
    ...(includeCompareRef ? ["COMPARE_REF_MATCHES"] : [])
  ];
  const rows = worktrees.map((worktree) => {
    const row = [
      worktree.path,
      worktree.branch ?? "-",
      shortHead(worktree.head),
      worktree.path_exists ? "yes" : "no",
      worktree.dirty_status,
      formatDirtyFiles(worktree)
    ];
    if (includeCompareRef) {
      row.push(formatCompareRef(worktree));
    }
    return row;
  });
  const widths = headers.map((header, index) =>
    Math.max(header.length, ...rows.map((row) => String(row[index]).length))
  );
  const formatRow = (row) => row.map((value, index) => pad(value, widths[index])).join("  ");
  const separator = widths.map((width) => "-".repeat(width)).join("  ");

  return `${summaryLine}${[formatRow(headers), separator, ...rows.map(formatRow)].join("\n")}\n`;
}

export function formatSummaryLine(summary) {
  return `Summary: total=${summary.total} clean=${summary.clean} dirty=${summary.dirty} missing=${summary.missing} error=${summary.error}`;
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    json: false,
    summaryOnly: false,
    dirtyOnly: false,
    cleanOnly: false,
    compareRef: null
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      continue;
    }
    if (arg === "--json") {
      parsed.json = true;
      continue;
    }
    if (arg === "--summary") {
      parsed.summaryOnly = true;
      continue;
    }
    if (arg === "--dirty-only") {
      parsed.dirtyOnly = true;
      continue;
    }
    if (arg === "--clean-only") {
      parsed.cleanOnly = true;
      continue;
    }
    if (arg === "--compare-ref") {
      const compareRef = argv[index + 1];
      if (!compareRef || compareRef.startsWith("--")) {
        throw new UsageError("--compare-ref requires a ref");
      }
      parsed.compareRef = compareRef;
      index += 1;
      continue;
    }
    throw new UsageError(`Unknown argument: ${arg}`);
  }

  if (parsed.dirtyOnly && parsed.cleanOnly) {
    throw new UsageError("--dirty-only and --clean-only cannot be used together");
  }

  return parsed;
}

export async function runCli(
  argv = process.argv.slice(2),
  {
    stdout = process.stdout,
    stderr = process.stderr,
    execFile = execFileAsync,
    pathExists = existsSync,
    cwd = process.cwd(),
    env = process.env
  } = {}
) {
  try {
    const args = parseArgs(argv);
    if (args.help) {
      stdout.write(USAGE);
      return 0;
    }

    const audited = filterAuditResults(
      await auditEvalWorktrees({ execFile, pathExists, cwd, env, compareRef: args.compareRef }),
      args
    );
    const summary = summarizeAudit(audited);
    const output = args.summaryOnly
      ? { summary }
      : {
          summary,
          worktrees: audited
        };

    if (args.json) {
      stdout.write(`${JSON.stringify(output, null, 2)}\n`);
    } else if (args.summaryOnly) {
      stdout.write(`${formatSummaryLine(summary)}\n`);
    } else {
      stdout.write(formatHumanTable(audited));
    }
    return 0;
  } catch (error) {
    const usage = error instanceof UsageError ? `\n\n${USAGE}` : "";
    stderr.write(`Error: ${error.message}${usage}\n`);
    return 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await runCli();
}
