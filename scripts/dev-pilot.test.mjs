import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("justfile exposes a dev-pilot recipe backed by the stable pilot launcher", async () => {
  const justfile = await readFile(resolve(repoRoot, "justfile"), "utf8");

  assert.match(justfile, /^dev-pilot:/m);
  assert.match(justfile, /^\s+bun run dev:pilot$/m);
});

test("dev-pilot dry run reports default and fallback launch commands", async () => {
  const { stdout } = await execFileAsync("bash", ["scripts/dev-pilot.sh"], {
    cwd: repoRoot,
    env: {
      ...process.env,
      KAIROX_DEV_PILOT_DRY_RUN: "1",
      KAIROX_DEV_PILOT_SKIP_DEPS: "1"
    },
    timeout: 10_000
  });

  assert.match(stdout, /Default command:/);
  assert.match(stdout, /bun --filter agent-gui tauri dev --features pilot/);
  assert.match(stdout, /Fallback commands:/);
  assert.match(stdout, /cd apps\/agent-gui && KAIROX_DEV_PORT=\d+ .*bun run dev/);
  assert.match(stdout, /cargo run --no-default-features --features pilot --/);
});

test("dev-pilot split fallback reuses the selected dynamic port and identifier", async () => {
  const { stdout } = await execFileAsync("bash", ["scripts/dev-pilot.sh"], {
    cwd: repoRoot,
    env: {
      ...process.env,
      KAIROX_DEV_PILOT_DRY_RUN: "1",
      KAIROX_DEV_PILOT_SKIP_DEPS: "1",
      KAIROX_DEV_PORT: "14217",
      KAIROX_DEV_STRICT_PORT: "1"
    },
    timeout: 10_000
  });

  assert.match(stdout, /Default pilot target:\n  port:\s+14217/);
  assert.match(stdout, /Fallback pilot target:\n  port:\s+14217/);
  assert.match(stdout, /identifier: dev\.kairox\.agent\.dev14217/);
  const socketMatches = [
    ...stdout.matchAll(/socket:\s+(\S*tauri-pilot-dev\.kairox\.agent\.dev14217\.sock)/g)
  ].map((match) => match[1]);
  assert.equal(socketMatches.length, 2);
  assert.equal(socketMatches[0], socketMatches[1]);
  assert.match(stdout, /KAIROX_DEV_PORT=14217 .*bun run dev/);
  assert.match(stdout, /TAURI_CONFIG=.*devUrl.*localhost:14217/);
  assert.match(stdout, /TAURI_CONFIG=.*identifier.*dev\.kairox\.agent\.dev14217/);
});
