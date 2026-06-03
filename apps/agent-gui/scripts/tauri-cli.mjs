#!/usr/bin/env node

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_DEV_PORT,
  DEFAULT_PORT_CHECK_HOST,
  buildTauriDevArgs,
  buildTauriDevConfig,
  buildTauriDevEnv,
  findAvailablePort,
  hasPilotFeature,
  isEnabled,
  parsePort
} from "./dev-port.mjs";

const args = process.argv.slice(2);
const tauriCommand = resolveTauriCommand();

if (args[0] !== "dev" || isEnabled(process.env.KAIROX_TAURI_RAW)) {
  runTauri(args, process.env);
} else {
  const port = await findAvailablePort({
    preferredPort: parsePort(process.env.KAIROX_DEV_PORT, DEFAULT_DEV_PORT),
    host: process.env.KAIROX_DEV_PORT_CHECK_HOST || DEFAULT_PORT_CHECK_HOST
  });
  const enablePilotIdentifier =
    hasPilotFeature(args) || isEnabled(process.env.KAIROX_DEV_DYNAMIC_IDENTIFIER);
  const config = buildTauriDevConfig({ port, enablePilotIdentifier });
  const devArgs = buildTauriDevArgs(args, config);
  const env = {
    ...process.env,
    ...buildTauriDevEnv(process.env, port)
  };

  console.error(`[kairox] Vite dev URL: http://localhost:${port}`);
  if (enablePilotIdentifier) {
    console.error(`[kairox] Tauri dev identifier: ${config.identifier}`);
  }

  runTauri(devArgs, env);
}

function resolveTauriCommand() {
  if (process.env.KAIROX_TAURI_BIN) {
    return process.env.KAIROX_TAURI_BIN;
  }

  const localBin = fileURLToPath(new URL("../node_modules/.bin/tauri", import.meta.url));
  if (existsSync(localBin)) {
    return localBin;
  }

  return "tauri";
}

function runTauri(runArgs, env) {
  const child = spawn(tauriCommand, runArgs, {
    env,
    stdio: "inherit"
  });

  child.on("error", (error) => {
    console.error(`[kairox] failed to start Tauri CLI: ${error.message}`);
    process.exit(1);
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 0);
  });
}
