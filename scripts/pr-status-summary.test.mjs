import assert from "node:assert/strict";
import test from "node:test";

import {
  formatHumanSummary,
  normalizeStatusCheckRollup,
  runCli,
  summarizePullRequest
} from "./pr-status-summary.mjs";

function createWritableCapture() {
  return {
    content: "",
    write(chunk) {
      this.content += chunk;
    }
  };
}

function samplePullRequest(overrides = {}) {
  return {
    number: 42,
    title: "Add watcher status summary",
    state: "OPEN",
    mergeStateStatus: "BLOCKED",
    headRefName: "codex/pr-status-summary",
    headRefOid: "abcdef1234567890",
    mergeCommit: { oid: "feedface9876543210" },
    statusCheckRollup: [
      { name: "lint", status: "COMPLETED", conclusion: "SUCCESS" },
      { name: "unit", status: "COMPLETED", conclusion: "FAILURE" },
      { name: "build", status: "IN_PROGRESS", conclusion: null },
      { context: "branch protection", state: "SUCCESS" },
      { name: "docs", status: "COMPLETED", conclusion: "SKIPPED" },
      { name: "mystery", status: "COMPLETED", conclusion: "NEUTRAL" }
    ],
    ...overrides
  };
}

test("normalizeStatusCheckRollup counts outcomes and preserves raw fields", () => {
  const normalized = normalizeStatusCheckRollup(samplePullRequest().statusCheckRollup);

  assert.deepEqual(normalized.counts, {
    success: 2,
    failure: 1,
    pending: 1,
    skipped: 1,
    unknown: 1
  });
  assert.deepEqual(normalized.checks, [
    {
      name: "lint",
      status: "COMPLETED",
      conclusion: "SUCCESS",
      state: null,
      result: "success"
    },
    {
      name: "unit",
      status: "COMPLETED",
      conclusion: "FAILURE",
      state: null,
      result: "failure"
    },
    {
      name: "build",
      status: "IN_PROGRESS",
      conclusion: null,
      state: null,
      result: "pending"
    },
    {
      name: "branch protection",
      status: null,
      conclusion: null,
      state: "SUCCESS",
      result: "success"
    },
    {
      name: "docs",
      status: "COMPLETED",
      conclusion: "SKIPPED",
      state: null,
      result: "skipped"
    },
    {
      name: "mystery",
      status: "COMPLETED",
      conclusion: "NEUTRAL",
      state: null,
      result: "unknown"
    }
  ]);
});

test("summarizePullRequest emits stable snake_case fields", () => {
  assert.deepEqual(summarizePullRequest(samplePullRequest()), {
    number: 42,
    title: "Add watcher status summary",
    state: "OPEN",
    merge_state_status: "BLOCKED",
    head_ref_name: "codex/pr-status-summary",
    head_ref_oid: "abcdef1234567890",
    merge_commit_oid: "feedface9876543210",
    checks: {
      counts: {
        success: 2,
        failure: 1,
        pending: 1,
        skipped: 1,
        unknown: 1
      },
      items: [
        {
          name: "lint",
          status: "COMPLETED",
          conclusion: "SUCCESS",
          state: null,
          result: "success"
        },
        {
          name: "unit",
          status: "COMPLETED",
          conclusion: "FAILURE",
          state: null,
          result: "failure"
        },
        {
          name: "build",
          status: "IN_PROGRESS",
          conclusion: null,
          state: null,
          result: "pending"
        },
        {
          name: "branch protection",
          status: null,
          conclusion: null,
          state: "SUCCESS",
          result: "success"
        },
        {
          name: "docs",
          status: "COMPLETED",
          conclusion: "SKIPPED",
          state: null,
          result: "skipped"
        },
        {
          name: "mystery",
          status: "COMPLETED",
          conclusion: "NEUTRAL",
          state: null,
          result: "unknown"
        }
      ]
    }
  });
});

test("runCli prints stable JSON and invokes gh pr view with required fields", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const calls = [];

  const exitCode = await runCli(["--json", "42"], {
    stdout,
    stderr,
    execFile: async (command, args) => {
      calls.push([command, args]);
      return { stdout: JSON.stringify(samplePullRequest()) };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(calls, [
    [
      "gh",
      [
        "pr",
        "view",
        "42",
        "--json",
        "number,title,state,mergeStateStatus,headRefName,headRefOid,mergeCommit,statusCheckRollup"
      ]
    ]
  ]);
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(samplePullRequest())]
      },
      null,
      2
    )}\n`
  );
});

test("formatHumanSummary renders a readable table with counts and check rows", () => {
  const output = formatHumanSummary([summarizePullRequest(samplePullRequest())]);

  assert.match(
    output,
    /PR\s+State\s+Merge\s+Head\s+Success\s+Failure\s+Pending\s+Skipped\s+Unknown\s+Title/
  );
  assert.match(
    output,
    /#42\s+OPEN\s+BLOCKED\s+codex\/pr-status-summary@abcdef12\s+2\s+1\s+1\s+1\s+1\s+Add watcher status summary/
  );
  assert.match(output, /Checks for #42/);
  assert.match(output, /lint\s+COMPLETED\s+SUCCESS\s+-\s+success/);
  assert.match(output, /branch protection\s+-\s+-\s+SUCCESS\s+success/);
});

test("runCli reports missing PR number as usage error", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli([], { stdout, stderr, execFile: async () => assert.fail() });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.match(stderr.content, /Missing required PR number/);
  assert.match(stderr.content, /Usage: node scripts\/pr-status-summary\.mjs/);
});

test("runCli reports missing gh clearly", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const error = new Error("spawn gh ENOENT");
  error.code = "ENOENT";

  const exitCode = await runCli(["42"], {
    stdout,
    stderr,
    execFile: async () => {
      throw error;
    }
  });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.match(stderr.content, /gh was not found on PATH/);
});

test("runCli reports PR view failures with gh stderr", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const error = new Error("Command failed");
  error.code = 1;
  error.stderr = "could not resolve to a PullRequest";

  const exitCode = await runCli(["9999"], {
    stdout,
    stderr,
    execFile: async () => {
      throw error;
    }
  });

  assert.equal(exitCode, 1);
  assert.equal(stdout.content, "");
  assert.match(stderr.content, /gh pr view 9999 failed \(exit 1\)/);
  assert.match(stderr.content, /could not resolve to a PullRequest/);
});
