import { execFile as execFileCallback } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);

const USAGE = `Usage: node scripts/session-diagnostics-snapshot.mjs --session <id> [--out <path>]

Exports compact session diagnostics from a running Kairox app through tauri-pilot.

Options:
  --session <id>  Session id to export.
  --out <path>   Write the same compact JSON to a file.
  --help, -h     Show this help.
`;

class UsageError extends Error {}

function firstPresent(source, names) {
  for (const name of names) {
    if (source?.[name] !== undefined && source[name] !== null) {
      return source[name];
    }
  }
  return undefined;
}

function countValue(value) {
  if (Array.isArray(value)) {
    return value.length;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value));
  }
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return Math.max(0, Math.trunc(parsed));
    }
  }
  return 0;
}

function normalizeEventTypeCounts(value) {
  const counts = new Map();

  if (Array.isArray(value)) {
    for (const entry of value) {
      const eventType = firstPresent(entry, ["event_type", "eventType", "type"]);
      if (typeof eventType !== "string" || eventType.length === 0) {
        continue;
      }
      counts.set(eventType, countValue(firstPresent(entry, ["count", "total"])));
    }
  } else if (value && typeof value === "object") {
    for (const [eventType, count] of Object.entries(value)) {
      counts.set(eventType, countValue(count));
    }
  }

  return Object.fromEntries(
    [...counts.entries()].sort(([left], [right]) => left.localeCompare(right))
  );
}

function sumCounts(counts) {
  return Object.values(counts).reduce((total, count) => total + countValue(count), 0);
}

function messageCount(source, countNames, listNames) {
  const explicit = firstPresent(source, countNames);
  if (explicit !== undefined) {
    return countValue(explicit);
  }
  return countValue(firstPresent(source, listNames));
}

function trajectoryFailedCount(source) {
  const explicit = firstPresent(source, ["trajectory_failed_count", "trajectoryFailedCount"]);
  if (explicit !== undefined) {
    return countValue(explicit);
  }

  const outcomes = firstPresent(source, [
    "trajectory_completed_outcomes",
    "trajectoryCompletedOutcomes"
  ]);
  if (!Array.isArray(outcomes)) {
    return 0;
  }

  return outcomes.filter((outcome) => String(outcome?.outcome ?? "").toLowerCase() === "failed")
    .length;
}

function hasTerminalAssistantMessage(source) {
  const explicit = firstPresent(source, [
    "has_terminal_assistant_message",
    "hasTerminalAssistantMessage"
  ]);
  if (explicit !== undefined) {
    return explicit === true;
  }

  return firstPresent(source, ["last_event_type", "lastEventType"]) === "AssistantMessageCompleted";
}

function diagnosticsLike(value) {
  return (
    value &&
    typeof value === "object" &&
    (firstPresent(value, ["session_id", "sessionId"]) !== undefined ||
      firstPresent(value, ["event_count", "eventCount"]) !== undefined ||
      firstPresent(value, ["event_type_counts", "eventTypeCounts"]) !== undefined)
  );
}

export function unwrapTauriPilotJson(value) {
  if (!value || typeof value !== "object") {
    throw new Error("tauri-pilot returned JSON that is not an object");
  }

  if (diagnosticsLike(value)) {
    return value;
  }

  for (const key of ["result", "data", "value"]) {
    const nested = value[key];
    if (diagnosticsLike(nested)) {
      return nested;
    }
  }

  if (value.error) {
    throw new Error(`tauri-pilot returned an IPC error: ${JSON.stringify(value.error)}`);
  }

  return value;
}

export function compactSessionDiagnostics(rawDiagnostics, { sessionId } = {}) {
  if (!rawDiagnostics || typeof rawDiagnostics !== "object") {
    throw new Error("Expected diagnostics JSON object");
  }

  const diagnostics = unwrapTauriPilotJson(rawDiagnostics);
  const eventTypeCounts = normalizeEventTypeCounts(
    firstPresent(diagnostics, ["event_type_counts", "eventTypeCounts"])
  );
  const explicitEventCount = firstPresent(diagnostics, ["event_count", "eventCount"]);

  return {
    session_id: String(firstPresent(diagnostics, ["session_id", "sessionId"]) ?? sessionId ?? ""),
    event_count:
      explicitEventCount === undefined
        ? sumCounts(eventTypeCounts)
        : countValue(explicitEventCount),
    event_type_counts: eventTypeCounts,
    user_message_count: messageCount(
      diagnostics,
      ["user_message_count", "userMessageCount"],
      ["user_messages", "userMessages"]
    ),
    assistant_message_count: messageCount(
      diagnostics,
      ["assistant_message_count", "assistantMessageCount"],
      ["assistant_messages", "assistantMessages"]
    ),
    running_model_requests: countValue(
      firstPresent(diagnostics, ["running_model_requests", "runningModelRequests"])
    ),
    running_tool_invocations: countValue(
      firstPresent(diagnostics, ["running_tool_invocations", "runningToolInvocations"])
    ),
    trajectory_started_count: countValue(
      firstPresent(diagnostics, ["trajectory_started_count", "trajectoryStartedCount"]) ??
        eventTypeCounts.TrajectoryStarted
    ),
    trajectory_completed_count: countValue(
      firstPresent(diagnostics, ["trajectory_completed_count", "trajectoryCompletedCount"]) ??
        eventTypeCounts.TrajectoryCompleted
    ),
    trajectory_failed_count: trajectoryFailedCount(diagnostics),
    has_terminal_assistant_message: hasTerminalAssistantMessage(diagnostics)
  };
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    out: undefined,
    session: undefined
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      continue;
    }
    if (arg === "--session") {
      parsed.session = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg.startsWith("--session=")) {
      parsed.session = arg.slice("--session=".length);
      continue;
    }
    if (arg === "--out") {
      parsed.out = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg.startsWith("--out=")) {
      parsed.out = arg.slice("--out=".length);
      continue;
    }
    throw new UsageError(`Unknown argument: ${arg}`);
  }

  if (parsed.help) {
    return parsed;
  }
  if (!parsed.session) {
    throw new UsageError("Missing required --session <id>.");
  }
  if (parsed.out === "") {
    throw new UsageError("--out requires a non-empty path.");
  }

  return parsed;
}

function parsePilotStdout(stdout) {
  const trimmed = stdout.trim();
  if (!trimmed) {
    throw new Error("tauri-pilot returned empty JSON output");
  }

  try {
    return JSON.parse(trimmed);
  } catch (error) {
    throw new Error(`tauri-pilot returned invalid JSON: ${error.message}`);
  }
}

function ipcFailureMessage(error) {
  if (error?.code === "ENOENT" || /spawn tauri-pilot ENOENT/.test(String(error?.message))) {
    return "tauri-pilot was not found on PATH. Install tauri-pilot or add it to PATH, then rerun this command.";
  }

  const detail = [error?.stderr, error?.stdout, error?.message].filter(Boolean).join("\n").trim();
  const exitCode = typeof error?.code === "number" ? error.code : "unknown";
  return [
    `tauri-pilot ipc export_session_diagnostics failed (exit ${exitCode}).`,
    "Ensure Kairox is running with the pilot feature enabled and the session id exists.",
    detail
  ]
    .filter(Boolean)
    .join("\n");
}

export async function exportSessionDiagnostics(
  sessionId,
  { execFile = execFileAsync, env = process.env, cwd } = {}
) {
  const args = [
    "ipc",
    "export_session_diagnostics",
    "--args",
    JSON.stringify({ sessionId }),
    "--json"
  ];

  let result;
  try {
    result = await execFile("tauri-pilot", args, {
      cwd,
      env,
      maxBuffer: 10 * 1024 * 1024
    });
  } catch (error) {
    throw new Error(ipcFailureMessage(error));
  }

  return unwrapTauriPilotJson(parsePilotStdout(result.stdout));
}

export async function runCli(
  argv = process.argv.slice(2),
  {
    stdout = process.stdout,
    stderr = process.stderr,
    execFile = execFileAsync,
    env = process.env,
    cwd = process.cwd()
  } = {}
) {
  try {
    const args = parseArgs(argv);
    if (args.help) {
      stdout.write(USAGE);
      return 0;
    }

    const diagnostics = await exportSessionDiagnostics(args.session, { execFile, env, cwd });
    const output = `${JSON.stringify(compactSessionDiagnostics(diagnostics, { sessionId: args.session }))}\n`;

    if (args.out) {
      await mkdir(dirname(args.out), { recursive: true });
      await writeFile(args.out, output);
    }

    stdout.write(output);
    return 0;
  } catch (error) {
    const usage = error instanceof UsageError ? `\n\n${USAGE}` : "";
    stderr.write(`Error: ${error.message}${usage}\n`);
    return 1;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await runCli();
}
