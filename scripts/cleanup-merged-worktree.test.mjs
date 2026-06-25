import assert from "node:assert/strict";
import test from "node:test";

import { runCli } from "./cleanup-merged-worktree.mjs";

const WORKTREES = `worktree /repo
HEAD 1111111111111111111111111111111111111111
branch refs/heads/main

worktree /repo/.worktrees/feature-x
HEAD 2222222222222222222222222222222222222222
branch refs/heads/feature/x
`;

function createWritableCapture() {
  return {
    content: "",
    write(chunk) {
      this.content += chunk;
    }
  };
}

function createMergedPr(overrides = {}) {
  return {
    number: 1099,
    state: "MERGED",
    headRefName: "feature/x",
    headRefOid: "2222222222222222222222222222222222222222",
    mergeCommit: { oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
    ...overrides
  };
}

function commandLine(command, args) {
  return `${command} ${args.join(" ")}`;
}

test("runCli removes worktree before force-deleting a squash-merged local branch", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const commands = [];

  const exitCode = await runCli(["--branch", "feature/x"], {
    stdout,
    stderr,
    cwd: "/repo",
    execFile: async (command, args) => {
      commands.push([command, args]);
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      if (line === "git -C /repo/.worktrees/feature-x status --short") {
        return { stdout: "" };
      }
      if (line === "git worktree remove /repo/.worktrees/feature-x") {
        return { stdout: "" };
      }
      if (line === "git worktree prune") {
        return { stdout: "" };
      }
      if (line === "git branch -D feature/x") {
        return { stdout: "Deleted branch feature/x.\n" };
      }
      if (line === "git push origin --delete feature/x") {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.match(stdout.content, /removed worktree: \/repo\/\.worktrees\/feature-x/);
  assert.match(stdout.content, /deleted local branch: feature\/x/);
  assert.match(stdout.content, /deleted remote branch: feature\/x/);

  const lines = commands.map(([command, args]) => commandLine(command, args));
  assert(
    lines.indexOf("git worktree remove /repo/.worktrees/feature-x") <
      lines.indexOf("git branch -D feature/x"),
    "worktree must be removed before deleting the branch that owns it"
  );
});

test("runCli uses the primary repository root when invoked from a linked worktree", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--branch", "feature/x"], {
    stdout,
    stderr,
    cwd: "/repo/.worktrees/tooling",
    execFile: async (command, args) => {
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo/.worktrees/tooling\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      if (line === "git -C /repo/.worktrees/feature-x status --short") {
        return { stdout: "" };
      }
      if (line === "git worktree remove /repo/.worktrees/feature-x") {
        return { stdout: "" };
      }
      if (line === "git worktree prune") {
        return { stdout: "" };
      }
      if (line === "git branch -D feature/x") {
        return { stdout: "Deleted branch feature/x.\n" };
      }
      if (line === "git push origin --delete feature/x") {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.match(stdout.content, /removed worktree: \/repo\/\.worktrees\/feature-x/);
});

test("runCli refuses cleanup when GitHub does not report a merged PR", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const commands = [];

  const exitCode = await runCli(["--branch", "feature/x"], {
    stdout,
    stderr,
    cwd: "/repo",
    execFile: async (command, args) => {
      commands.push([command, args]);
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr({ state: "OPEN", mergeCommit: null })) };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.match(stderr.content, /PR for feature\/x is not merged/);
  assert(!commands.some(([, args]) => args.includes("remove")));
  assert(!commands.some(([, args]) => args.includes("-D")));
});

test("runCli refuses worktree paths outside the repository .worktrees directory", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const commands = [];

  const exitCode = await runCli(["--branch", "feature/x", "--worktree", "/tmp/feature-x"], {
    stdout,
    stderr,
    cwd: "/repo",
    execFile: async (command, args) => {
      commands.push([command, args]);
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.match(stderr.content, /Refusing to remove worktree outside \/repo\/\.worktrees/);
  assert(!commands.some(([, args]) => args.includes("remove")));
  assert(!commands.some(([, args]) => args.includes("-D")));
});

test("runCli refuses dirty worktrees unless force-dirty is supplied", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const commands = [];

  const exitCode = await runCli(["--branch", "feature/x"], {
    stdout,
    stderr,
    cwd: "/repo",
    execFile: async (command, args) => {
      commands.push([command, args]);
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      if (line === "git -C /repo/.worktrees/feature-x status --short") {
        return { stdout: " M scripts/example.mjs\n" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.match(stderr.content, /Worktree is dirty/);
  assert(!commands.some(([, args]) => args.includes("remove")));
  assert(!commands.some(([, args]) => args.includes("-D")));
});

test("runCli dry-run reports planned cleanup without destructive git commands", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const commands = [];

  const exitCode = await runCli(["--branch", "feature/x", "--dry-run"], {
    stdout,
    stderr,
    cwd: "/repo",
    execFile: async (command, args) => {
      commands.push([command, args]);
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      if (line === "git -C /repo/.worktrees/feature-x status --short") {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.match(stdout.content, /dry-run: planned cleanup for feature\/x/);
  assert.match(stdout.content, /dry-run: would remove worktree: \/repo\/\.worktrees\/feature-x/);
  assert.match(stdout.content, /dry-run: would delete local branch: feature\/x/);
  assert.match(stdout.content, /dry-run: would delete remote branch: feature\/x/);

  const lines = commands.map(([command, args]) => commandLine(command, args));
  assert(!lines.includes("git worktree remove /repo/.worktrees/feature-x"));
  assert(!lines.includes("git worktree prune"));
  assert(!lines.includes("git branch -D feature/x"));
  assert(!lines.includes("git push origin --delete feature/x"));
});

test("runCli dry-run json writes a parseable cleanup action plan only to stdout", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--branch", "feature/x", "--dry-run", "--json"], {
    stdout,
    stderr,
    cwd: "/repo",
    execFile: async (command, args) => {
      const line = commandLine(command, args);
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      if (line === "git -C /repo/.worktrees/feature-x status --short") {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.doesNotThrow(() => JSON.parse(stdout.content));
  const plan = JSON.parse(stdout.content);
  assert.equal(plan.branch, "feature/x");
  assert.equal(plan.worktree_path, "/repo/.worktrees/feature-x");
  assert.equal(plan.pr_number, 1099);
  assert.equal(plan.merge_commit_oid, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  assert.equal(plan.dry_run, true);
  assert.deepEqual(
    plan.actions.map((action) => action.status),
    ["planned", "planned", "planned", "planned"]
  );
  assert.deepEqual(
    plan.actions.map((action) => action.action),
    ["remove_worktree", "prune_worktrees", "delete_local_branch", "delete_remote_branch"]
  );
});

test("runCli infers the branch from the current git branch when branch is omitted", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const commands = [];

  const exitCode = await runCli(["--dry-run"], {
    stdout,
    stderr,
    cwd: "/repo/.worktrees/feature-x",
    execFile: async (command, args) => {
      commands.push([command, args]);
      const line = commandLine(command, args);
      if (line === "git branch --show-current") {
        return { stdout: "feature/x\n" };
      }
      if (line === "git rev-parse --show-toplevel") {
        return { stdout: "/repo/.worktrees/feature-x\n" };
      }
      if (line === "git rev-parse --git-common-dir") {
        return { stdout: "/repo/.git\n" };
      }
      if (line === "git worktree list --porcelain") {
        return { stdout: WORKTREES };
      }
      if (line === "gh pr view feature/x --json number,state,mergeCommit,headRefName,headRefOid") {
        return { stdout: JSON.stringify(createMergedPr()) };
      }
      if (line === "git fetch origin main") {
        return { stdout: "" };
      }
      if (
        line === "git merge-base --is-ancestor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa origin/main"
      ) {
        return { stdout: "" };
      }
      if (line === "git -C /repo/.worktrees/feature-x status --short") {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${line}`);
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.match(stdout.content, /dry-run: planned cleanup for feature\/x/);
  assert(
    commands.some(([command, args]) => commandLine(command, args) === "git branch --show-current")
  );
});
