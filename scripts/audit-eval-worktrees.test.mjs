import assert from "node:assert/strict";
import test from "node:test";

import {
  auditEvalWorktrees,
  filterEvalWorktrees,
  formatHumanTable,
  parseWorktreePorcelain,
  runCli
} from "./audit-eval-worktrees.mjs";

const PORCELAIN = `worktree /repo
HEAD 1111111111111111111111111111111111111111
branch refs/heads/main

worktree /repo/.worktrees/eval-a
HEAD 2222222222222222222222222222222222222222
branch refs/heads/eval/a

worktree /repo/.worktrees/eval-kairox-b
HEAD 3333333333333333333333333333333333333333
branch refs/heads/codex/not-eval

worktree /repo/.worktrees/codex-c
HEAD 4444444444444444444444444444444444444444
branch refs/heads/codex/c

worktree /repo/.worktrees/eval-kairox-detached
HEAD 5555555555555555555555555555555555555555
detached
`;

function createWritableCapture() {
  return {
    content: "",
    write(chunk) {
      this.content += chunk;
    }
  };
}

test("parseWorktreePorcelain parses paths, branches, heads, and detached worktrees", () => {
  assert.deepEqual(parseWorktreePorcelain(PORCELAIN), [
    {
      path: "/repo",
      branch: "main",
      head: "1111111111111111111111111111111111111111"
    },
    {
      path: "/repo/.worktrees/eval-a",
      branch: "eval/a",
      head: "2222222222222222222222222222222222222222"
    },
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333"
    },
    {
      path: "/repo/.worktrees/codex-c",
      branch: "codex/c",
      head: "4444444444444444444444444444444444444444"
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555"
    }
  ]);
});

test("filterEvalWorktrees selects eval branches or eval-kairox path basenames", () => {
  assert.deepEqual(filterEvalWorktrees(parseWorktreePorcelain(PORCELAIN)), [
    {
      path: "/repo/.worktrees/eval-a",
      branch: "eval/a",
      head: "2222222222222222222222222222222222222222"
    },
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333"
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555"
    }
  ]);
});

test("auditEvalWorktrees marks dirty, clean, and missing worktrees without deleting anything", async () => {
  const commands = [];
  const existingPaths = new Set(["/repo/.worktrees/eval-a", "/repo/.worktrees/eval-kairox-b"]);
  const audited = await auditEvalWorktrees({
    execFile: async (command, args) => {
      commands.push([command, args]);
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-a status --short") {
        return { stdout: "" };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return { stdout: " M crates/agent-eval/src/main.rs\n?? scratch.txt\n" };
      }
      throw new Error(`unexpected command: ${command} ${args.join(" ")}`);
    },
    pathExists: (path) => existingPaths.has(path)
  });

  assert.deepEqual(audited, [
    {
      path: "/repo/.worktrees/eval-a",
      branch: "eval/a",
      head: "2222222222222222222222222222222222222222",
      dirty_status: "clean",
      path_exists: true
    },
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333",
      dirty_status: "dirty",
      path_exists: true
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555",
      dirty_status: "missing",
      path_exists: false
    }
  ]);
  assert.deepEqual(
    commands.map(([command, args]) => `${command} ${args.join(" ")}`),
    [
      "git worktree list --porcelain",
      "git -C /repo/.worktrees/eval-a status --short",
      "git -C /repo/.worktrees/eval-kairox-b status --short"
    ]
  );
  for (const [, args] of commands) {
    assert(!args.includes("remove"));
    assert(!args.includes("delete"));
  }
});

test("formatHumanTable emits a stable table with path, branch, head, exists, and dirty columns", () => {
  const table = formatHumanTable([
    {
      path: "/repo/.worktrees/eval-a",
      branch: "eval/a",
      head: "2222222222222222222222222222222222222222",
      dirty_status: "clean",
      path_exists: true
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555",
      dirty_status: "missing",
      path_exists: false
    }
  ]);

  assert.match(table, /^PATH\s+BRANCH\s+HEAD\s+PATH_EXISTS\s+DIRTY_STATUS/m);
  assert.match(table, /\/repo\/\.worktrees\/eval-a\s+eval\/a\s+222222222222\s+yes\s+clean/);
  assert.match(
    table,
    /\/repo\/\.worktrees\/eval-kairox-detached\s+-\s+555555555555\s+no\s+missing/
  );
});

test("runCli writes stable JSON without touching the real Git repository", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--json"], {
    stdout,
    stderr,
    pathExists: (path) => path !== "/repo/.worktrees/eval-kairox-detached",
    execFile: async (_command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      return { stdout: "" };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(JSON.parse(stdout.content), [
    {
      path: "/repo/.worktrees/eval-a",
      branch: "eval/a",
      head: "2222222222222222222222222222222222222222",
      dirty_status: "clean",
      path_exists: true
    },
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333",
      dirty_status: "clean",
      path_exists: true
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555",
      dirty_status: "missing",
      path_exists: false
    }
  ]);
});

test("runCli prints help without invoking git", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  let invoked = false;

  const exitCode = await runCli(["--help"], {
    stdout,
    stderr,
    execFile: async () => {
      invoked = true;
      throw new Error("git should not be invoked for help");
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(invoked, false);
  assert.match(stdout.content, /Usage: node scripts\/audit-eval-worktrees\.mjs \[--json\]/);
});
