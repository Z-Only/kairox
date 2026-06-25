import { execFile as execFileCallback } from "node:child_process";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

import { parseWorktreePorcelain } from "./audit-eval-worktrees.mjs";

const execFileAsync = promisify(execFileCallback);
const GIT_BUFFER = 10 * 1024 * 1024;
const GH_PR_VIEW_FIELDS = "number,state,mergeCommit,headRefName,headRefOid";

export const USAGE = `Usage: node scripts/cleanup-merged-worktree.mjs [--branch <branch>] [--worktree <path>] [--pr <number>] [--force-dirty] [--dry-run] [--json]

Removes a merged feature worktree, then force-deletes its local and remote branch.
When --branch is omitted, the current git branch is used.

Safety checks:
  - GitHub must report the PR as MERGED.
  - The PR merge commit must be reachable from origin/main.
  - The target worktree must live under the repository .worktrees directory.
  - Dirty worktrees are refused unless --force-dirty is supplied.

Options:
  --branch <branch>    Local branch to clean up. Defaults to current branch.
  --worktree <path>   Worktree path. Defaults to the worktree owning --branch.
  --pr <number>       PR number to verify. Defaults to resolving by --branch.
  --force-dirty       Force-remove a dirty worktree.
  --dry-run           Print the planned cleanup without deleting anything.
  --json              Write the cleanup result as JSON. No human output is written on success.
  --help, -h          Show this help.
`;

class UsageError extends Error {}

function parsePositiveIntegerOption(name, value) {
  if (!/^[1-9]\d*$/.test(value ?? "")) {
    throw new UsageError(`${name} must be a positive integer: ${value}`);
  }
  return value;
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    branch: undefined,
    worktree: undefined,
    pr: undefined,
    forceDirty: false,
    dryRun: false,
    json: false
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      continue;
    }
    if (arg === "--force-dirty") {
      parsed.forceDirty = true;
      continue;
    }
    if (arg === "--dry-run") {
      parsed.dryRun = true;
      continue;
    }
    if (arg === "--json") {
      parsed.json = true;
      continue;
    }
    if (arg === "--branch" || arg === "--worktree" || arg === "--pr") {
      const value = argv[index + 1];
      if (value === undefined || value.startsWith("-")) {
        throw new UsageError(`${arg} requires a value.`);
      }
      if (arg === "--branch") {
        parsed.branch = value;
      } else if (arg === "--worktree") {
        parsed.worktree = value;
      } else {
        parsed.pr = parsePositiveIntegerOption(arg, value);
      }
      index += 1;
      continue;
    }
    if (arg.startsWith("--branch=")) {
      parsed.branch = arg.slice("--branch=".length);
      continue;
    }
    if (arg.startsWith("--worktree=")) {
      parsed.worktree = arg.slice("--worktree=".length);
      continue;
    }
    if (arg.startsWith("--pr=")) {
      parsed.pr = parsePositiveIntegerOption("--pr", arg.slice("--pr=".length));
      continue;
    }
    throw new UsageError(`Unknown argument: ${arg}`);
  }

  if (parsed.help) {
    return parsed;
  }
  if (parsed.branch === "") {
    throw new UsageError("--branch requires a non-empty value.");
  }
  if (parsed.worktree === "") {
    throw new UsageError("--worktree requires a non-empty path.");
  }

  return parsed;
}

function trimStdout(result) {
  return String(result.stdout ?? "").trim();
}

function parseJsonStdout(stdout, label) {
  const trimmed = String(stdout ?? "").trim();
  if (!trimmed) {
    throw new Error(`${label} returned empty JSON output`);
  }

  try {
    return JSON.parse(trimmed);
  } catch (error) {
    throw new Error(`${label} returned invalid JSON: ${error.message}`);
  }
}

function commandFailureMessage(error, command, args) {
  if (error?.code === "ENOENT" || new RegExp(`spawn ${command} ENOENT`).test(error?.message)) {
    return `${command} was not found on PATH. Install it or add it to PATH, then rerun this command.`;
  }

  const detail = [error?.stderr, error?.stdout, error?.message].filter(Boolean).join("\n").trim();
  const exitCode = typeof error?.code === "number" ? error.code : "unknown";
  return [`${command} ${args.join(" ")} failed (exit ${exitCode}).`, detail]
    .filter(Boolean)
    .join("\n");
}

async function execChecked(execFile, command, args, { cwd, env }) {
  try {
    return await execFile(command, args, {
      cwd,
      env,
      maxBuffer: GIT_BUFFER
    });
  } catch (error) {
    throw new Error(commandFailureMessage(error, command, args));
  }
}

async function readCurrentWorktreeRoot({ execFile, cwd, env }) {
  const result = await execChecked(execFile, "git", ["rev-parse", "--show-toplevel"], {
    cwd,
    env
  });
  const worktreeRoot = trimStdout(result);
  if (!worktreeRoot) {
    throw new Error("git rev-parse --show-toplevel returned an empty path");
  }
  return worktreeRoot;
}

async function readCurrentBranch({ execFile, cwd, env }) {
  const result = await execChecked(execFile, "git", ["branch", "--show-current"], {
    cwd,
    env
  });
  const branch = trimStdout(result);
  if (!branch) {
    throw new UsageError("Unable to infer current branch. Pass --branch <branch>.");
  }
  return branch;
}

function resolveGitPath(path, cwd) {
  return isAbsolute(path) ? resolve(path) : resolve(cwd, path);
}

async function readRepoRoot({ execFile, cwd, env, currentWorktreeRoot }) {
  const result = await execChecked(execFile, "git", ["rev-parse", "--git-common-dir"], {
    cwd,
    env
  });
  const commonGitDir = trimStdout(result);
  if (!commonGitDir) {
    throw new Error("git rev-parse --git-common-dir returned an empty path");
  }

  const resolvedCommonGitDir = resolveGitPath(commonGitDir, cwd);
  if (basename(resolvedCommonGitDir) === ".git") {
    return dirname(resolvedCommonGitDir);
  }
  return currentWorktreeRoot;
}

async function readWorktrees({ execFile, cwd, env }) {
  const result = await execChecked(execFile, "git", ["worktree", "list", "--porcelain"], {
    cwd,
    env
  });
  return parseWorktreePorcelain(result.stdout);
}

async function readPullRequest({ execFile, cwd, env, branch, pr }) {
  const selector = pr ?? branch;
  const args = ["pr", "view", selector, "--json", GH_PR_VIEW_FIELDS];
  const result = await execChecked(execFile, "gh", args, { cwd, env });
  return parseJsonStdout(result.stdout, `gh ${args.join(" ")}`);
}

function mergeCommitOid(value) {
  if (value && typeof value === "object") {
    return value.oid ? String(value.oid) : null;
  }
  return value === undefined || value === null ? null : String(value);
}

function assertPullRequestMerged(pullRequest, branch) {
  const mergeOid = mergeCommitOid(pullRequest?.mergeCommit);
  if (pullRequest?.state !== "MERGED" || !mergeOid) {
    throw new Error(`PR for ${branch} is not merged`);
  }
  if (pullRequest.headRefName && pullRequest.headRefName !== branch) {
    throw new Error(
      `PR head ref ${pullRequest.headRefName} does not match requested branch ${branch}`
    );
  }
  return mergeOid;
}

async function assertMergeCommitOnMain(mergeOid, { execFile, cwd, env }) {
  await execChecked(execFile, "git", ["fetch", "origin", "main"], { cwd, env });
  await execChecked(execFile, "git", ["merge-base", "--is-ancestor", mergeOid, "origin/main"], {
    cwd,
    env
  });
}

function resolveProvidedWorktreePath(worktree, repoRoot) {
  if (!worktree) {
    return null;
  }
  return isAbsolute(worktree) ? resolve(worktree) : resolve(repoRoot, worktree);
}

function resolveBranchWorktreePath({ branch, worktree, worktrees, repoRoot }) {
  const providedPath = resolveProvidedWorktreePath(worktree, repoRoot);
  if (providedPath) {
    return providedPath;
  }

  const matched = worktrees.find((candidate) => candidate.branch === branch);
  if (!matched) {
    throw new Error(`No worktree found for branch ${branch}`);
  }
  return resolve(matched.path);
}

function assertWorktreeUnderRepoWorktrees(repoRoot, worktreePath) {
  const allowedRoot = resolve(repoRoot, ".worktrees");
  const normalizedWorktree = resolve(worktreePath);
  const rel = relative(allowedRoot, normalizedWorktree);
  if (rel === "" || rel.startsWith("..") || isAbsolute(rel)) {
    throw new Error(`Refusing to remove worktree outside ${allowedRoot}: ${normalizedWorktree}`);
  }
}

async function readDirtyStatus(worktreePath, { execFile, env }) {
  const result = await execChecked(execFile, "git", ["-C", worktreePath, "status", "--short"], {
    cwd: undefined,
    env
  });
  return String(result.stdout ?? "");
}

function assertCleanWorktree(status, worktreePath, forceDirty) {
  if (status.trim() === "" || forceDirty) {
    return;
  }
  throw new Error(`Worktree is dirty: ${worktreePath}\n${status.trimEnd()}`);
}

function remoteBranchMissing(error) {
  const text = [error?.message, error?.stderr, error?.stdout].filter(Boolean).join("\n");
  return /remote ref does not exist|not found|unable to delete/i.test(text);
}

async function deleteRemoteBranch(execFile, branch, { cwd, env, stdout, quiet = false }) {
  try {
    await execFile("git", ["push", "origin", "--delete", branch], {
      cwd,
      env,
      maxBuffer: GIT_BUFFER
    });
    if (!quiet) {
      stdout.write(`deleted remote branch: ${branch}\n`);
    }
    return { status: "completed" };
  } catch (error) {
    if (remoteBranchMissing(error)) {
      if (!quiet) {
        stdout.write(`remote branch already absent: ${branch}\n`);
      }
      return {
        status: "skipped",
        reason: "remote branch already absent"
      };
    }
    throw new Error(commandFailureMessage(error, "git", ["push", "origin", "--delete", branch]));
  }
}

function createCleanupPlan({ branch, worktreePath, prNumber, mergeOid, forceDirty, dryRun }) {
  const removeArgs = forceDirty
    ? ["worktree", "remove", "--force", worktreePath]
    : ["worktree", "remove", worktreePath];
  const status = dryRun ? "planned" : "pending";

  return {
    branch,
    worktree_path: worktreePath,
    pr_number: prNumber,
    merge_commit_oid: mergeOid,
    dry_run: dryRun,
    actions: [
      {
        action: "remove_worktree",
        command: "git",
        args: removeArgs,
        status,
        target: worktreePath
      },
      {
        action: "prune_worktrees",
        command: "git",
        args: ["worktree", "prune"],
        status
      },
      {
        action: "delete_local_branch",
        command: "git",
        args: ["branch", "-D", branch],
        status,
        target: branch
      },
      {
        action: "delete_remote_branch",
        command: "git",
        args: ["push", "origin", "--delete", branch],
        status,
        target: branch
      }
    ]
  };
}

function writeHumanDryRunPlan(plan, stdout) {
  stdout.write(`dry-run: planned cleanup for ${plan.branch}\n`);
  stdout.write(`dry-run: would remove worktree: ${plan.worktree_path}\n`);
  stdout.write("dry-run: would prune worktree metadata\n");
  stdout.write(`dry-run: would delete local branch: ${plan.branch}\n`);
  stdout.write(`dry-run: would delete remote branch: ${plan.branch}\n`);
}

async function executeCleanupPlan(plan, { execFile, cwd, env, stdout, quiet }) {
  const [removeWorktree, pruneWorktrees, deleteLocalBranch, deleteRemoteBranchAction] =
    plan.actions;

  await execChecked(execFile, "git", removeWorktree.args, { cwd, env });
  removeWorktree.status = "completed";
  if (!quiet) {
    stdout.write(`removed worktree: ${plan.worktree_path}\n`);
  }

  await execChecked(execFile, "git", pruneWorktrees.args, { cwd, env });
  pruneWorktrees.status = "completed";
  if (!quiet) {
    stdout.write("pruned worktree metadata\n");
  }

  await execChecked(execFile, "git", deleteLocalBranch.args, { cwd, env });
  deleteLocalBranch.status = "completed";
  if (!quiet) {
    stdout.write(`deleted local branch: ${plan.branch}\n`);
  }

  const remoteResult = await deleteRemoteBranch(execFile, plan.branch, {
    cwd,
    env,
    stdout,
    quiet
  });
  deleteRemoteBranchAction.status = remoteResult.status;
  if (remoteResult.reason) {
    deleteRemoteBranchAction.reason = remoteResult.reason;
  }
}

export async function cleanupMergedWorktree({
  branch,
  worktree,
  pr,
  forceDirty = false,
  dryRun = false,
  json = false,
  stdout = process.stdout,
  execFile = execFileAsync,
  cwd = process.cwd(),
  env = process.env
}) {
  const currentWorktreeRoot = await readCurrentWorktreeRoot({ execFile, cwd, env });
  const repoRoot = await readRepoRoot({ execFile, cwd, env, currentWorktreeRoot });
  const worktrees = worktree ? [] : await readWorktrees({ execFile, cwd: repoRoot, env });
  const pullRequest = await readPullRequest({ execFile, cwd: repoRoot, env, branch, pr });
  const mergeOid = assertPullRequestMerged(pullRequest, branch);

  await assertMergeCommitOnMain(mergeOid, { execFile, cwd: repoRoot, env });

  const worktreePath = resolveBranchWorktreePath({ branch, worktree, worktrees, repoRoot });
  assertWorktreeUnderRepoWorktrees(repoRoot, worktreePath);

  const dirtyStatus = await readDirtyStatus(worktreePath, { execFile, env });
  assertCleanWorktree(dirtyStatus, worktreePath, forceDirty);

  const result = createCleanupPlan({
    branch,
    worktreePath,
    prNumber: Number(pullRequest.number),
    mergeOid,
    forceDirty,
    dryRun
  });

  if (dryRun) {
    if (!json) {
      writeHumanDryRunPlan(result, stdout);
    }
    return result;
  }

  await executeCleanupPlan(result, {
    execFile,
    cwd: repoRoot,
    env,
    stdout,
    quiet: json
  });
  return result;
}

export async function runCli(
  argv = process.argv.slice(2),
  {
    stdout = process.stdout,
    stderr = process.stderr,
    execFile = execFileAsync,
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

    const branch = args.branch ?? (await readCurrentBranch({ execFile, cwd, env }));
    const result = await cleanupMergedWorktree({
      branch,
      worktree: args.worktree,
      pr: args.pr,
      forceDirty: args.forceDirty,
      dryRun: args.dryRun,
      json: args.json,
      stdout,
      execFile,
      cwd,
      env
    });
    if (args.json) {
      stdout.write(`${JSON.stringify(result, null, 2)}\n`);
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
