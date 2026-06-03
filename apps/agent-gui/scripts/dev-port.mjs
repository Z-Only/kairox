import fs from "node:fs";
import net from "node:net";
import path from "node:path";

export const DEFAULT_DEV_PORT = 1420;
export const DEFAULT_PORT_CHECK_HOST = "127.0.0.1";
export const TAURI_DEV_COMMAND = "bun run dev";

const MAX_PORT = 65_535;

export function parsePort(value, fallback = DEFAULT_DEV_PORT) {
  if (value === undefined || value === null || value === "") {
    return fallback;
  }

  const port = Number.parseInt(String(value), 10);
  if (!Number.isInteger(port) || port < 1 || port > MAX_PORT) {
    throw new Error(`Invalid dev server port: ${value}`);
  }
  return port;
}

export function isEnabled(value) {
  return ["1", "true", "yes", "on"].includes(String(value ?? "").toLowerCase());
}

export function shouldUseStrictPort(env = process.env) {
  return isEnabled(env.KAIROX_DEV_STRICT_PORT);
}

export function buildTauriDevIdentifier(port) {
  return `dev.kairox.agent.dev${parsePort(port)}`;
}

export function isPortAvailable(port, host = DEFAULT_PORT_CHECK_HOST) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();

    server.unref();
    server.once("error", (error) => {
      if (error.code === "EADDRINUSE" || error.code === "EACCES") {
        resolve(false);
        return;
      }
      reject(error);
    });
    server.listen({ port, host }, () => {
      server.close(() => resolve(true));
    });
  });
}

export async function findAvailablePort({
  preferredPort = DEFAULT_DEV_PORT,
  host = DEFAULT_PORT_CHECK_HOST,
  maxAttempts = 100
} = {}) {
  const startPort = parsePort(preferredPort);
  for (let offset = 0; offset < maxAttempts; offset++) {
    const port = startPort + offset;
    if (port > MAX_PORT) {
      break;
    }
    if (await isPortAvailable(port, host)) {
      return port;
    }
  }

  throw new Error(
    `No available dev server port found from ${startPort} after ${maxAttempts} attempts`
  );
}

export async function resolveTauriDevPort(env = process.env) {
  const preferredPort = parsePort(env.KAIROX_DEV_PORT, DEFAULT_DEV_PORT);
  if (shouldUseStrictPort(env)) {
    return preferredPort;
  }

  return findAvailablePort({
    preferredPort,
    host: env.KAIROX_DEV_PORT_CHECK_HOST || DEFAULT_PORT_CHECK_HOST
  });
}

export function buildTauriDevConfig({ port, enablePilotIdentifier = false }) {
  const resolvedPort = parsePort(port);
  const config = {
    build: {
      devUrl: `http://localhost:${resolvedPort}`,
      beforeDevCommand: TAURI_DEV_COMMAND
    }
  };

  if (enablePilotIdentifier) {
    config.identifier = buildTauriDevIdentifier(resolvedPort);
  }

  return config;
}

export function buildTauriPilotSocketPath(identifier, env = process.env) {
  if (process.platform === "win32") {
    return `\\\\.\\pipe\\tauri-pilot-${identifier}`;
  }
  return path.join(resolvePilotSocketDir(env), `tauri-pilot-${identifier}.sock`);
}

export function resolvePilotSocketDir(env = process.env) {
  const xdgRuntimeDir = env.XDG_RUNTIME_DIR;
  if (xdgRuntimeDir && isPrivateDirectory(xdgRuntimeDir)) {
    return xdgRuntimeDir;
  }
  return "/tmp";
}

export function buildTauriDevEnv(_env, port) {
  return {
    KAIROX_DEV_PORT: String(parsePort(port)),
    KAIROX_DEV_STRICT_PORT: "1"
  };
}

export function buildTauriDevArgs(args, config) {
  const separatorIndex = args.indexOf("--");
  const configArgs = ["--config", JSON.stringify(config)];
  if (separatorIndex === -1) {
    return [...args, ...configArgs];
  }
  return [...args.slice(0, separatorIndex), ...configArgs, ...args.slice(separatorIndex)];
}

export function hasPilotFeature(args) {
  for (let index = 0; index < args.length; index++) {
    const arg = args[index];

    if (arg.startsWith("--features=")) {
      if (featureListHasPilot(arg.slice("--features=".length))) {
        return true;
      }
      continue;
    }

    if (arg === "--features" || arg === "-f") {
      for (let featureIndex = index + 1; featureIndex < args.length; featureIndex++) {
        const featureArg = args[featureIndex];
        if (featureArg.startsWith("-")) {
          break;
        }
        if (featureListHasPilot(featureArg)) {
          return true;
        }
      }
    }
  }

  return false;
}

function featureListHasPilot(value) {
  return String(value)
    .split(/[,\s]+/)
    .filter(Boolean)
    .includes("pilot");
}

function isPrivateDirectory(dir) {
  try {
    const stat = fs.statSync(dir);
    const uid = typeof process.getuid === "function" ? process.getuid() : stat.uid;
    return stat.isDirectory() && stat.uid === uid && (stat.mode & 0o077) === 0;
  } catch {
    return false;
  }
}
