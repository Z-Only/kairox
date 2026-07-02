import { execFile as execFileCallback } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);

const USAGE = `Usage: node scripts/session-diagnostics-snapshot.mjs --session <id> [--out <path>] [--meta <key=value>...]

Exports compact session diagnostics from a running Kairox app through tauri-pilot.

Options:
  --session <id>  Session id to export.
  --out <path>   Write the same compact JSON to a file.
  --meta <k=v>   Overlay one resume metadata field into the compact JSON.
  --help, -h     Show this help.
`;

class UsageError extends Error {}

const EVENT_DB_PATH_KEYS = [
  "event_db_path",
  "eventDbPath",
  "db_path",
  "dbPath",
  "database_path",
  "databasePath"
];

const EVENT_DB_PATH_SOURCE_KEYS = [
  "event_db_path_source",
  "eventDbPathSource",
  "db_path_source",
  "dbPathSource",
  "database_path_source",
  "databasePathSource"
];

const FORBIDDEN_EVAL_TOOL_IDS = new Set(["browser.action", "browser.batch", "computer.use"]);

function firstPresent(source, names) {
  for (const name of names) {
    if (source?.[name] !== undefined && source[name] !== null) {
      return source[name];
    }
  }
  return undefined;
}

function firstPresentOrNull(source, names) {
  return firstPresent(source, names) ?? null;
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

function normalizeToolCounts(value) {
  const counts = new Map();

  if (Array.isArray(value)) {
    for (const entry of value) {
      const toolId = firstPresent(entry, ["tool_id", "toolId", "id"]);
      if (typeof toolId !== "string" || toolId.length === 0) {
        continue;
      }
      counts.set(toolId, (counts.get(toolId) ?? 0) + countValue(firstPresent(entry, ["count"])));
    }
  } else if (value && typeof value === "object") {
    for (const [toolId, count] of Object.entries(value)) {
      if (toolId.length === 0) {
        continue;
      }
      counts.set(toolId, countValue(count));
    }
  }

  return Object.fromEntries(
    [...counts.entries()].sort(([left], [right]) => left.localeCompare(right))
  );
}

function sumCounts(counts) {
  return Object.values(counts).reduce((total, count) => total + countValue(count), 0);
}

function normalizeContextUsage(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value;
}

function latestContextUsage(source) {
  const direct = normalizeContextUsage(
    firstPresent(source, [
      "context_usage",
      "contextUsage",
      "last_context_usage",
      "lastContextUsage"
    ])
  );
  if (direct) {
    return direct;
  }

  const history = firstPresent(source, [
    "context_usage_history",
    "contextUsageHistory",
    "context_assembled_usages",
    "contextAssembledUsages"
  ]);
  if (!Array.isArray(history)) {
    return null;
  }

  for (const entry of history.slice().reverse()) {
    const usage = normalizeContextUsage(
      firstPresent(entry, ["usage", "context_usage", "contextUsage"])
    );
    if (usage) {
      return usage;
    }
    const entryAsUsage = normalizeContextUsage(entry);
    if (entryAsUsage) {
      return entryAsUsage;
    }
  }
  return null;
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

function trajectoryCancelledCount(source) {
  const outcomes = firstPresent(source, [
    "trajectory_completed_outcomes",
    "trajectoryCompletedOutcomes"
  ]);
  if (!Array.isArray(outcomes)) {
    return 0;
  }

  return outcomes.filter((outcome) =>
    ["cancelled", "canceled"].includes(String(outcome?.outcome ?? "").toLowerCase())
  ).length;
}

function snakeCaseEventType(eventType) {
  return eventType.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
}

function stringOrNull(value) {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function numberOrNull(value) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value));
  }
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return Math.max(0, Math.trunc(parsed));
    }
  }
  return null;
}

function parseModelStreamMessage(message) {
  const parsed = {
    last_event: null,
    assistant_chars: null,
    emitted_tool_calls: null,
    model_profile: null,
    model_id: null,
    provider: null,
    base_url: null
  };
  if (typeof message !== "string") {
    return parsed;
  }

  const patterns = {
    last_event: /(?:^|[\s,;])last_event=([^\s,;]+)/,
    assistant_chars: /(?:^|[\s,;])assistant_chars=(\d+)/,
    emitted_tool_calls: /(?:^|[\s,;])emitted_tool_calls=(\d+)/,
    model_profile: /(?:^|[\s,;])model_profile=([^\s,;]+)/,
    model_id: /(?:^|[\s,;])model_id=([^\s,;]+)/,
    provider: /(?:^|[\s,;])provider=([^\s,;]+)/,
    base_url: /(?:^|[\s,;])base_url=([^\s,;]+)/
  };

  for (const [key, pattern] of Object.entries(patterns)) {
    const match = message.match(pattern);
    if (!match) {
      continue;
    }
    parsed[key] =
      key === "assistant_chars" || key === "emitted_tool_calls" ? numberOrNull(match[1]) : match[1];
  }

  return parsed;
}

function modelStreamStatusEntries(source) {
  const value = firstPresent(source, [
    "recent_model_stream_statuses",
    "recentModelStreamStatuses",
    "model_stream_statuses",
    "modelStreamStatuses"
  ]);
  return Array.isArray(value) ? value : [];
}

function modelStreamFailure(source) {
  const statuses = modelStreamStatusEntries(source);
  for (const status of statuses.slice().reverse()) {
    if (!status || typeof status !== "object") {
      continue;
    }
    if (firstPresent(status, ["retrying"]) === true) {
      continue;
    }

    const phase = stringOrNull(firstPresent(status, ["phase"]));
    const message = stringOrNull(firstPresent(status, ["message"]));
    const parsedMessage = parseModelStreamMessage(message);
    const assistantChars = parsedMessage.assistant_chars;
    const emittedToolCalls = parsedMessage.emitted_tool_calls;
    const hasProgress = (assistantChars ?? 0) > 0 || (emittedToolCalls ?? 0) > 0;
    const timeoutLike =
      /timeout|timed out|stalled|prematurely|without producing|no stream events|before any events/i.test(
        message ?? ""
      );
    if (!timeoutLike) {
      continue;
    }

    return {
      kind: hasProgress ? "stalled_after_progress" : "no_event_timeout",
      phase,
      retry_attempt: numberOrNull(firstPresent(status, ["retry_attempt", "retryAttempt"])),
      max_retries: numberOrNull(firstPresent(status, ["max_retries", "maxRetries"])),
      last_event: parsedMessage.last_event,
      assistant_chars: assistantChars,
      emitted_tool_calls: emittedToolCalls,
      model_profile:
        parsedMessage.model_profile ??
        firstPresentOrNull(source, ["model_profile", "modelProfile"]),
      model_id: parsedMessage.model_id ?? firstPresentOrNull(source, ["model_id", "modelId"]),
      provider: parsedMessage.provider ?? firstPresentOrNull(source, ["provider"]),
      base_url: parsedMessage.base_url ?? firstPresentOrNull(source, ["base_url", "baseUrl"])
    };
  }
  return null;
}

function failureSignal(
  eventTypeCounts,
  trajectoryFailedCountValue,
  trajectoryCancelledCountValue,
  modelStreamFailureValue
) {
  if (modelStreamFailureValue) {
    return `model_stream_${modelStreamFailureValue.kind}`;
  }
  for (const eventType of Object.keys(eventTypeCounts)) {
    if (
      countValue(eventTypeCounts[eventType]) > 0 &&
      /(Blocked|Failed|Denied|Cancelled|Canceled)$/.test(eventType)
    ) {
      return snakeCaseEventType(eventType);
    }
  }
  if (trajectoryFailedCountValue > 0) {
    return "trajectory_failed";
  }
  return trajectoryCancelledCountValue > 0 ? "trajectory_cancelled" : null;
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
  const permissionDeniedToolCounts = normalizeToolCounts(
    firstPresent(diagnostics, ["permission_denied_tools", "permissionDeniedTools"])
  );
  const explicitEventCount = firstPresent(diagnostics, ["event_count", "eventCount"]);
  const runningToolInvocations = countValue(
    firstPresent(diagnostics, ["running_tool_invocations", "runningToolInvocations"])
  );
  const modelToolCallCount = countValue(
    firstPresent(diagnostics, ["model_tool_calls", "modelToolCalls"])
  );
  const mcpToolCallCount = countValue(
    firstPresent(diagnostics, ["mcp_tool_calls", "mcpToolCalls"])
  );
  const modelTokenDeltaCount = countValue(
    firstPresent(diagnostics, ["model_token_delta_count", "modelTokenDeltaCount"]) ??
      eventTypeCounts.ModelTokenDelta
  );
  const trajectoryFailedCountValue = trajectoryFailedCount(diagnostics);
  const trajectoryCancelledCountValue = trajectoryCancelledCount(diagnostics);
  const modelStreamFailureValue = modelStreamFailure(diagnostics);
  const hasToolProgress =
    modelToolCallCount > 0 || mcpToolCallCount > 0 || runningToolInvocations > 0;
  const terminalAssistantMessage = hasTerminalAssistantMessage(diagnostics);

  return {
    session_id: String(firstPresent(diagnostics, ["session_id", "sessionId"]) ?? sessionId ?? ""),
    workspace_id: firstPresentOrNull(diagnostics, ["workspace_id", "workspaceId"]),
    project_id: firstPresentOrNull(diagnostics, ["project_id", "projectId"]),
    worktree_path: firstPresentOrNull(diagnostics, ["worktree_path", "worktreePath"]),
    branch: firstPresentOrNull(diagnostics, ["branch"]),
    model_profile: firstPresentOrNull(diagnostics, [
      "model_profile",
      "modelProfile",
      "profile",
      "current_profile",
      "currentProfile"
    ]),
    model_id: firstPresentOrNull(diagnostics, ["model_id", "modelId"]),
    provider: firstPresentOrNull(diagnostics, ["provider"]),
    last_event_id: firstPresentOrNull(diagnostics, ["last_event_id", "lastEventId"]),
    session_created_at: firstPresentOrNull(diagnostics, [
      "session_created_at",
      "sessionCreatedAt",
      "created_at",
      "createdAt"
    ]),
    generated_at: firstPresentOrNull(diagnostics, ["generated_at", "generatedAt"]),
    event_db_path: firstPresentOrNull(diagnostics, EVENT_DB_PATH_KEYS),
    event_db_path_source: firstPresentOrNull(diagnostics, EVENT_DB_PATH_SOURCE_KEYS),
    pilot_socket_path: firstPresentOrNull(diagnostics, [
      "pilot_socket_path",
      "pilotSocketPath",
      "dev_app_socket",
      "devAppSocket",
      "socket_path",
      "socketPath"
    ]),
    context_usage: latestContextUsage(diagnostics),
    event_count:
      explicitEventCount === undefined
        ? sumCounts(eventTypeCounts)
        : countValue(explicitEventCount),
    last_event_type: firstPresent(diagnostics, ["last_event_type", "lastEventType"]) ?? null,
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
    running_tool_invocations: runningToolInvocations,
    model_tool_call_count: modelToolCallCount,
    mcp_tool_call_count: mcpToolCallCount,
    permission_denied_tool_counts: permissionDeniedToolCounts,
    forbidden_tool_denied_count: Object.entries(permissionDeniedToolCounts).reduce(
      (total, [toolId, count]) =>
        FORBIDDEN_EVAL_TOOL_IDS.has(toolId) ? total + countValue(count) : total,
      0
    ),
    model_token_delta_count: modelTokenDeltaCount,
    model_stream_failure: modelStreamFailureValue,
    has_tool_progress: hasToolProgress,
    suspicious_no_tool_completion: terminalAssistantMessage && !hasToolProgress,
    trajectory_started_count: countValue(
      firstPresent(diagnostics, ["trajectory_started_count", "trajectoryStartedCount"]) ??
        eventTypeCounts.TrajectoryStarted
    ),
    trajectory_completed_count: countValue(
      firstPresent(diagnostics, ["trajectory_completed_count", "trajectoryCompletedCount"]) ??
        eventTypeCounts.TrajectoryCompleted
    ),
    trajectory_failed_count: trajectoryFailedCountValue,
    failure_signal: failureSignal(
      eventTypeCounts,
      trajectoryFailedCountValue,
      trajectoryCancelledCountValue,
      modelStreamFailureValue
    ),
    has_terminal_assistant_message: terminalAssistantMessage
  };
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    meta: {},
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
    if (arg === "--meta") {
      addMeta(parsed.meta, argv[index + 1]);
      index += 1;
      continue;
    }
    if (arg.startsWith("--meta=")) {
      addMeta(parsed.meta, arg.slice("--meta=".length));
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

function addMeta(meta, value) {
  const separator = String(value ?? "").indexOf("=");
  if (separator <= 0) {
    throw new UsageError("--meta requires key=value");
  }
  meta[value.slice(0, separator)] = value.slice(separator + 1);
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

function pidIsRunning(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

function startedAtMillis(record) {
  const millis = Date.parse(record?.started_at ?? "");
  return Number.isFinite(millis) ? millis : 0;
}

function inferEventDbPath(kairoxHome, pathExists = existsSync, processIsRunning = pidIsRunning) {
  const dataDir = join(String(kairoxHome), ".kairox");
  const registryDir = join(dataDir, "runtime", "instances");
  try {
    const records = [];
    for (const file of readdirSync(registryDir).filter((entry) => entry.endsWith(".json"))) {
      try {
        records.push(JSON.parse(readFileSync(join(registryDir, file), "utf8")));
      } catch {
        // Match the runtime registry: ignore partial or invalid records.
      }
    }
    records.sort((left, right) => startedAtMillis(right) - startedAtMillis(left));
    for (const record of records) {
      if (
        typeof record?.pid !== "number" ||
        !Number.isFinite(record.pid) ||
        typeof record?.data_dir !== "string" ||
        typeof record?.database_filename !== "string"
      ) {
        continue;
      }
      if (!processIsRunning(record.pid)) {
        continue;
      }
      const eventDbPath = join(record.data_dir, record.database_filename);
      if (pathExists(eventDbPath)) {
        return { path: eventDbPath, source: "runtime_registry" };
      }
    }
  } catch {
    // Fall back to the default GUI database below.
  }

  const eventDbPath = join(dataDir, "kairox-gui.sqlite");
  return pathExists(eventDbPath) ? { path: eventDbPath, source: "default_kairox_home" } : null;
}

async function inferResumeMeta(
  meta,
  { execFile, env, pathExists = existsSync, processIsRunning = pidIsRunning }
) {
  const inferred = {};
  if (!firstPresent(meta, EVENT_DB_PATH_KEYS) && env?.KAIROX_HOME) {
    const eventDbPath = inferEventDbPath(env.KAIROX_HOME, pathExists, processIsRunning);
    if (eventDbPath) {
      inferred.event_db_path = eventDbPath.path;
      inferred.event_db_path_source = eventDbPath.source;
    }
  }
  if (
    !firstPresent(meta, ["pilot_socket_path", "pilotSocketPath", "socket_path", "socketPath"]) &&
    env?.TAURI_PILOT_SOCKET
  ) {
    inferred.pilot_socket_path = env.TAURI_PILOT_SOCKET;
  }

  const worktreePath = firstPresent(meta, ["worktree_path", "worktreePath"]);
  if (!worktreePath || meta.branch) {
    return inferred;
  }

  try {
    const result = await execFile("git", ["-C", worktreePath, "branch", "--show-current"], {
      env,
      maxBuffer: 1024 * 1024
    });
    const branch = result.stdout.trim();
    return branch ? { ...inferred, branch } : inferred;
  } catch {
    return inferred;
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
    pathExists = existsSync,
    processIsRunning = pidIsRunning,
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

    const rawDiagnostics = await exportSessionDiagnostics(args.session, { execFile, env, cwd });
    const explicitMeta = { ...args.meta };
    if (
      firstPresent(explicitMeta, EVENT_DB_PATH_KEYS) &&
      !firstPresent(explicitMeta, EVENT_DB_PATH_SOURCE_KEYS)
    ) {
      explicitMeta.event_db_path_source = "explicit_meta";
    }
    const resumeMeta = { ...rawDiagnostics, ...explicitMeta };
    const diagnostics = {
      ...resumeMeta,
      ...(await inferResumeMeta(resumeMeta, { execFile, env, pathExists, processIsRunning })),
      ...explicitMeta
    };
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

if (
  typeof process.argv[1] === "string" &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  process.exitCode = await runCli();
}
