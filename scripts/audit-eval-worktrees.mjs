import { execFile as execFileCallback } from "node:child_process";
import { existsSync } from "node:fs";
import { basename } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);
const GIT_BUFFER = 10 * 1024 * 1024;

export const USAGE = `Usage: node scripts/audit-eval-worktrees.mjs [--json]

Audits local eval worktrees without deleting worktrees or branches.

Selection:
  Includes worktrees whose branch starts with "eval/" or whose path basename
  starts with "eval-kairox-".

Options:
  --json       Print a stable JSON array.
  --help, -h   Show this help.
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

async function dirtyStatus(worktreePath, { execFile, env, pathExists }) {
  const exists = pathExists(worktreePath);
  if (!exists) {
    return {
      dirty_status: "missing",
      path_exists: false
    };
  }

  try {
    const result = await execFile("git", ["-C", worktreePath, "status", "--short"], {
      env,
      maxBuffer: GIT_BUFFER
    });
    return {
      dirty_status: result.stdout.trim() === "" ? "clean" : "dirty",
      path_exists: true
    };
  } catch {
    return {
      dirty_status: "error",
      path_exists: true
    };
  }
}

export async function auditEvalWorktrees({
  execFile = execFileAsync,
  pathExists = existsSync,
  cwd = process.cwd(),
  env = process.env
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
      ...(await dirtyStatus(worktree.path, { execFile, env, pathExists }))
    });
  }

  return audited;
}

function shortHead(head) {
  return head ? head.slice(0, 12) : "-";
}

function pad(value, width) {
  return String(value).padEnd(width, " ");
}

export function formatHumanTable(worktrees) {
  if (worktrees.length === 0) {
    return "No eval worktrees found.\n";
  }

  const headers = ["PATH", "BRANCH", "HEAD", "PATH_EXISTS", "DIRTY_STATUS"];
  const rows = worktrees.map((worktree) => [
    worktree.path,
    worktree.branch ?? "-",
    shortHead(worktree.head),
    worktree.path_exists ? "yes" : "no",
    worktree.dirty_status
  ]);
  const widths = headers.map((header, index) =>
    Math.max(header.length, ...rows.map((row) => String(row[index]).length))
  );
  const formatRow = (row) => row.map((value, index) => pad(value, widths[index])).join("  ");
  const separator = widths.map((width) => "-".repeat(width)).join("  ");

  return `${[formatRow(headers), separator, ...rows.map(formatRow)].join("\n")}\n`;
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    json: false
  };

  for (const arg of argv) {
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      continue;
    }
    if (arg === "--json") {
      parsed.json = true;
      continue;
    }
    throw new UsageError(`Unknown argument: ${arg}`);
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

    const audited = await auditEvalWorktrees({ execFile, pathExists, cwd, env });
    stdout.write(args.json ? `${JSON.stringify(audited, null, 2)}\n` : formatHumanTable(audited));
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
