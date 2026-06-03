import assert from "node:assert/strict";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { after, describe, it } from "node:test";
import {
  buildTauriDevConfig,
  buildTauriDevEnv,
  buildTauriDevIdentifier,
  buildTauriDevArgs,
  buildTauriPilotSocketPath,
  findAvailablePort,
  hasPilotFeature,
  resolveTauriDevPort
} from "./dev-port.mjs";

const openServers = [];
const tempDirs = [];

after(async () => {
  await Promise.all(
    openServers.map(
      (server) =>
        new Promise((resolve) => {
          server.close(resolve);
        })
    )
  );
  for (const dir of tempDirs) {
    fs.rmSync(dir, { force: true, recursive: true });
  }
});

function listen(port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      openServers.push(server);
      resolve(server);
    });
  });
}

describe("dev port helpers", () => {
  it("skips an occupied preferred port", async () => {
    const preferredPort = await findAvailablePort({
      preferredPort: 49_200,
      host: "127.0.0.1"
    });
    await listen(preferredPort);

    const resolvedPort = await findAvailablePort({
      preferredPort,
      host: "127.0.0.1",
      maxAttempts: 20
    });

    assert.notEqual(resolvedPort, preferredPort);
    assert.ok(resolvedPort > preferredPort);
  });

  it("builds a Tauri dev config that matches the selected Vite port", () => {
    assert.deepEqual(buildTauriDevConfig({ port: 14_217, enablePilotIdentifier: true }), {
      build: {
        devUrl: "http://localhost:14217",
        beforeDevCommand: "bun run dev"
      },
      identifier: "dev.kairox.agent.dev14217"
    });
  });

  it("passes the selected port to Vite with strict binding enabled", () => {
    assert.deepEqual(buildTauriDevEnv({}, 14_217), {
      KAIROX_DEV_PORT: "14217",
      KAIROX_DEV_STRICT_PORT: "1"
    });
  });

  it("uses the strict preferred port without scanning", async () => {
    assert.equal(
      await resolveTauriDevPort({
        KAIROX_DEV_PORT: "14217",
        KAIROX_DEV_STRICT_PORT: "1"
      }),
      14_217
    );
  });

  it("inserts dynamic config before Tauri runner arguments", () => {
    const config = buildTauriDevConfig({ port: 14_217 });
    assert.deepEqual(buildTauriDevArgs(["dev", "--", "--runner-arg"], config), [
      "dev",
      "--config",
      JSON.stringify(config),
      "--",
      "--runner-arg"
    ]);
  });

  it("detects pilot feature arguments", () => {
    assert.equal(hasPilotFeature(["dev", "--features", "pilot"]), true);
    assert.equal(hasPilotFeature(["dev", "-f", "foo", "pilot"]), true);
    assert.equal(hasPilotFeature(["dev", "--features", "foo,typegen"]), false);
  });

  it("builds a deterministic pilot socket path", () => {
    const runtimeDir = fs.mkdtempSync(path.join(os.tmpdir(), "kairox-pilot-runtime-"));
    tempDirs.push(runtimeDir);
    fs.chmodSync(runtimeDir, 0o700);

    const identifier = buildTauriDevIdentifier(14_217);
    assert.equal(identifier, "dev.kairox.agent.dev14217");
    assert.equal(
      buildTauriPilotSocketPath(identifier, { XDG_RUNTIME_DIR: runtimeDir }),
      path.join(runtimeDir, "tauri-pilot-dev.kairox.agent.dev14217.sock")
    );
    assert.equal(
      buildTauriPilotSocketPath(identifier, { XDG_RUNTIME_DIR: "/tmp" }),
      "/tmp/tauri-pilot-dev.kairox.agent.dev14217.sock"
    );
  });
});
