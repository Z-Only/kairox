import { execFile as execFileCallback } from "node:child_process";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFileCallback);

export const GENERATED_BINDING_PATHS = [
  "apps/agent-gui/src/generated/commands.ts",
  "apps/agent-gui/src/generated/events.ts"
];

export const TRIGGER_PATH_DESCRIPTIONS = [
  "apps/agent-gui/src-tauri/**",
  "crates/agent-core/src/events.rs",
  "crates/agent-core/src/**/*.rs"
];

export const USAGE = `Usage: node scripts/check-generated-guardrails.mjs [--base <ref>]

Fails when generated GUI bindings changed without a Rust/Specta/event source trigger.

Options:
  --base <ref>  Compare <ref>...HEAD instead of staged/unstaged changes.
  --help, -h    Show this help.
`;

class UsageError extends Error {}

function normalizeChangedPath(path) {
  return String(path ?? "")
    .trim()
    .replaceAll("\\", "/")
    .replace(/^\.\//, "");
}

function parseNameOnlyOutput(stdout) {
  return String(stdout ?? "")
    .split(/\r?\n/)
    .map(normalizeChangedPath)
    .filter(Boolean);
}

function isTriggerPath(path) {
  return (
    path.startsWith("apps/agent-gui/src-tauri/") ||
    path === "crates/agent-core/src/events.rs" ||
    (path.startsWith("crates/agent-core/src/") && path.endsWith(".rs"))
  );
}

function formatFailureMessage(generatedPaths) {
  return [
    `generated bindings changed without a Rust/Specta/event source trigger: ${generatedPaths.join(", ")}`,
    `Allowed source triggers: ${TRIGGER_PATH_DESCRIPTIONS.join(", ")}`,
    "Run `just gen-types` only after generator/source changes, then commit the generated bindings with those changes."
  ].join("\n");
}

export function evaluateGeneratedGuardrails(changedPaths) {
  const normalizedPaths = [
    ...new Set((changedPaths ?? []).map(normalizeChangedPath).filter(Boolean))
  ];
  const generatedPaths = normalizedPaths.filter((path) => GENERATED_BINDING_PATHS.includes(path));
  const triggerPaths = normalizedPaths.filter(isTriggerPath);

  if (generatedPaths.length === 0) {
    return {
      ok: true,
      generatedPaths,
      triggerPaths,
      message: "No generated bindings changed."
    };
  }

  if (triggerPaths.length > 0) {
    return {
      ok: true,
      generatedPaths,
      triggerPaths,
      message: "Generated bindings changed with a Rust/Specta/event source trigger."
    };
  }

  return {
    ok: false,
    generatedPaths,
    triggerPaths,
    message: formatFailureMessage(generatedPaths)
  };
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    base: undefined
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      continue;
    }
    if (arg === "--base") {
      const value = argv[index + 1];
      if (value === undefined || value.startsWith("-")) {
        throw new UsageError("--base requires a value.");
      }
      parsed.base = value;
      index += 1;
      continue;
    }
    if (arg.startsWith("--base=")) {
      parsed.base = arg.slice("--base=".length);
      continue;
    }
    throw new UsageError(`Unknown argument: ${arg}`);
  }

  if (!parsed.help && parsed.base === "") {
    throw new UsageError("--base requires a non-empty value.");
  }

  return parsed;
}

async function readGitNameOnly(execFile, args, { cwd, env }) {
  const result = await execFile("git", args, {
    cwd,
    env,
    maxBuffer: 10 * 1024 * 1024
  });
  return parseNameOnlyOutput(result.stdout);
}

export async function readChangedPaths({
  execFile = execFileAsync,
  cwd = process.cwd(),
  env = process.env,
  base
} = {}) {
  if (base) {
    return readGitNameOnly(execFile, ["diff", "--name-only", `${base}...HEAD`], { cwd, env });
  }

  const stagedPaths = await readGitNameOnly(execFile, ["diff", "--name-only", "--cached"], {
    cwd,
    env
  });
  if (stagedPaths.length > 0) {
    return stagedPaths;
  }

  return readGitNameOnly(execFile, ["diff", "--name-only"], { cwd, env });
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

    const changedPaths = await readChangedPaths({ execFile, cwd, env, base: args.base });
    const result = evaluateGeneratedGuardrails(changedPaths);
    const stream = result.ok ? stdout : stderr;
    stream.write(`${result.message}\n`);
    return result.ok ? 0 : 1;
  } catch (error) {
    const usage = error instanceof UsageError ? `\n\n${USAGE}` : "";
    stderr.write(`Error: ${error.message}${usage}\n`);
    return 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await runCli();
}
