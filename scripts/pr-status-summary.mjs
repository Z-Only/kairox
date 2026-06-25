import { execFile as execFileCallback } from "node:child_process";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);

export const GH_PR_VIEW_FIELDS =
  "number,title,state,mergeStateStatus,headRefName,headRefOid,mergeCommit,statusCheckRollup";

const DEFAULT_WATCH_INTERVAL_MS = 30_000;
const DEFAULT_WATCH_TIMEOUT_MS = 30 * 60_000;

const USAGE = `Usage: node scripts/pr-status-summary.mjs [--json] [--watch] [--interval-ms <n>] [--timeout-ms <n>] <pr-number> [<pr-number> ...]

Summarizes GitHub PR merge and status check state for watcher or manual review.

Options:
  --json             Print stable JSON instead of a human-readable table.
  --watch            Poll until checks finish or fail.
  --interval-ms <n>  Poll interval for --watch. Default: ${DEFAULT_WATCH_INTERVAL_MS}.
  --timeout-ms <n>   Overall timeout for --watch. Default: ${DEFAULT_WATCH_TIMEOUT_MS}.
  --help, -h         Show this help.
`;

class UsageError extends Error {}

function stringOrNull(value) {
  if (value === undefined || value === null) {
    return null;
  }
  return String(value);
}

function upperOrEmpty(value) {
  return stringOrNull(value)?.toUpperCase() ?? "";
}

function firstPresent(source, names) {
  for (const name of names) {
    if (source?.[name] !== undefined && source[name] !== null) {
      return source[name];
    }
  }
  return undefined;
}

const SUCCESS_CONCLUSIONS = new Set(["SUCCESS"]);
const FAILURE_CONCLUSIONS = new Set([
  "ACTION_REQUIRED",
  "CANCELLED",
  "FAILURE",
  "STARTUP_FAILURE",
  "TIMED_OUT"
]);
const NEUTRAL_CONCLUSIONS = new Set(["NEUTRAL"]);
const SKIPPED_CONCLUSIONS = new Set(["SKIPPED"]);
const PENDING_STATUSES = new Set(["EXPECTED", "IN_PROGRESS", "PENDING", "QUEUED", "REQUESTED"]);
const SUCCESS_STATES = new Set(["SUCCESS"]);
const FAILURE_STATES = new Set(["ERROR", "FAILURE"]);
const PENDING_STATES = new Set(["EXPECTED", "PENDING"]);

function classifyCheck({ status, conclusion, state }) {
  const normalizedConclusion = upperOrEmpty(conclusion);
  if (SUCCESS_CONCLUSIONS.has(normalizedConclusion)) {
    return "success";
  }
  if (FAILURE_CONCLUSIONS.has(normalizedConclusion)) {
    return "failure";
  }
  if (SKIPPED_CONCLUSIONS.has(normalizedConclusion)) {
    return "skipped";
  }
  if (NEUTRAL_CONCLUSIONS.has(normalizedConclusion)) {
    return "neutral";
  }

  const normalizedState = upperOrEmpty(state);
  if (SUCCESS_STATES.has(normalizedState)) {
    return "success";
  }
  if (FAILURE_STATES.has(normalizedState)) {
    return "failure";
  }
  if (PENDING_STATES.has(normalizedState)) {
    return "pending";
  }

  if (PENDING_STATUSES.has(upperOrEmpty(status))) {
    return "pending";
  }

  return "unknown";
}

function createCounts() {
  return {
    success: 0,
    failure: 0,
    pending: 0,
    skipped: 0,
    neutral: 0,
    unknown: 0
  };
}

export function normalizeStatusCheckRollup(statusCheckRollup) {
  const checks = [];
  const counts = createCounts();

  if (!Array.isArray(statusCheckRollup)) {
    return { counts, checks };
  }

  for (const [index, rawCheck] of statusCheckRollup.entries()) {
    const name = String(
      firstPresent(rawCheck, ["name", "context", "workflowName"]) ?? `check-${index + 1}`
    );
    const status = stringOrNull(rawCheck?.status);
    const conclusion = stringOrNull(rawCheck?.conclusion);
    const state = stringOrNull(rawCheck?.state);
    const result = classifyCheck({ status, conclusion, state });

    counts[result] += 1;
    checks.push({
      name,
      status,
      conclusion,
      state,
      result
    });
  }

  return { counts, checks };
}

function mergeCommitOid(value) {
  if (value && typeof value === "object") {
    return stringOrNull(value.oid);
  }
  return stringOrNull(value);
}

export function summarizePullRequest(rawPullRequest) {
  if (!rawPullRequest || typeof rawPullRequest !== "object") {
    throw new Error("Expected gh pr view JSON object");
  }

  const normalizedChecks = normalizeStatusCheckRollup(rawPullRequest.statusCheckRollup);

  return {
    number: Number(rawPullRequest.number),
    title: stringOrNull(rawPullRequest.title) ?? "",
    state: stringOrNull(rawPullRequest.state) ?? "",
    merge_state_status: stringOrNull(rawPullRequest.mergeStateStatus),
    head_ref_name: stringOrNull(rawPullRequest.headRefName),
    head_ref_oid: stringOrNull(rawPullRequest.headRefOid),
    merge_commit_oid: mergeCommitOid(rawPullRequest.mergeCommit),
    checks: {
      counts: normalizedChecks.counts,
      items: normalizedChecks.checks
    }
  };
}

function parseArgs(argv) {
  const parsed = {
    help: false,
    json: false,
    watch: false,
    intervalMs: DEFAULT_WATCH_INTERVAL_MS,
    timeoutMs: DEFAULT_WATCH_TIMEOUT_MS,
    prNumbers: []
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
    if (arg === "--watch") {
      parsed.watch = true;
      continue;
    }
    if (arg === "--interval-ms" || arg === "--timeout-ms") {
      const value = argv[index + 1];
      if (value === undefined || value.startsWith("-")) {
        throw new UsageError(`${arg} requires a positive integer value.`);
      }
      if (arg === "--interval-ms") {
        parsed.intervalMs = parsePositiveIntegerOption(arg, value);
      } else {
        parsed.timeoutMs = parsePositiveIntegerOption(arg, value);
      }
      index += 1;
      continue;
    }
    if (arg.startsWith("-")) {
      throw new UsageError(`Unknown argument: ${arg}`);
    }
    parsed.prNumbers.push(arg);
  }

  if (parsed.help) {
    return parsed;
  }
  if (parsed.prNumbers.length === 0) {
    throw new UsageError("Missing required PR number.");
  }

  for (const prNumber of parsed.prNumbers) {
    if (!/^[1-9]\d*$/.test(prNumber)) {
      throw new UsageError(`PR number must be a positive integer: ${prNumber}`);
    }
  }

  return parsed;
}

function parsePositiveIntegerOption(name, value) {
  if (!/^[1-9]\d*$/.test(value)) {
    throw new UsageError(`${name} must be a positive integer: ${value}`);
  }
  return Number(value);
}

function parseGhStdout(stdout, prNumber) {
  const trimmed = String(stdout ?? "").trim();
  if (!trimmed) {
    throw new Error(`gh pr view ${prNumber} returned empty JSON output`);
  }

  try {
    return JSON.parse(trimmed);
  } catch (error) {
    throw new Error(`gh pr view ${prNumber} returned invalid JSON: ${error.message}`);
  }
}

function ghFailureMessage(error, prNumber) {
  if (error?.code === "ENOENT" || /spawn gh ENOENT/.test(String(error?.message))) {
    return "gh was not found on PATH. Install GitHub CLI or add it to PATH, then rerun this command.";
  }

  const detail = [error?.stderr, error?.stdout, error?.message].filter(Boolean).join("\n").trim();
  const exitCode = typeof error?.code === "number" ? error.code : "unknown";
  return [`gh pr view ${prNumber} failed (exit ${exitCode}).`, detail].filter(Boolean).join("\n");
}

export async function readPullRequest(
  prNumber,
  { execFile = execFileAsync, env = process.env, cwd = process.cwd() } = {}
) {
  let result;
  try {
    result = await execFile("gh", ["pr", "view", String(prNumber), "--json", GH_PR_VIEW_FIELDS], {
      cwd,
      env,
      maxBuffer: 10 * 1024 * 1024
    });
  } catch (error) {
    throw new Error(ghFailureMessage(error, prNumber));
  }

  return parseGhStdout(result.stdout, prNumber);
}

function shortOid(oid) {
  const value = stringOrNull(oid);
  return value ? value.slice(0, 8) : "-";
}

function headLabel(summary) {
  const branch = summary.head_ref_name ?? "-";
  return `${branch}@${shortOid(summary.head_ref_oid)}`;
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function formatTable(headers, rows) {
  const stringRows = rows.map((row) => row.map((cell) => String(cell ?? "-")));
  const widths = headers.map((header, index) =>
    Math.max(header.length, ...stringRows.map((row) => row[index].length))
  );
  const formatRow = (row) =>
    row
      .map((cell, index) => String(cell ?? "-").padEnd(widths[index]))
      .join("  ")
      .trimEnd();

  return [
    formatRow(headers),
    formatRow(headers.map((header, index) => "-".repeat(widths[index]))),
    ...stringRows.map(formatRow)
  ].join("\n");
}

function checkCell(value) {
  return value ?? "-";
}

export function formatHumanSummary(summaries) {
  const header = [
    "PR",
    "State",
    "Merge",
    "Head",
    "Success",
    "Failure",
    "Pending",
    "Skipped",
    "Neutral",
    "Unknown",
    "Title"
  ];
  const rows = summaries.map((summary) => [
    `#${summary.number}`,
    summary.state || "-",
    summary.merge_state_status || "-",
    headLabel(summary),
    summary.checks.counts.success,
    summary.checks.counts.failure,
    summary.checks.counts.pending,
    summary.checks.counts.skipped,
    summary.checks.counts.neutral,
    summary.checks.counts.unknown,
    summary.title
  ]);
  const sections = [formatTable(header, rows)];

  for (const summary of summaries) {
    sections.push("");
    sections.push(`Checks for #${summary.number}`);
    const checkRows =
      summary.checks.items.length === 0
        ? [["(no status checks)", "-", "-", "-", "-"]]
        : summary.checks.items.map((check) => [
            check.name,
            checkCell(check.status),
            checkCell(check.conclusion),
            checkCell(check.state),
            check.result
          ]);
    sections.push(formatTable(["Name", "Status", "Conclusion", "State", "Result"], checkRows));
  }

  return sections.join("\n");
}

async function readPullRequestSummaries(prNumbers, { execFile, env, cwd }) {
  const summaries = [];
  for (const prNumber of prNumbers) {
    summaries.push(summarizePullRequest(await readPullRequest(prNumber, { execFile, env, cwd })));
  }
  return summaries;
}

function hasFailures(summaries) {
  return summaries.some((summary) => summary.checks.counts.failure > 0);
}

function allReady(summaries) {
  return summaries.every(
    (summary) =>
      summary.checks.counts.pending === 0 &&
      summary.checks.counts.failure === 0 &&
      summary.checks.counts.unknown === 0
  );
}

async function watchPullRequests(
  prNumbers,
  { execFile, env, cwd, intervalMs, timeoutMs, sleepFn = sleep, now = Date.now }
) {
  const deadline = now() + timeoutMs;

  while (true) {
    const summaries = await readPullRequestSummaries(prNumbers, { execFile, env, cwd });
    if (hasFailures(summaries)) {
      return { exitCode: 1, summaries };
    }
    if (allReady(summaries)) {
      return { exitCode: 0, summaries };
    }
    if (now() >= deadline) {
      return { exitCode: 1, summaries };
    }

    await sleepFn(intervalMs);
  }
}

function writeSummaries(stdout, summaries, { json }) {
  if (json) {
    stdout.write(`${JSON.stringify({ pull_requests: summaries }, null, 2)}\n`);
  } else {
    stdout.write(`${formatHumanSummary(summaries)}\n`);
  }
}

export async function runCli(
  argv = process.argv.slice(2),
  {
    stdout = process.stdout,
    stderr = process.stderr,
    execFile = execFileAsync,
    env = process.env,
    cwd = process.cwd(),
    sleep: sleepFn = sleep,
    now = Date.now
  } = {}
) {
  try {
    const args = parseArgs(argv);
    if (args.help) {
      stdout.write(USAGE);
      return 0;
    }

    if (args.watch) {
      const result = await watchPullRequests(args.prNumbers, {
        execFile,
        env,
        cwd,
        intervalMs: args.intervalMs,
        timeoutMs: args.timeoutMs,
        sleepFn,
        now
      });
      writeSummaries(stdout, result.summaries, args);
      return result.exitCode;
    }

    const summaries = await readPullRequestSummaries(args.prNumbers, { execFile, env, cwd });
    writeSummaries(stdout, summaries, args);
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
