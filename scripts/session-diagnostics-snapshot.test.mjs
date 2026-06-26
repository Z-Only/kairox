import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
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
    workspaceId: "wrk_123",
    projectId: "proj_123",
    worktreePath: "/repo/.worktrees/eval",
    branch: "eval/foo",
    modelProfile: "kairox-live",
    modelId: "gpt-5",
    provider: "openai",
    lastEventId: "42",
    createdAt: "2026-06-26T04:00:00Z",
    generatedAt: "2026-06-26T04:01:00Z",
    eventDbPath: "/tmp/kairox/events.sqlite",
    pilotSocketPath: "/tmp/tauri-pilot.sock",
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
    workspace_id: "wrk_123",
    project_id: "proj_123",
    worktree_path: "/repo/.worktrees/eval",
    branch: "eval/foo",
    model_profile: "kairox-live",
    model_id: "gpt-5",
    provider: "openai",
    last_event_id: "42",
    session_created_at: "2026-06-26T04:00:00Z",
    generated_at: "2026-06-26T04:01:00Z",
    event_db_path: "/tmp/kairox/events.sqlite",
    pilot_socket_path: "/tmp/tauri-pilot.sock",
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
    suspicious_no_tool_completion: false,
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

test("compactSessionDiagnostics reports cancelled event signals", () => {
  const compact = compactSessionDiagnostics({
    session_id: "ses_cancelled",
    event_type_counts: {
      SessionCancelled: 1,
      UserMessageAdded: 1
    }
  });

  assert.equal(compact.failure_signal, "session_cancelled");
});

test("compactSessionDiagnostics reports cancelled trajectory outcomes", () => {
  const compact = compactSessionDiagnostics({
    session_id: "ses_trajectory_cancelled",
    trajectory_completed_outcomes: [{ trajectory_id: "t1", step_count: 1, outcome: "cancelled" }]
  });

  assert.equal(compact.trajectory_failed_count, 0);
  assert.equal(compact.failure_signal, "trajectory_cancelled");
});

test("compactSessionDiagnostics defaults missing newer diagnostics fields", () => {
  const compact = compactSessionDiagnostics({
    sessionId: "ses_legacy",
    event_type_counts: { UserMessageAdded: 1 },
    user_messages: [{ message_id: "u1", content: "hello" }]
  });

  assert.equal(compact.session_id, "ses_legacy");
  assert.equal(compact.workspace_id, null);
  assert.equal(compact.project_id, null);
  assert.equal(compact.worktree_path, null);
  assert.equal(compact.branch, null);
  assert.equal(compact.model_profile, null);
  assert.equal(compact.model_id, null);
  assert.equal(compact.provider, null);
  assert.equal(compact.last_event_id, null);
  assert.equal(compact.session_created_at, null);
  assert.equal(compact.generated_at, null);
  assert.equal(compact.event_db_path, null);
  assert.equal(compact.pilot_socket_path, null);
  assert.equal(compact.event_count, 1);
  assert.equal(compact.last_event_type, null);
  assert.equal(compact.user_message_count, 1);
  assert.equal(compact.assistant_message_count, 0);
  assert.equal(compact.running_model_requests, 0);
  assert.equal(compact.running_tool_invocations, 0);
  assert.equal(compact.model_tool_call_count, 0);
  assert.equal(compact.mcp_tool_call_count, 0);
  assert.equal(compact.has_tool_progress, false);
  assert.equal(compact.suspicious_no_tool_completion, false);
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

test("module import does not require process.argv[1]", async () => {
  const result = await execFileAsync(process.execPath, [
    "--input-type=module",
    "-e",
    `import ${JSON.stringify(pathToFileURL(scriptPath).href)}; console.log("imported");`
  ]);

  assert.equal(result.stdout.trim(), "imported");
  assert.equal(result.stderr, "");
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
      workspace_id: null,
      project_id: null,
      worktree_path: null,
      branch: null,
      model_profile: null,
      model_id: null,
      provider: null,
      last_event_id: null,
      session_created_at: null,
      generated_at: null,
      event_db_path: null,
      pilot_socket_path: null,
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
      suspicious_no_tool_completion: false,
      trajectory_started_count: 0,
      trajectory_completed_count: 0,
      trajectory_failed_count: 0,
      failure_signal: null,
      has_terminal_assistant_message: false
    })}\n`
  );
});

test("CLI overlays explicit resume metadata", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(
    [
      "--session",
      "ses_meta",
      "--meta",
      "worktree_path=/repo/.worktrees/eval",
      "--meta",
      "branch=eval/foo",
      "--meta",
      "event_db_path=/tmp/kairox/events.sqlite",
      "--meta",
      "pilot_socket_path=/tmp/tauri-pilot.sock"
    ],
    {
      stdout,
      stderr,
      execFile: async () => ({
        stdout: JSON.stringify({
          session_id: "ses_meta",
          event_count: 1,
          event_type_counts: [{ event_type: "UserMessageAdded", count: 1 }]
        })
      })
    }
  );

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(JSON.parse(stdout.content), {
    session_id: "ses_meta",
    workspace_id: null,
    project_id: null,
    worktree_path: "/repo/.worktrees/eval",
    branch: "eval/foo",
    model_profile: null,
    model_id: null,
    provider: null,
    last_event_id: null,
    session_created_at: null,
    generated_at: null,
    event_db_path: "/tmp/kairox/events.sqlite",
    pilot_socket_path: "/tmp/tauri-pilot.sock",
    event_count: 1,
    last_event_type: null,
    event_type_counts: { UserMessageAdded: 1 },
    user_message_count: 0,
    assistant_message_count: 0,
    running_model_requests: 0,
    running_tool_invocations: 0,
    model_tool_call_count: 0,
    mcp_tool_call_count: 0,
    has_tool_progress: false,
    suspicious_no_tool_completion: false,
    trajectory_started_count: 0,
    trajectory_completed_count: 0,
    trajectory_failed_count: 0,
    failure_signal: null,
    has_terminal_assistant_message: false
  });
});

test("CLI infers branch metadata from worktree_path", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(
    ["--session", "ses_branch", "--meta", "worktree_path=/repo/.worktrees/eval"],
    {
      stdout,
      stderr,
      execFile: async (command, args) => {
        if (command === "tauri-pilot") {
          return {
            stdout: JSON.stringify({
              session_id: "ses_branch",
              event_count: 1,
              event_type_counts: [{ event_type: "UserMessageAdded", count: 1 }]
            })
          };
        }
        if (
          command === "git" &&
          args.join(" ") === "-C /repo/.worktrees/eval branch --show-current"
        ) {
          return { stdout: "eval/foo\n" };
        }
        throw new Error(`unexpected command: ${command} ${args.join(" ")}`);
      }
    }
  );

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(JSON.parse(stdout.content).branch, "eval/foo");
});

test("CLI infers pilot socket metadata from TAURI_PILOT_SOCKET", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--session", "ses_socket"], {
    stdout,
    stderr,
    env: { TAURI_PILOT_SOCKET: "/tmp/tauri-pilot.sock" },
    execFile: async (command) => {
      assert.equal(command, "tauri-pilot");
      return {
        stdout: JSON.stringify({
          session_id: "ses_socket",
          event_count: 1,
          event_type_counts: [{ event_type: "UserMessageAdded", count: 1 }]
        })
      };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(JSON.parse(stdout.content).pilot_socket_path, "/tmp/tauri-pilot.sock");
});

test("CLI infers event DB metadata from KAIROX_HOME", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-home-"));
  const dataDir = join(root, ".kairox");
  const dbPath = join(dataDir, "kairox-gui.sqlite");
  await mkdir(dataDir);
  await writeFile(dbPath, "");
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--session", "ses_event_db"], {
    stdout,
    stderr,
    env: { KAIROX_HOME: root },
    execFile: async (command) => {
      assert.equal(command, "tauri-pilot");
      return {
        stdout: JSON.stringify({
          session_id: "ses_event_db",
          event_count: 1,
          event_type_counts: [{ event_type: "UserMessageAdded", count: 1 }]
        })
      };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(JSON.parse(stdout.content).event_db_path, dbPath);
});

test("compactSessionDiagnostics flags terminal assistant messages without tool progress", () => {
  const compact = compactSessionDiagnostics({
    session_id: "ses_no_tools",
    event_type_counts: {
      UserMessageAdded: 1,
      AssistantMessageCompleted: 1
    },
    last_event_type: "AssistantMessageCompleted",
    assistant_messages: [{ message_id: "a1", content: "I will do it" }]
  });

  assert.equal(compact.has_terminal_assistant_message, true);
  assert.equal(compact.has_tool_progress, false);
  assert.equal(compact.suspicious_no_tool_completion, true);
});
