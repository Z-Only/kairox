import { execFile as execFileCallback } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, join, relative } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);
const GIT_BUFFER = 10 * 1024 * 1024;
const DIRTY_FILE_LIMIT = 5;
const DIAGNOSTICS_SIGNAL_FILE_LIMIT = 5;

export const USAGE = `Usage: node scripts/audit-eval-worktrees.mjs [--json] [--summary] [--dirty-only|--clean-only] [--compare-ref <ref>] [--all-files] [--recommend-cleanup] [--fail-on-suspicious-no-tool]

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
  --all-files   Print every dirty and compare-ref file instead of the first five.
  --recommend-cleanup Annotate each worktree with remove/prune/keep/inspect guidance and safe cleanup command previews.
  --fail-on-suspicious-no-tool Exit 2 when session diagnostics report suspicious no-tool completion.
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

function limitFiles(files, fileLimit) {
  return fileLimit === null ? files : files.slice(0, fileLimit);
}

function summarizeDirtyFiles(dirtyFiles, fileLimit = DIRTY_FILE_LIMIT) {
  const summary = {
    dirty_file_count: dirtyFiles.length,
    dirty_files: limitFiles(dirtyFiles, fileLimit)
  };
  if (dirtyFiles.length > 0) {
    summary.dirty_scope = dirtyFiles.every(isDiagnosticsFile) ? "diagnostics_only" : "code";
  }
  return summary;
}

function isDiagnosticsFile(path) {
  return path === ".kairox-eval" || path === ".kairox-eval/" || path.startsWith(".kairox-eval/");
}

function* walkFiles(root) {
  let entries = [];
  try {
    entries = readdirSync(root, { withFileTypes: true });
  } catch {
    return;
  }

  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      yield* walkFiles(path);
    } else if (entry.isFile()) {
      yield path;
    }
  }
}

function summarizeDiagnosticsSignals(worktreePath, dirtyFiles) {
  if (!dirtyFiles.some(isDiagnosticsFile)) {
    return {};
  }

  const diagnosticsRoot = join(worktreePath, ".kairox-eval");
  if (!existsSync(diagnosticsRoot)) {
    return {};
  }

  let suspiciousCount = 0;
  const suspiciousFiles = [];
  for (const file of walkFiles(diagnosticsRoot)) {
    if (!file.endsWith(".json")) {
      continue;
    }
    try {
      const diagnostics = JSON.parse(readFileSync(file, "utf8"));
      if (diagnostics?.suspicious_no_tool_completion === true) {
        suspiciousCount += 1;
        if (suspiciousFiles.length < DIAGNOSTICS_SIGNAL_FILE_LIMIT) {
          suspiciousFiles.push(relative(worktreePath, file).replaceAll("\\", "/"));
        }
      }
    } catch {
      // Ignore partial or non-diagnostics JSON files under .kairox-eval.
    }
  }

  return suspiciousCount === 0
    ? {}
    : {
        suspicious_no_tool_completion_count: suspiciousCount,
        suspicious_no_tool_completion_files: suspiciousFiles
      };
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

async function compareDirtyFilesToRef(
  worktreePath,
  dirtyFiles,
  compareRef,
  { execFile, env, fileLimit }
) {
  let checkedCount = 0;
  const matchingFiles = [];
  const unmatchedFiles = [];

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
      } else {
        unmatchedFiles.push(dirtyFile);
      }
    } catch {
      // Untracked, deleted, or absent-in-ref files are not comparable.
      unmatchedFiles.push(dirtyFile);
    }
  }

  return {
    compare_ref: compareRef,
    compare_ref_checked_count: checkedCount,
    compare_ref_match_count: matchingFiles.length,
    compare_ref_matching_files: limitFiles(matchingFiles, fileLimit),
    compare_ref_unmatched_count: unmatchedFiles.length,
    compare_ref_unmatched_files: limitFiles(unmatchedFiles, fileLimit)
  };
}

async function compareHeadToRef(cwd, head, compareRef, { execFile, env }) {
  if (!compareRef || !head) {
    return {};
  }

  try {
    await execFile("git", ["-C", cwd, "merge-base", "--is-ancestor", head, compareRef], {
      env,
      maxBuffer: GIT_BUFFER
    });
    return {
      compare_ref: compareRef,
      head_in_compare_ref: true
    };
  } catch (error) {
    return {
      compare_ref: compareRef,
      head_in_compare_ref: error?.code === 1 ? false : null
    };
  }
}

async function dirtyStatus(worktreePath, { execFile, env, pathExists, compareRef, fileLimit }) {
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
    const dirtySummary = summarizeDirtyFiles(dirtyFiles, fileLimit);
    const diagnosticsSummary = summarizeDiagnosticsSignals(worktreePath, dirtyFiles);
    const compareSummary =
      compareRef && dirtyFiles.length > 0
        ? await compareDirtyFilesToRef(worktreePath, dirtyFiles, compareRef, {
            execFile,
            env,
            fileLimit
          })
        : {};
    return {
      dirty_status: dirtySummary.dirty_file_count === 0 ? "clean" : "dirty",
      path_exists: true,
      ...dirtySummary,
      ...diagnosticsSummary,
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
  compareRef = null,
  fileLimit = DIRTY_FILE_LIMIT
} = {}) {
  const result = await execFile("git", ["worktree", "list", "--porcelain"], {
    cwd,
    env,
    maxBuffer: GIT_BUFFER
  });
  const selected = filterEvalWorktrees(parseWorktreePorcelain(result.stdout));
  const audited = [];

  for (const worktree of selected) {
    const headSummary = await compareHeadToRef(cwd, worktree.head, compareRef, { execFile, env });
    audited.push({
      path: worktree.path,
      branch: worktree.branch,
      head: worktree.head,
      ...headSummary,
      ...(await dirtyStatus(worktree.path, { execFile, env, pathExists, compareRef, fileLimit }))
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
    error: 0,
    code_dirty: 0,
    diagnostics_only_dirty: 0
  };

  for (const worktree of worktrees) {
    if (Object.hasOwn(summary, worktree.dirty_status)) {
      summary[worktree.dirty_status] += 1;
    }
    if (worktree.dirty_status === "dirty" && worktree.dirty_scope === "code") {
      summary.code_dirty += 1;
    }
    if (worktree.dirty_status === "dirty" && worktree.dirty_scope === "diagnostics_only") {
      summary.diagnostics_only_dirty += 1;
    }
    if (worktree.suspicious_no_tool_completion_count > 0) {
      summary.suspicious_no_tool_completion_count ??= 0;
      summary.suspicious_no_tool_completion_count += worktree.suspicious_no_tool_completion_count;
    }
    if (worktree.dirty_status === "dirty" && worktree.dirty_scope === "code") {
      const unmatchedCount = worktree.compare_ref_unmatched_count;
      if (typeof unmatchedCount === "number") {
        summary.code_compare_ref_unmatched_files ??= 0;
        summary.code_compare_ref_unmatched_files += unmatchedCount;
      }
    }
    if (worktree.cleanup_recommendation) {
      summary.cleanup_remove ??= 0;
      summary.cleanup_prune ??= 0;
      summary.cleanup_keep ??= 0;
      summary.cleanup_inspect ??= 0;
      const cleanupKey = `cleanup_${worktree.cleanup_recommendation}`;
      if (Object.hasOwn(summary, cleanupKey)) {
        summary[cleanupKey] += 1;
      }
      if (worktree.path) {
        summary.cleanup_actions ??= [];
        summary.cleanup_actions.push(summarizeCleanupAction(worktree));
      }
    }
  }

  return summary;
}

function summarizeCleanupAction(worktree) {
  const action = {
    action: worktree.cleanup_recommendation,
    reason: worktree.cleanup_reason,
    path: worktree.path,
    branch: worktree.branch ?? null,
    dirty_status: worktree.dirty_status
  };
  for (const key of [
    "dirty_scope",
    "compare_ref_unmatched_count",
    "compare_ref_unmatched_files",
    "cleanup_command"
  ]) {
    if (worktree[key] !== undefined) {
      action[key] = worktree[key];
    }
  }
  if (worktree.dirty_file_count > 0) {
    action.dirty_file_count = worktree.dirty_file_count;
  }
  return action;
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

  const headStatus =
    worktree.head_in_compare_ref === true
      ? "; head=yes"
      : worktree.head_in_compare_ref === false
        ? "; head=no"
        : worktree.head_in_compare_ref === null
          ? "; head=unknown"
          : "";
  if (worktree.compare_ref_checked_count === undefined) {
    return `${worktree.compare_ref}${headStatus}`;
  }

  const matchingFiles = Array.isArray(worktree.compare_ref_matching_files)
    ? worktree.compare_ref_matching_files
    : [];
  const matchCount = worktree.compare_ref_match_count ?? matchingFiles.length;
  const checkedCount = worktree.compare_ref_checked_count ?? 0;
  const remaining = matchCount - matchingFiles.length;
  const suffix = remaining > 0 ? `, +${remaining} more` : "";
  const files = matchingFiles.length > 0 ? `: ${matchingFiles.join(", ")}${suffix}` : "";
  const unmatchedFiles = Array.isArray(worktree.compare_ref_unmatched_files)
    ? worktree.compare_ref_unmatched_files
    : [];
  const unmatchedCount = worktree.compare_ref_unmatched_count ?? unmatchedFiles.length;
  const unmatchedRemaining = unmatchedCount - unmatchedFiles.length;
  const unmatchedSuffix = unmatchedRemaining > 0 ? `, +${unmatchedRemaining} more` : "";
  const unmatched =
    unmatchedCount > 0
      ? `; unmatched ${unmatchedCount}: ${unmatchedFiles.join(", ")}${unmatchedSuffix}`
      : "";
  if (worktree.dirty_scope === "diagnostics_only" && checkedCount === 0) {
    return `${worktree.compare_ref} diagnostics_only${unmatched}${headStatus}`;
  }
  return `${worktree.compare_ref} ${matchCount}/${checkedCount}${files}${unmatched}${headStatus}`;
}

function shellQuote(value) {
  const text = String(value);
  return /^[A-Za-z0-9_./:@%+=,-]+$/.test(text) ? text : `'${text.replaceAll("'", "'\\''")}'`;
}

function cleanupCommand(worktree, recommendation) {
  if (recommendation === "remove") {
    return `git worktree remove ${shellQuote(worktree.path)}`;
  }
  if (recommendation === "prune") {
    return "git worktree prune";
  }
  if (recommendation === "inspect" && worktree.dirty_scope === "diagnostics_only") {
    return `git -C ${shellQuote(worktree.path)} clean -nd -- .kairox-eval/`;
  }
  return null;
}

function cleanupRecommendation(worktree) {
  const withCommand = (recommendation) => {
    const command = cleanupCommand(worktree, recommendation.cleanup_recommendation);
    return command ? { ...recommendation, cleanup_command: command } : recommendation;
  };

  if (worktree.dirty_status === "clean") {
    return withCommand({ cleanup_recommendation: "remove", cleanup_reason: "clean_worktree" });
  }
  if (worktree.dirty_status === "missing") {
    return withCommand({ cleanup_recommendation: "prune", cleanup_reason: "missing_path" });
  }
  if (worktree.dirty_status === "error") {
    return { cleanup_recommendation: "inspect", cleanup_reason: "status_error" };
  }
  if (worktree.dirty_scope === "diagnostics_only") {
    return withCommand({
      cleanup_recommendation: "inspect",
      cleanup_reason: "diagnostics_only_dirty"
    });
  }
  if (worktree.compare_ref_unmatched_count === 0) {
    return {
      cleanup_recommendation: "inspect",
      cleanup_reason: "dirty_files_match_compare_ref"
    };
  }
  if (worktree.compare_ref_unmatched_count > 0) {
    return {
      cleanup_recommendation: "keep",
      cleanup_reason: "dirty_files_not_in_compare_ref"
    };
  }
  return { cleanup_recommendation: "inspect", cleanup_reason: "dirty_without_compare_ref" };
}

function annotateCleanupRecommendations(worktrees) {
  return worktrees.map((worktree) => ({
    ...worktree,
    ...cleanupRecommendation(worktree)
  }));
}

function formatCleanupRecommendation(worktree) {
  if (!worktree.cleanup_recommendation) {
    return "-";
  }
  const label = `${worktree.cleanup_recommendation}:${worktree.cleanup_reason}`;
  return worktree.cleanup_command ? `${label} (${worktree.cleanup_command})` : label;
}

export function formatHumanTable(worktrees) {
  const summary = summarizeAudit(worktrees);
  const summaryLine = `${formatSummaryLine(summary)}\n`;

  if (worktrees.length === 0) {
    return `${summaryLine}No eval worktrees found.\n`;
  }

  const includeCompareRef = worktrees.some((worktree) => worktree.compare_ref);
  const includeCleanup = worktrees.some((worktree) => worktree.cleanup_recommendation);
  const includeDirtyScope = worktrees.some((worktree) => worktree.dirty_scope);
  const headers = [
    "PATH",
    "BRANCH",
    "HEAD",
    "PATH_EXISTS",
    "DIRTY_STATUS",
    ...(includeDirtyScope ? ["DIRTY_SCOPE"] : []),
    "DIRTY_FILES",
    ...(includeCompareRef ? ["COMPARE_REF_MATCHES"] : []),
    ...(includeCleanup ? ["CLEANUP"] : [])
  ];
  const rows = worktrees.map((worktree) => {
    const row = [
      worktree.path,
      worktree.branch ?? "-",
      shortHead(worktree.head),
      worktree.path_exists ? "yes" : "no",
      worktree.dirty_status,
      ...(includeDirtyScope ? [worktree.dirty_scope ?? "-"] : []),
      formatDirtyFiles(worktree)
    ];
    if (includeCompareRef) {
      row.push(formatCompareRef(worktree));
    }
    if (includeCleanup) {
      row.push(formatCleanupRecommendation(worktree));
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
  const unmatched =
    summary.code_compare_ref_unmatched_files === undefined
      ? ""
      : ` code_compare_ref_unmatched_files=${summary.code_compare_ref_unmatched_files}`;
  const cleanup =
    summary.cleanup_remove === undefined
      ? ""
      : ` cleanup_remove=${summary.cleanup_remove} cleanup_prune=${summary.cleanup_prune} cleanup_keep=${summary.cleanup_keep} cleanup_inspect=${summary.cleanup_inspect}`;
  const suspicious =
    summary.suspicious_no_tool_completion_count === undefined
      ? ""
      : ` suspicious_no_tool_completion_count=${summary.suspicious_no_tool_completion_count}`;
  return `Summary: total=${summary.total} clean=${summary.clean} dirty=${summary.dirty} code_dirty=${summary.code_dirty} diagnostics_only_dirty=${summary.diagnostics_only_dirty}${unmatched}${suspicious} missing=${summary.missing} error=${summary.error}${cleanup}`;
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    json: false,
    summaryOnly: false,
    dirtyOnly: false,
    cleanOnly: false,
    compareRef: null,
    allFiles: false,
    recommendCleanup: false,
    failOnSuspiciousNoTool: false
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
    if (arg === "--all-files") {
      parsed.allFiles = true;
      continue;
    }
    if (arg === "--recommend-cleanup") {
      parsed.recommendCleanup = true;
      continue;
    }
    if (arg === "--fail-on-suspicious-no-tool") {
      parsed.failOnSuspiciousNoTool = true;
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

    let audited = filterAuditResults(
      await auditEvalWorktrees({
        execFile,
        pathExists,
        cwd,
        env,
        compareRef: args.summaryOnly && !args.recommendCleanup ? null : args.compareRef,
        fileLimit: args.allFiles ? null : DIRTY_FILE_LIMIT
      }),
      args
    );
    if (args.recommendCleanup) {
      audited = annotateCleanupRecommendations(audited);
    }
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
    if (
      args.failOnSuspiciousNoTool &&
      typeof summary.suspicious_no_tool_completion_count === "number" &&
      summary.suspicious_no_tool_completion_count > 0
    ) {
      stderr.write(
        `Error: suspicious_no_tool_completion_count=${summary.suspicious_no_tool_completion_count}\n`
      );
      return 2;
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
