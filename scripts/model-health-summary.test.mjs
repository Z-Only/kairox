import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import test from "node:test";

import {
  classifyModelHealthIssue,
  readEvalResults,
  summarizeModelHealth
} from "./model-health-summary.mjs";

const execFileAsync = promisify(execFile);
const scriptPath = fileURLToPath(new URL("./model-health-summary.mjs", import.meta.url));

function result(overrides = {}) {
  return {
    scenario_id: "scenario",
    profile: "fake",
    passed: true,
    elapsed_ms: 10,
    ...overrides
  };
}

test("classifyModelHealthIssue maps backend failures to stable categories", () => {
  assert.equal(classifyModelHealthIssue(""), "empty_response");
  assert.equal(classifyModelHealthIssue("model returned an empty response"), "empty_response");
  assert.equal(classifyModelHealthIssue("runtime error: HTTP 429 rate limit"), "rate_limited");
  assert.equal(classifyModelHealthIssue("provider quota exceeded"), "rate_limited");
  assert.equal(classifyModelHealthIssue("runtime error: invalid API key"), "auth");
  assert.equal(classifyModelHealthIssue("request timed out after 60000ms"), "network");
  assert.equal(classifyModelHealthIssue("assistant response missing substring: ok"), "other");
});

test("summarizeModelHealth groups failed scenarios and recommendations", () => {
  const summary = summarizeModelHealth([
    result({
      scenario_id: "a",
      passed: false,
      error: "model returned an empty response; check model availability, quota, or plan"
    }),
    result({
      scenario_id: "b",
      passed: false,
      failures: ["runtime error: HTTP 429 rate limit"]
    }),
    result({
      scenario_id: "c",
      passed: false,
      failures: ["runtime error: invalid API key"]
    }),
    result({ scenario_id: "d", passed: true })
  ]);

  assert.equal(summary.total_scenarios, 4);
  assert.deepEqual(summary.failed_scenario_ids, ["a", "b", "c"]);
  assert.deepEqual(summary.category_counts, {
    empty_response: 1,
    rate_limited: 1,
    auth: 1
  });
  assert.ok(
    summary.recommendations.some((recommendation) =>
      /model availability.*quota.*plan/i.test(recommendation)
    )
  );
});

test("readEvalResults reads JSONL EvalResult lines", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-model-health-jsonl-"));
  const resultsPath = join(root, "results.jsonl");
  const results = [
    result({ scenario_id: "jsonl-pass" }),
    result({ scenario_id: "jsonl-fail", passed: false, failures: ["runtime error: timeout"] })
  ];
  await writeFile(resultsPath, `${results.map((item) => JSON.stringify(item)).join("\n")}\n\n`);

  assert.deepEqual(await readEvalResults(resultsPath), results);
});

test("readEvalResults reads report JSON results", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-model-health-report-"));
  const reportPath = join(root, "report.json");
  const results = [
    result({ scenario_id: "report-pass" }),
    result({
      scenario_id: "report-fail",
      passed: false,
      failures: ["runtime error: invalid API key"]
    })
  ];
  await writeFile(reportPath, JSON.stringify({ summary: { total: 2 }, results }, null, 2));

  assert.deepEqual(await readEvalResults(reportPath), results);
});

test("CLI --json prints only the model health JSON and exits zero for failures", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-model-health-cli-json-"));
  const resultsPath = join(root, "results.jsonl");
  await writeFile(
    resultsPath,
    `${JSON.stringify(
      result({
        scenario_id: "cli-fail",
        passed: false,
        failures: ["runtime error: HTTP 429 rate limit"]
      })
    )}\n`
  );

  const { stdout, stderr } = await execFileAsync(process.execPath, [
    scriptPath,
    resultsPath,
    "--json"
  ]);

  assert.equal(stderr, "");
  assert.deepEqual(JSON.parse(stdout), {
    total_scenarios: 1,
    failed_scenario_ids: ["cli-fail"],
    category_counts: {
      rate_limited: 1
    },
    recommendations: [
      "Rate limits or quota errors detected. Reduce concurrency, wait, or use a profile with sufficient quota."
    ]
  });
});

test("CLI human output lists totals, failures, categories, and recommendations", async () => {
  const root = await mkdtemp(join(tmpdir(), "kairox-model-health-cli-human-"));
  const reportPath = join(root, "report.json");
  await writeFile(
    reportPath,
    JSON.stringify({
      summary: { total: 1 },
      results: [
        result({
          scenario_id: "human-fail",
          passed: false,
          error: "model returned empty response"
        })
      ]
    })
  );

  const { stdout, stderr } = await execFileAsync(process.execPath, [scriptPath, reportPath]);

  assert.equal(stderr, "");
  assert.match(stdout, /Total scenarios: 1/);
  assert.match(stdout, /Failed scenarios: human-fail/);
  assert.match(stdout, /empty_response: 1/);
  assert.match(stdout, /model availability.*quota.*plan/i);
});

test("CLI --help prints usage", async () => {
  const { stdout, stderr } = await execFileAsync(process.execPath, [scriptPath, "--help"]);

  assert.equal(stderr, "");
  assert.match(stdout, /Usage: node scripts\/model-health-summary\.mjs/);
});
