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
    neutral: 1,
    unknown: 0
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
      result: "neutral"
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
        neutral: 1,
        unknown: 0
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
          result: "neutral"
        }
      ]
    }
  });
});

test("summarizePullRequest classifies non-action review bot metadata comments", () => {
  assert.deepEqual(
    summarizePullRequest(
      samplePullRequest({
        comments: [
          {
            author: { login: "qodo-code-review" },
            body: "### Qodo reviews are paused for this user."
          },
          {
            author: { login: "coderabbitai" },
            body: "## Review limit reached\nMore reviews will be available later."
          },
          {
            author: { login: "coderabbitai" },
            body: "## Review failed\n\nThe pull request is closed."
          },
          {
            author: { login: "alice" },
            body: "Please rename this helper."
          }
        ]
      })
    ).review_notes,
    {
      non_action_bot_comment_count: 3,
      non_action_bot_comments: [
        { author: "qodo-code-review", reason: "paused_review" },
        { author: "coderabbitai", reason: "rate_limited_review" },
        { author: "coderabbitai", reason: "closed_pr_review" }
      ]
    }
  );
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
        "number,title,state,mergeStateStatus,headRefName,headRefOid,mergeCommit,statusCheckRollup,comments,reviews"
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

test("runCli watch waits until every PR has no pending or failing checks", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const calls = [];
  const sleeps = [];
  let currentTime = 0;

  const pendingPullRequest = samplePullRequest({
    statusCheckRollup: [{ name: "build", status: "IN_PROGRESS", conclusion: null }]
  });
  const readyPullRequest = samplePullRequest({
    mergeStateStatus: "CLEAN",
    statusCheckRollup: [{ name: "build", status: "COMPLETED", conclusion: "SUCCESS" }]
  });

  const exitCode = await runCli(
    ["--watch", "--interval-ms", "5", "--timeout-ms", "2000", "--json", "42"],
    {
      stdout,
      stderr,
      now: () => currentTime,
      sleep: async (ms) => {
        sleeps.push(ms);
        currentTime += ms;
      },
      execFile: async (command, args) => {
        calls.push([command, args]);
        return {
          stdout: JSON.stringify(calls.length === 1 ? pendingPullRequest : readyPullRequest)
        };
      }
    }
  );

  assert.equal(exitCode, 0);
  assert.match(
    stderr.content,
    /watch #42 abcdef12 merge=BLOCKED success=0 failure=0 pending=1 unknown=0 pending_checks="build"/
  );
  assert.deepEqual(sleeps, [5]);
  assert.equal(calls.length, 2);
  assert.deepEqual(
    calls.map(([, args]) => args.at(-1)),
    [
      "number,title,state,mergeStateStatus,headRefName,headRefOid,mergeCommit,statusCheckRollup",
      "number,title,state,mergeStateStatus,headRefName,headRefOid,mergeCommit,statusCheckRollup"
    ]
  );
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(readyPullRequest)]
      },
      null,
      2
    )}\n`
  );
});

test("runCli watch prints a heartbeat while checks remain pending", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const sleeps = [];
  let currentTime = 0;

  const pendingPullRequest = samplePullRequest({
    mergeStateStatus: "BLOCKED",
    statusCheckRollup: [
      { name: "Format", status: "COMPLETED", conclusion: "SUCCESS" },
      { name: "Coverage (Rust)", status: "IN_PROGRESS", conclusion: null }
    ]
  });
  const readyPullRequest = samplePullRequest({
    mergeStateStatus: "CLEAN",
    statusCheckRollup: [{ name: "Format", status: "COMPLETED", conclusion: "SUCCESS" }]
  });

  const responses = [pendingPullRequest, readyPullRequest];

  const exitCode = await runCli(
    ["--watch", "--interval-ms", "5", "--timeout-ms", "2000", "--json", "42"],
    {
      stdout,
      stderr,
      now: () => currentTime,
      sleep: async (ms) => {
        sleeps.push(ms);
        currentTime += ms;
      },
      execFile: async () => ({ stdout: JSON.stringify(responses.shift()) })
    }
  );

  assert.equal(exitCode, 0);
  assert.deepEqual(sleeps, [5]);
  assert.match(
    stderr.content,
    /watch #42 abcdef12 merge=BLOCKED success=1 failure=0 pending=1 unknown=0 pending_checks="Coverage \(Rust\)"/
  );
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(readyPullRequest)]
      },
      null,
      2
    )}\n`
  );
});

test("runCli watch retries transient gh query failures", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const calls = [];
  const sleeps = [];
  let currentTime = 0;

  const transient = new Error("Command failed");
  transient.code = 1;
  transient.stderr = 'Post "https://api.github.com/graphql": Service Unavailable';

  const pendingPullRequest = samplePullRequest({
    statusCheckRollup: [{ name: "Coverage (Rust)", status: "IN_PROGRESS", conclusion: null }]
  });
  const readyPullRequest = samplePullRequest({
    mergeStateStatus: "CLEAN",
    statusCheckRollup: [{ name: "Coverage (Rust)", status: "COMPLETED", conclusion: "SUCCESS" }]
  });

  const exitCode = await runCli(
    ["--watch", "--interval-ms", "5", "--timeout-ms", "2000", "--json", "42"],
    {
      stdout,
      stderr,
      now: () => currentTime,
      sleep: async (ms) => {
        sleeps.push(ms);
        currentTime += ms;
      },
      execFile: async () => {
        calls.push("gh");
        if (calls.length === 1) {
          throw transient;
        }
        return {
          stdout: JSON.stringify(calls.length === 2 ? pendingPullRequest : readyPullRequest)
        };
      }
    }
  );

  assert.equal(exitCode, 0);
  assert.equal(calls.length, 3);
  assert.deepEqual(sleeps, [1_000, 5]);
  assert.match(stderr.content, /transient gh failure for #42; retrying in 1000ms/);
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(readyPullRequest)]
      },
      null,
      2
    )}\n`
  );
});

test("runCli watch exits 1 immediately when any PR has failing checks", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const sleeps = [];
  const failingPullRequest = samplePullRequest({
    statusCheckRollup: [{ name: "unit", status: "COMPLETED", conclusion: "FAILURE" }]
  });

  const exitCode = await runCli(
    ["--watch", "--interval-ms", "5", "--timeout-ms", "100", "--json", "42"],
    {
      stdout,
      stderr,
      now: () => 0,
      sleep: async (ms) => sleeps.push(ms),
      execFile: async () => ({ stdout: JSON.stringify(failingPullRequest) })
    }
  );

  assert.equal(exitCode, 1);
  assert.equal(stderr.content, "");
  assert.deepEqual(sleeps, []);
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(failingPullRequest)]
      },
      null,
      2
    )}\n`
  );
});

test("runCli watch exits 1 on timeout and prints the last summary", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const sleeps = [];
  let currentTime = 0;
  const pendingPullRequest = samplePullRequest({
    statusCheckRollup: [{ name: "build", status: "IN_PROGRESS", conclusion: null }]
  });

  const exitCode = await runCli(
    ["--watch", "--interval-ms", "5", "--timeout-ms", "10", "--json", "42"],
    {
      stdout,
      stderr,
      now: () => currentTime,
      sleep: async (ms) => {
        sleeps.push(ms);
        currentTime += ms;
      },
      execFile: async () => ({ stdout: JSON.stringify(pendingPullRequest) })
    }
  );

  assert.equal(exitCode, 1);
  assert.match(
    stderr.content,
    /watch #42 abcdef12 merge=BLOCKED success=0 failure=0 pending=1 unknown=0 pending_checks="build"/
  );
  assert.deepEqual(sleeps, [5, 5]);
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(pendingPullRequest)]
      },
      null,
      2
    )}\n`
  );
});

test("runCli watch treats unknown checks as not ready", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const sleeps = [];
  let currentTime = 0;
  const unknownPullRequest = samplePullRequest({
    statusCheckRollup: [{ name: "new-status", status: "COMPLETED", conclusion: "NEW_CONCLUSION" }]
  });

  const exitCode = await runCli(
    ["--watch", "--interval-ms", "5", "--timeout-ms", "5", "--json", "42"],
    {
      stdout,
      stderr,
      now: () => currentTime,
      sleep: async (ms) => {
        sleeps.push(ms);
        currentTime += ms;
      },
      execFile: async () => ({ stdout: JSON.stringify(unknownPullRequest) })
    }
  );

  assert.equal(exitCode, 1);
  assert.match(
    stderr.content,
    /watch #42 abcdef12 merge=BLOCKED success=0 failure=0 pending=0 unknown=1 pending_checks=-/
  );
  assert.deepEqual(sleeps, [5]);
  assert.equal(
    stdout.content,
    `${JSON.stringify(
      {
        pull_requests: [summarizePullRequest(unknownPullRequest)]
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
    /PR\s+State\s+Merge\s+Head\s+Success\s+Failure\s+Pending\s+Skipped\s+Neutral\s+Unknown\s+Title/
  );
  assert.match(
    output,
    /#42\s+OPEN\s+BLOCKED\s+codex\/pr-status-summary@abcdef12\s+2\s+1\s+1\s+1\s+1\s+0\s+Add watcher status summary/
  );
  assert.match(output, /Checks for #42/);
  assert.match(output, /lint\s+COMPLETED\s+SUCCESS\s+-\s+success/);
  assert.match(output, /branch protection\s+-\s+-\s+SUCCESS\s+success/);
});

test("formatHumanSummary shows merged pull requests as merged when merge state is absent", () => {
  const summary = summarizePullRequest(
    samplePullRequest({
      state: "MERGED",
      mergeStateStatus: null
    })
  );
  const output = formatHumanSummary([summary]);

  assert.equal(summary.merge_state_status, null);
  assert.match(
    output,
    /#42\s+MERGED\s+MERGED\s+codex\/pr-status-summary@abcdef12\s+2\s+1\s+1\s+1\s+1\s+0\s+Add watcher status summary/
  );
});

test("formatHumanSummary includes non-action review bot metadata comments", () => {
  const output = formatHumanSummary([
    summarizePullRequest(
      samplePullRequest({
        comments: [
          {
            author: { login: "coderabbitai" },
            body: "Review limit reached for this repository."
          }
        ]
      })
    )
  ]);

  assert.match(output, /Review notes for #42/);
  assert.match(output, /coderabbitai\s+rate_limited_review/);
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
