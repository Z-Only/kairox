import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
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

test(
  "dev-pilot retries the default launch once after StarPoint allows a Tauri binary that exited before pilot readiness",
  { timeout: 15_000 },
  async () => {
    const tempRoot = await mkdtemp(join(tmpdir(), "kairox-dev-pilot-starpoint-"));
    try {
      const fakeBin = resolve(tempRoot, "bin");
      const allowedMarker = resolve(tempRoot, "starpoint-allowed");
      const bunCalls = resolve(tempRoot, "bun-calls.log");
      const helper = resolve(tempRoot, "check-and-allow.sh");
      await mkdir(fakeBin, { recursive: true });

      await writeFile(
        resolve(fakeBin, "bun"),
        `#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$STARPOINT_TEST_BUN_CALLS"
if [[ "$*" == "--filter agent-gui tauri dev --features pilot" ]]; then
  echo "Finished dev profile"
  printf '\\033[1m\\033[92m     Running\\033[0m \`%s/target/debug/agent-gui-tauri\`\\n' "$PWD"
  if [[ -f "$STARPOINT_TEST_ALLOWED" ]]; then
    exit 0
  fi
  exit 0
fi
echo "unexpected bun invocation: $*" >&2
exit 1
`
      );
      await chmod(resolve(fakeBin, "bun"), 0o755);

      await writeFile(resolve(fakeBin, "cargo"), "#!/usr/bin/env bash\nexit 1\n");
      await chmod(resolve(fakeBin, "cargo"), 0o755);
      await writeFile(resolve(fakeBin, "tauri"), "#!/usr/bin/env bash\nexit 0\n");
      await chmod(resolve(fakeBin, "tauri"), 0o755);
      await writeFile(
        resolve(fakeBin, "tauri-pilot"),
        `#!/usr/bin/env bash
if [[ -f "$STARPOINT_TEST_ALLOWED" ]]; then
  exit 0
fi
exit 1
`
      );
      await chmod(resolve(fakeBin, "tauri-pilot"), 0o755);

      await writeFile(
        helper,
        `#!/usr/bin/env bash
touch "$STARPOINT_TEST_ALLOWED"
echo "allowed"
`
      );
      await chmod(helper, 0o755);

      const { stdout, stderr } = await execFileAsync("bash", ["scripts/dev-pilot.sh"], {
        cwd: repoRoot,
        env: {
          ...process.env,
          PATH: `${fakeBin}:${process.env.PATH}`,
          KAIROX_HOME: resolve(tempRoot, "home"),
          KAIROX_DEV_PILOT_SKIP_DEPS: "1",
          KAIROX_DEV_PILOT_TIMEOUT_SECS: "1",
          KAIROX_DEV_PILOT_PING_INTERVAL_SECS: "1",
          KAIROX_DEV_PILOT_ACTIVE_STARTUP_EXTRA_WAIT_SECS: "0",
          KAIROX_DEV_PILOT_APP_LOG: resolve(tempRoot, "app.log"),
          KAIROX_DEV_PILOT_VITE_LOG: resolve(tempRoot, "vite.log"),
          KAIROX_DEV_PILOT_TAURI_LOG: resolve(tempRoot, "tauri.log"),
          KAIROX_DEV_PILOT_STARPOINT_HELPER: helper,
          STARPOINT_TEST_ALLOWED: allowedMarker,
          STARPOINT_TEST_BUN_CALLS: bunCalls
        },
        timeout: 10_000
      });

      const output = `${stdout}\n${stderr}`;
      assert.match(output, /StarPoint helper reported allowed; retrying default Tauri dev command/);
      assert.doesNotMatch(output, /Starting split Vite \+ Tauri fallback/);

      const defaultLaunchCalls = (await readFile(bunCalls, "utf8"))
        .trim()
        .split("\n")
        .filter((line) => line === "--filter agent-gui tauri dev --features pilot");
      assert.equal(defaultLaunchCalls.length, 2);
    } finally {
      await rm(tempRoot, { recursive: true, force: true });
    }
  }
);

test(
  "dev-pilot retries the split Tauri command once after StarPoint allows a blocked binary",
  { timeout: 15_000 },
  async () => {
    const tempRoot = await mkdtemp(join(tmpdir(), "kairox-dev-pilot-split-starpoint-"));
    try {
      const fakeBin = resolve(tempRoot, "bin");
      const allowedMarker = resolve(tempRoot, "starpoint-allowed");
      const cargoCalls = resolve(tempRoot, "cargo-calls.log");
      const helper = resolve(tempRoot, "check-and-allow.sh");
      await mkdir(fakeBin, { recursive: true });

      await writeFile(
        resolve(fakeBin, "bun"),
        `#!/usr/bin/env bash
if [[ "$*" == "run dev" ]]; then
  node -e 'const net = require("node:net"); const server = net.createServer(); server.listen(Number(process.env.KAIROX_DEV_PORT), "127.0.0.1"); setTimeout(() => server.close(), 10000);'
  exit 0
fi
echo "unexpected bun invocation: $*" >&2
exit 1
`
      );
      await chmod(resolve(fakeBin, "bun"), 0o755);

      await writeFile(
        resolve(fakeBin, "cargo"),
        `#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$STARPOINT_TEST_CARGO_CALLS"
echo "Running $PWD/target/debug/agent-gui-tauri"
if [[ -f "$STARPOINT_TEST_ALLOWED" ]]; then
  exit 0
fi
echo "Killed: 9"
exit 137
`
      );
      await chmod(resolve(fakeBin, "cargo"), 0o755);
      await writeFile(
        resolve(fakeBin, "tauri-pilot"),
        `#!/usr/bin/env bash
if [[ -f "$STARPOINT_TEST_ALLOWED" ]]; then
  exit 0
fi
exit 1
`
      );
      await chmod(resolve(fakeBin, "tauri-pilot"), 0o755);

      await writeFile(
        helper,
        `#!/usr/bin/env bash
touch "$STARPOINT_TEST_ALLOWED"
echo "allowed"
`
      );
      await chmod(helper, 0o755);

      const { stdout, stderr } = await execFileAsync("bash", ["scripts/dev-pilot.sh"], {
        cwd: repoRoot,
        env: {
          ...process.env,
          PATH: `${fakeBin}:${process.env.PATH}`,
          KAIROX_HOME: resolve(tempRoot, "home"),
          KAIROX_DEV_PORT: "14218",
          KAIROX_DEV_STRICT_PORT: "1",
          KAIROX_DEV_PILOT_SKIP_DEPS: "1",
          KAIROX_DEV_PILOT_TIMEOUT_SECS: "1",
          KAIROX_DEV_PILOT_PING_INTERVAL_SECS: "1",
          KAIROX_DEV_PILOT_ACTIVE_STARTUP_EXTRA_WAIT_SECS: "0",
          KAIROX_DEV_PILOT_APP_LOG: resolve(tempRoot, "app.log"),
          KAIROX_DEV_PILOT_VITE_LOG: resolve(tempRoot, "vite.log"),
          KAIROX_DEV_PILOT_TAURI_LOG: resolve(tempRoot, "tauri.log"),
          KAIROX_DEV_PILOT_STARPOINT_HELPER: helper,
          STARPOINT_TEST_ALLOWED: allowedMarker,
          STARPOINT_TEST_CARGO_CALLS: cargoCalls
        },
        timeout: 10_000
      });

      const output = `${stdout}\n${stderr}`;
      assert.match(output, /Starting split Vite \+ Tauri fallback/);
      assert.match(output, /StarPoint helper reported allowed; retrying split Tauri cargo command/);

      const splitCargoCalls = (await readFile(cargoCalls, "utf8"))
        .trim()
        .split("\n")
        .filter((line) => line === "run --no-default-features --features pilot --");
      assert.equal(splitCargoCalls.length, 2);
    } finally {
      await rm(tempRoot, { recursive: true, force: true });
    }
  }
);
