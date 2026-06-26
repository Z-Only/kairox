import assert from "node:assert/strict";
import test from "node:test";

import {
  auditEvalWorktrees,
  filterAuditResults,
  filterEvalWorktrees,
  formatHumanTable,
  parseWorktreePorcelain,
  runCli,
  summarizeAudit
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

test("summarizeAudit counts total and dirty status buckets", () => {
  assert.deepEqual(
    summarizeAudit([
      { dirty_status: "clean" },
      { dirty_status: "dirty" },
      { dirty_status: "missing" },
      { dirty_status: "error" },
      { dirty_status: "dirty" }
    ]),
    {
      total: 5,
      clean: 1,
      dirty: 2,
      missing: 1,
      error: 1
    }
  );
});

test("filterAuditResults applies dirty and clean filters", () => {
  const worktrees = [
    { path: "/clean", dirty_status: "clean" },
    { path: "/dirty", dirty_status: "dirty" },
    { path: "/missing", dirty_status: "missing" },
    { path: "/error", dirty_status: "error" }
  ];

  assert.deepEqual(filterAuditResults(worktrees), worktrees);
  assert.deepEqual(filterAuditResults(worktrees, { dirtyOnly: true }), [
    { path: "/dirty", dirty_status: "dirty" },
    { path: "/missing", dirty_status: "missing" },
    { path: "/error", dirty_status: "error" }
  ]);
  assert.deepEqual(filterAuditResults(worktrees, { cleanOnly: true }), [
    { path: "/clean", dirty_status: "clean" }
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
        return { stdout: " M crates/agent-runtime/src/lib.rs\n?? scratch.txt\n" };
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
      path_exists: true,
      dirty_file_count: 0,
      dirty_files: []
    },
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333",
      dirty_status: "dirty",
      path_exists: true,
      dirty_file_count: 2,
      dirty_files: ["crates/agent-runtime/src/lib.rs", "scratch.txt"]
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555",
      dirty_status: "missing",
      path_exists: false,
      dirty_file_count: 0,
      dirty_files: []
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

test("auditEvalWorktrees reports empty dirty file details when status lookup errors", async () => {
  const existingPaths = new Set([
    "/repo/.worktrees/eval-a",
    "/repo/.worktrees/eval-kairox-b",
    "/repo/.worktrees/eval-kairox-detached"
  ]);
  const audited = await auditEvalWorktrees({
    execFile: async (command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        throw new Error("status failed");
      }
      if (command === "git" && args.includes("status")) {
        return { stdout: "" };
      }
      throw new Error(`unexpected command: ${command} ${args.join(" ")}`);
    },
    pathExists: (path) => existingPaths.has(path)
  });

  assert.deepEqual(audited[1], {
    path: "/repo/.worktrees/eval-kairox-b",
    branch: "codex/not-eval",
    head: "3333333333333333333333333333333333333333",
    dirty_status: "error",
    path_exists: true,
    dirty_file_count: 0,
    dirty_files: []
  });
});

test("auditEvalWorktrees caps dirty file details while counting every status line", async () => {
  const existingPaths = new Set(["/repo/.worktrees/eval-kairox-b"]);
  const audited = await auditEvalWorktrees({
    execFile: async (_command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return {
          stdout: [
            " M one.txt",
            " M two.txt",
            " M three.txt",
            " M four.txt",
            " M five.txt",
            " M six.txt"
          ].join("\n")
        };
      }
      throw new Error(`unexpected command: ${args.join(" ")}`);
    },
    pathExists: (path) => existingPaths.has(path)
  });

  assert.equal(audited[1].dirty_file_count, 6);
  assert.deepEqual(audited[1].dirty_files, [
    "one.txt",
    "two.txt",
    "three.txt",
    "four.txt",
    "five.txt"
  ]);
});

test("runCli annotates dirty files that match a compare ref", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--json", "--compare-ref", "origin/main"], {
    stdout,
    stderr,
    pathExists: (path) => path !== "/repo/.worktrees/eval-kairox-detached",
    execFile: async (_command, args) => {
      const joined = args.join(" ");
      if (joined === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (joined === "-C /repo/.worktrees/eval-a status --short") {
        return { stdout: "" };
      }
      if (joined === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return {
          stdout: [" M crates/agent-runtime/src/lib.rs", " M different.txt", "?? scratch.txt"].join(
            "\n"
          )
        };
      }
      if (
        joined ===
        "-C /repo/.worktrees/eval-kairox-b rev-parse origin/main:crates/agent-runtime/src/lib.rs"
      ) {
        return { stdout: "same-hash\n" };
      }
      if (
        joined ===
        "-C /repo/.worktrees/eval-kairox-b hash-object -- crates/agent-runtime/src/lib.rs"
      ) {
        return { stdout: "same-hash\n" };
      }
      if (joined === "-C /repo/.worktrees/eval-kairox-b rev-parse origin/main:different.txt") {
        return { stdout: "ref-hash\n" };
      }
      if (joined === "-C /repo/.worktrees/eval-kairox-b hash-object -- different.txt") {
        return { stdout: "worktree-hash\n" };
      }
      if (joined === "-C /repo/.worktrees/eval-kairox-b rev-parse origin/main:scratch.txt") {
        throw new Error("not in ref");
      }
      throw new Error(`unexpected command: ${joined}`);
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  const result = JSON.parse(stdout.content);
  assert.deepEqual(result.worktrees[1], {
    path: "/repo/.worktrees/eval-kairox-b",
    branch: "codex/not-eval",
    head: "3333333333333333333333333333333333333333",
    dirty_status: "dirty",
    path_exists: true,
    dirty_file_count: 3,
    dirty_files: ["crates/agent-runtime/src/lib.rs", "different.txt", "scratch.txt"],
    compare_ref: "origin/main",
    compare_ref_checked_count: 2,
    compare_ref_match_count: 1,
    compare_ref_matching_files: ["crates/agent-runtime/src/lib.rs"],
    compare_ref_unmatched_count: 2,
    compare_ref_unmatched_files: ["different.txt", "scratch.txt"]
  });
});

test("formatHumanTable emits a stable table with path, branch, head, exists, dirty, and dirty files columns", () => {
  const table = formatHumanTable([
    {
      path: "/repo/.worktrees/eval-a",
      branch: "eval/a",
      head: "2222222222222222222222222222222222222222",
      dirty_status: "clean",
      path_exists: true,
      dirty_file_count: 0,
      dirty_files: []
    },
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333",
      dirty_status: "dirty",
      path_exists: true,
      dirty_file_count: 2,
      dirty_files: ["crates/agent-runtime/src/lib.rs", "scratch.txt"]
    },
    {
      path: "/repo/.worktrees/eval-kairox-detached",
      branch: null,
      head: "5555555555555555555555555555555555555555",
      dirty_status: "missing",
      path_exists: false,
      dirty_file_count: 0,
      dirty_files: []
    }
  ]);

  assert.match(table, /^Summary: total=3 clean=1 dirty=1 missing=1 error=0$/m);
  assert.match(table, /^PATH\s+BRANCH\s+HEAD\s+PATH_EXISTS\s+DIRTY_STATUS\s+DIRTY_FILES/m);
  assert.match(table, /\/repo\/\.worktrees\/eval-a\s+eval\/a\s+222222222222\s+yes\s+clean\s+-/);
  assert.match(
    table,
    /\/repo\/\.worktrees\/eval-kairox-b\s+codex\/not-eval\s+333333333333\s+yes\s+dirty\s+2: crates\/agent-runtime\/src\/lib\.rs, scratch\.txt/
  );
  assert.match(
    table,
    /\/repo\/\.worktrees\/eval-kairox-detached\s+-\s+555555555555\s+no\s+missing\s+-/
  );
});

test("formatHumanTable includes compare ref matches when present", () => {
  const table = formatHumanTable([
    {
      path: "/repo/.worktrees/eval-kairox-b",
      branch: "codex/not-eval",
      head: "3333333333333333333333333333333333333333",
      dirty_status: "dirty",
      path_exists: true,
      dirty_file_count: 2,
      dirty_files: ["same.txt", "different.txt"],
      compare_ref: "origin/main",
      compare_ref_checked_count: 2,
      compare_ref_match_count: 1,
      compare_ref_matching_files: ["same.txt"],
      compare_ref_unmatched_count: 1,
      compare_ref_unmatched_files: ["different.txt"]
    }
  ]);

  assert.match(
    table,
    /^PATH\s+BRANCH\s+HEAD\s+PATH_EXISTS\s+DIRTY_STATUS\s+DIRTY_FILES\s+COMPARE_REF_MATCHES/m
  );
  assert.match(table, /origin\/main 1\/2: same\.txt/);
  assert.match(table, /unmatched 1: different\.txt/);
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
  assert.deepEqual(JSON.parse(stdout.content), {
    summary: {
      total: 3,
      clean: 2,
      dirty: 0,
      missing: 1,
      error: 0
    },
    worktrees: [
      {
        path: "/repo/.worktrees/eval-a",
        branch: "eval/a",
        head: "2222222222222222222222222222222222222222",
        dirty_status: "clean",
        path_exists: true,
        dirty_file_count: 0,
        dirty_files: []
      },
      {
        path: "/repo/.worktrees/eval-kairox-b",
        branch: "codex/not-eval",
        head: "3333333333333333333333333333333333333333",
        dirty_status: "clean",
        path_exists: true,
        dirty_file_count: 0,
        dirty_files: []
      },
      {
        path: "/repo/.worktrees/eval-kairox-detached",
        branch: null,
        head: "5555555555555555555555555555555555555555",
        dirty_status: "missing",
        path_exists: false,
        dirty_file_count: 0,
        dirty_files: []
      }
    ]
  });
});

test("runCli filters JSON output with --dirty-only", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--json", "--dirty-only"], {
    stdout,
    stderr,
    pathExists: (path) => path !== "/repo/.worktrees/eval-kairox-detached",
    execFile: async (_command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return { stdout: " M changed.txt\n" };
      }
      return { stdout: "" };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(JSON.parse(stdout.content), {
    summary: {
      total: 2,
      clean: 0,
      dirty: 1,
      missing: 1,
      error: 0
    },
    worktrees: [
      {
        path: "/repo/.worktrees/eval-kairox-b",
        branch: "codex/not-eval",
        head: "3333333333333333333333333333333333333333",
        dirty_status: "dirty",
        path_exists: true,
        dirty_file_count: 1,
        dirty_files: ["changed.txt"]
      },
      {
        path: "/repo/.worktrees/eval-kairox-detached",
        branch: null,
        head: "5555555555555555555555555555555555555555",
        dirty_status: "missing",
        path_exists: false,
        dirty_file_count: 0,
        dirty_files: []
      }
    ]
  });
});

test("runCli filters JSON output with --clean-only", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--json", "--clean-only"], {
    stdout,
    stderr,
    pathExists: (path) => path !== "/repo/.worktrees/eval-kairox-detached",
    execFile: async (_command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return { stdout: " M changed.txt\n" };
      }
      return { stdout: "" };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(JSON.parse(stdout.content), {
    summary: {
      total: 1,
      clean: 1,
      dirty: 0,
      missing: 0,
      error: 0
    },
    worktrees: [
      {
        path: "/repo/.worktrees/eval-a",
        branch: "eval/a",
        head: "2222222222222222222222222222222222222222",
        dirty_status: "clean",
        path_exists: true,
        dirty_file_count: 0,
        dirty_files: []
      }
    ]
  });
});

test("runCli prints only the summary with --summary", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--summary"], {
    stdout,
    stderr,
    pathExists: (path) => path !== "/repo/.worktrees/eval-kairox-detached",
    execFile: async (_command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return { stdout: " M changed.txt\n" };
      }
      return { stdout: "" };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(stdout.content, "Summary: total=3 clean=1 dirty=1 missing=1 error=0\n");
});

test("runCli prints stable summary JSON with --json --summary", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--json", "--summary", "--dirty-only"], {
    stdout,
    stderr,
    pathExists: (path) => path !== "/repo/.worktrees/eval-kairox-detached",
    execFile: async (_command, args) => {
      if (args.join(" ") === "worktree list --porcelain") {
        return { stdout: PORCELAIN };
      }
      if (args.join(" ") === "-C /repo/.worktrees/eval-kairox-b status --short") {
        return { stdout: " M changed.txt\n" };
      }
      return { stdout: "" };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(JSON.parse(stdout.content), {
    summary: {
      total: 2,
      clean: 0,
      dirty: 1,
      missing: 1,
      error: 0
    }
  });
});

test("runCli rejects conflicting dirty-only and clean-only filters", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  let invoked = false;

  const exitCode = await runCli(["--dirty-only", "--clean-only"], {
    stdout,
    stderr,
    execFile: async () => {
      invoked = true;
      throw new Error("git should not be invoked for invalid filters");
    }
  });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.equal(invoked, false);
  assert.match(stderr.content, /Error: --dirty-only and --clean-only cannot be used together/);
  assert.match(stderr.content, /Usage: node scripts\/audit-eval-worktrees\.mjs/);
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
  assert.match(
    stdout.content,
    /Usage: node scripts\/audit-eval-worktrees\.mjs \[--json\] \[--summary\] \[--dirty-only\|--clean-only\] \[--compare-ref <ref>\]/
  );
});
