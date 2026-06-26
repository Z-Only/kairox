import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import test from "node:test";

import { compactSessionDiagnostics, runCli } from "./session-diagnostics-snapshot.mjs";

const execFileAsync = promisify(execFile);
const scriptPath = fileURLToPath(new URL("./session-diagnostics-snapshot.mjs", import.meta.url));

function createWritableCapture() {
  return {
    content: "",
    write(chunk) {
      this.content += chunk;
    }
  };
}

test("compactSessionDiagnostics emits stable compact counts from diagnostics JSON", () => {
  const compact = compactSessionDiagnostics({
    session_id: "ses_123",
    event_count: 9,
    event_type_counts: [
      { event_type: "UserMessageAdded", count: 2 },
      { event_type: "AssistantMessageCompleted", count: 1 },
      { event_type: "TrajectoryStarted", count: 2 },
      { event_type: "TrajectoryCompleted", count: 2 }
    ],
    last_event_type: "AssistantMessageCompleted",
    user_messages: [
      { message_id: "u1", content: "hello" },
      { message_id: "u2", content: "again" }
    ],
    assistant_messages: [{ message_id: "a1", content: "done" }],
    running_model_requests: 3,
    running_tool_invocations: 1,
    model_tool_calls: [
      { tool_call_id: "call_1", tool_id: "shell.exec" },
      { tool_call_id: "call_2", tool_id: "fs.read" }
    ],
    mcp_tool_calls: [{ server_id: "srv", tool_name: "lookup", status: "completed" }],
    trajectory_started_count: 2,
    trajectory_completed_count: 2,
    trajectory_completed_outcomes: [
      { trajectory_id: "t1", step_count: 2, outcome: "success" },
      { trajectory_id: "t2", step_count: 1, outcome: "failed" }
    ]
  });

  assert.deepEqual(compact, {
    session_id: "ses_123",
    event_count: 9,
    last_event_type: "AssistantMessageCompleted",
    event_type_counts: {
      AssistantMessageCompleted: 1,
      TrajectoryCompleted: 2,
      TrajectoryStarted: 2,
      UserMessageAdded: 2
    },
    user_message_count: 2,
    assistant_message_count: 1,
    running_model_requests: 3,
    running_tool_invocations: 1,
    model_tool_call_count: 2,
    mcp_tool_call_count: 1,
    has_tool_progress: true,
    trajectory_started_count: 2,
    trajectory_completed_count: 2,
    trajectory_failed_count: 1,
    failure_signal: "trajectory_failed",
    has_terminal_assistant_message: true
  });
});

test("compactSessionDiagnostics reports failed and blocked event signals", () => {
  const compact = compactSessionDiagnostics({
    session_id: "ses_failed",
    event_type_counts: {
      ToolInvocationFailed: 1,
      TaskBlocked: 1,
      UserMessageAdded: 1
    }
  });

  assert.equal(compact.failure_signal, "task_blocked");
});

test("compactSessionDiagnostics defaults missing newer diagnostics fields", () => {
  const compact = compactSessionDiagnostics({
    sessionId: "ses_legacy",
    event_type_counts: { UserMessageAdded: 1 },
    user_messages: [{ message_id: "u1", content: "hello" }]
  });

  assert.equal(compact.session_id, "ses_legacy");
  assert.equal(compact.event_count, 1);
  assert.equal(compact.last_event_type, null);
  assert.equal(compact.user_message_count, 1);
  assert.equal(compact.assistant_message_count, 0);
  assert.equal(compact.running_model_requests, 0);
  assert.equal(compact.running_tool_invocations, 0);
  assert.equal(compact.model_tool_call_count, 0);
  assert.equal(compact.mcp_tool_call_count, 0);
  assert.equal(compact.has_tool_progress, false);
  assert.equal(compact.trajectory_started_count, 0);
  assert.equal(compact.trajectory_completed_count, 0);
  assert.equal(compact.trajectory_failed_count, 0);
  assert.equal(compact.failure_signal, null);
  assert.equal(compact.has_terminal_assistant_message, false);
});

test("CLI reports missing tauri-pilot and does not create --out", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-session-diagnostics-"));
  const outPath = join(root, "nested", "snapshot.json");

  await assert.rejects(
    execFileAsync(process.execPath, [scriptPath, "--session", "ses_missing", "--out", outPath], {
      env: { ...process.env, PATH: "/nonexistent" }
    }),
    (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /tauri-pilot was not found on PATH/);
      return true;
    }
  );

  assert.equal(existsSync(outPath), false);
});

test("CLI writes the same compact JSON to stdout and --out", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-session-diagnostics-out-"));
  const outPath = join(root, "nested", "snapshot.json");
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--session", "ses_out", "--out", outPath], {
    stdout,
    stderr,
    cwd: root,
    env: {},
    execFile: async (command, args) => {
      assert.equal(command, "tauri-pilot");
      assert.deepEqual(args, [
        "ipc",
        "export_session_diagnostics",
        "--args",
        JSON.stringify({ sessionId: "ses_out" }),
        "--json"
      ]);
      return {
        stdout: JSON.stringify({
          session_id: "ses_out",
          event_count: 2,
          event_type_counts: [{ event_type: "UserMessageAdded", count: 1 }],
          user_messages: [{ message_id: "u1", content: "hello" }]
        })
      };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(await readFile(outPath, "utf8"), stdout.content);
  assert.equal(
    stdout.content,
    `${JSON.stringify({
      session_id: "ses_out",
      event_count: 2,
      last_event_type: null,
      event_type_counts: { UserMessageAdded: 1 },
      user_message_count: 1,
      assistant_message_count: 0,
      running_model_requests: 0,
      running_tool_invocations: 0,
      model_tool_call_count: 0,
      mcp_tool_call_count: 0,
      has_tool_progress: false,
      trajectory_started_count: 0,
      trajectory_completed_count: 0,
      trajectory_failed_count: 0,
      failure_signal: null,
      has_terminal_assistant_message: false
    })}\n`
  );
});
