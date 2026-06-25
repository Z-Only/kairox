import assert from "node:assert/strict";
import test from "node:test";

import { evaluateGeneratedGuardrails, runCli, USAGE } from "./check-generated-guardrails.mjs";

function createWritableCapture() {
  return {
    content: "",
    write(chunk) {
      this.content += chunk;
    }
  };
}

test("evaluateGeneratedGuardrails fails generated-only changes", () => {
  const result = evaluateGeneratedGuardrails(["apps/agent-gui/src/generated/commands.ts"]);

  assert.equal(result.ok, false);
  assert.deepEqual(result.generatedPaths, ["apps/agent-gui/src/generated/commands.ts"]);
  assert.deepEqual(result.triggerPaths, []);
  assert.match(
    result.message,
    /generated bindings changed without a Rust\/Specta\/event source trigger/
  );
  assert.match(result.message, /apps\/agent-gui\/src\/generated\/commands\.ts/);
  assert.match(result.message, /apps\/agent-gui\/src-tauri\/\*\*/);
  assert.match(result.message, /crates\/agent-core\/src\/events\.rs/);
  assert.match(result.message, /just gen-types/);
  assert.match(result.message, /only after generator\/source changes/);
});

test("evaluateGeneratedGuardrails passes generated changes with source triggers", () => {
  assert.deepEqual(
    evaluateGeneratedGuardrails([
      "apps/agent-gui/src/generated/events.ts",
      "apps/agent-gui/src-tauri/src/commands/chat.rs"
    ]),
    {
      ok: true,
      generatedPaths: ["apps/agent-gui/src/generated/events.ts"],
      triggerPaths: ["apps/agent-gui/src-tauri/src/commands/chat.rs"],
      message: "Generated bindings changed with a Rust/Specta/event source trigger."
    }
  );

  assert.equal(
    evaluateGeneratedGuardrails([
      "apps/agent-gui/src/generated/commands.ts",
      "crates/agent-core/src/events.rs"
    ]).ok,
    true
  );
});

test("evaluateGeneratedGuardrails passes when no generated files changed", () => {
  assert.deepEqual(evaluateGeneratedGuardrails(["README.md", "scripts/example.mjs"]), {
    ok: true,
    generatedPaths: [],
    triggerPaths: [],
    message: "No generated bindings changed."
  });
});

test("runCli prints help", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();

  const exitCode = await runCli(["--help"], { stdout, stderr });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.equal(stdout.content, USAGE);
  assert.match(stdout.content, /Usage: node scripts\/check-generated-guardrails\.mjs/);
});

test("runCli falls back from empty staged diff to unstaged diff", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const calls = [];

  const exitCode = await runCli([], {
    stdout,
    stderr,
    execFile: async (command, args) => {
      calls.push([command, args]);
      if (args.join(" ") === "diff --name-only --cached") {
        return { stdout: "\n" };
      }
      if (args.join(" ") === "diff --name-only") {
        return { stdout: "apps/agent-gui/src/generated/commands.ts\n" };
      }
      throw new Error(`unexpected command: ${command} ${args.join(" ")}`);
    }
  });

  assert.equal(exitCode, 1);
  assert.deepEqual(calls, [
    ["git", ["diff", "--name-only", "--cached"]],
    ["git", ["diff", "--name-only"]]
  ]);
  assert.equal(stdout.content, "");
  assert.match(
    stderr.content,
    /generated bindings changed without a Rust\/Specta\/event source trigger/
  );
});

test("runCli reads base diff when --base is supplied", async () => {
  const stdout = createWritableCapture();
  const stderr = createWritableCapture();
  const calls = [];

  const exitCode = await runCli(["--base", "origin/main"], {
    stdout,
    stderr,
    execFile: async (command, args) => {
      calls.push([command, args]);
      return {
        stdout: "apps/agent-gui/src/generated/events.ts\ncrates/agent-core/src/context_types.rs\n"
      };
    }
  });

  assert.equal(exitCode, 0);
  assert.equal(stderr.content, "");
  assert.deepEqual(calls, [["git", ["diff", "--name-only", "origin/main...HEAD"]]]);
  assert.match(
    stdout.content,
    /Generated bindings changed with a Rust\/Specta\/event source trigger/
  );
});
