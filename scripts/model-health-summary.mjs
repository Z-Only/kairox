import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

export const USAGE = `Usage: node scripts/model-health-summary.mjs <results.jsonl|report.json> [--json]

Diagnoses completed kairox-eval output for common model backend health issues.

Options:
  --json        Print stable JSON only.
  --help, -h    Show this help.
`;

const CATEGORY_ORDER = ["empty_response", "rate_limited", "auth", "network", "other"];

const RECOMMENDATIONS = {
  empty_response:
    "Empty model responses detected. Check model availability, quota, and plan limits for the configured profile.",
  rate_limited:
    "Rate limits or quota errors detected. Reduce concurrency, wait, or use a profile with sufficient quota.",
  auth: "Authentication errors detected. Check API keys, provider credentials, and profile configuration.",
  network:
    "Network or timeout errors detected. Check connectivity, provider endpoint, and timeout settings.",
  other: "Other eval failures remain. Inspect scenario failures and trace output."
};

class UsageError extends Error {}

export function classifyModelHealthIssue(text) {
  const value = String(text ?? "").trim();
  if (value === "") {
    return "empty_response";
  }

  const lower = value.toLowerCase();
  if (
    /\b(empty|blank)\s+(model\s+|assistant\s+)?response\b/.test(lower) ||
    /\bmodel returned an? empty response\b/.test(lower) ||
    /\bno response (text|from model)\b/.test(lower)
  ) {
    return "empty_response";
  }
  if (/\b(429|too many requests|rate[-_\s]?limit(?:ed)?|quota|insufficient_quota)\b/.test(lower)) {
    return "rate_limited";
  }
  if (
    /\b(401|403|unauthori[sz]ed|forbidden|authentication|authorization|auth)\b/.test(lower) ||
    /\binvalid\s+(api\s+)?key\b/.test(lower) ||
    /\bapi\s+key\s+(is\s+)?invalid\b/.test(lower)
  ) {
    return "auth";
  }
  if (
    /\b(timeout|timed out|network|fetch failed|dns|socket|connection|tls)\b/.test(lower) ||
    /\b(econnreset|econnrefused|etimedout|enotfound|eai_again)\b/.test(lower)
  ) {
    return "network";
  }

  return "other";
}

export function summarizeModelHealth(results) {
  if (!Array.isArray(results)) {
    throw new Error("Expected an array of eval results.");
  }

  const failedScenarioIds = [];
  const categoryCounts = {};

  results.forEach((result, index) => {
    if (!isFailedResult(result)) {
      return;
    }

    failedScenarioIds.push(scenarioId(result, index));
    for (const issueText of issueTexts(result)) {
      const category = classifyModelHealthIssue(issueText);
      categoryCounts[category] = (categoryCounts[category] ?? 0) + 1;
    }
  });

  return {
    total_scenarios: results.length,
    failed_scenario_ids: failedScenarioIds,
    category_counts: orderedCounts(categoryCounts),
    recommendations: recommendationsFor(categoryCounts)
  };
}

export async function readEvalResults(path) {
  const raw = await readFile(path, "utf8");
  const trimmed = raw.trim();
  if (trimmed === "") {
    return [];
  }

  if (trimmed.startsWith("[")) {
    const parsed = parseJson(trimmed, path);
    if (Array.isArray(parsed)) {
      return parsed;
    }
    throw new Error(`Expected JSON array of eval results: ${path}`);
  }

  if (trimmed.startsWith("{")) {
    const parsed = tryParseJson(trimmed);
    if (parsed && Array.isArray(parsed.results)) {
      return parsed.results;
    }
    if (parsed && ("summary" in parsed || "results" in parsed)) {
      throw new Error(`Expected report JSON with a results array: ${path}`);
    }
  }

  return raw
    .split(/\r?\n/)
    .map((line, index) => ({ line: line.trim(), number: index + 1 }))
    .filter(({ line }) => line !== "")
    .map(({ line, number }) => parseJsonLine(line, path, number));
}

export function formatHumanSummary(summary) {
  const lines = [
    "Model health summary",
    `Total scenarios: ${summary.total_scenarios}`,
    `Failed scenarios: ${
      summary.failed_scenario_ids.length > 0 ? summary.failed_scenario_ids.join(", ") : "none"
    }`
  ];

  const categories = orderedCategories(summary.category_counts);
  if (categories.length === 0) {
    lines.push("Categories: none");
  } else {
    lines.push("Categories:");
    for (const category of categories) {
      lines.push(`  ${category}: ${summary.category_counts[category]}`);
    }
  }

  lines.push("Recommendations:");
  for (const recommendation of summary.recommendations) {
    lines.push(`  - ${recommendation}`);
  }

  return `${lines.join("\n")}\n`;
}

export async function runCli(
  argv = process.argv.slice(2),
  { stdout = process.stdout, stderr = process.stderr } = {}
) {
  try {
    const args = parseArgs(argv);
    if (args.help) {
      stdout.write(USAGE);
      return 0;
    }

    const results = await readEvalResults(args.path);
    const summary = summarizeModelHealth(results);
    stdout.write(args.json ? `${JSON.stringify(summary, null, 2)}\n` : formatHumanSummary(summary));
    return 0;
  } catch (error) {
    const message = error instanceof UsageError ? `${error.message}\n\n${USAGE}` : error.message;
    stderr.write(`${message}\n`);
    return 1;
  }
}

function parseArgs(argv) {
  const parsed = {
    help: false,
    json: false,
    path: null
  };

  for (const arg of argv) {
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      continue;
    }
    if (arg === "--json") {
      parsed.json = true;
      continue;
    }
    if (arg.startsWith("-")) {
      throw new UsageError(`Unknown argument: ${arg}`);
    }
    if (parsed.path) {
      throw new UsageError(`Unexpected extra input path: ${arg}`);
    }
    parsed.path = arg;
  }

  if (!parsed.help && !parsed.path) {
    throw new UsageError("Missing required eval results path.");
  }

  return parsed;
}

function parseJson(raw, path) {
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`Failed to parse JSON in ${path}: ${error.message}`);
  }
}

function tryParseJson(raw) {
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function parseJsonLine(line, path, number) {
  try {
    return JSON.parse(line);
  } catch (error) {
    throw new Error(`Failed to parse JSONL ${path}:${number}: ${error.message}`);
  }
}

function isFailedResult(result) {
  return (
    result?.passed === false || nonEmptyString(result?.error) || arrayHasValues(result?.failures)
  );
}

function scenarioId(result, index) {
  return String(result?.scenario_id ?? result?.scenarioId ?? result?.id ?? `scenario-${index + 1}`);
}

function issueTexts(result) {
  const texts = [];
  if (nonEmptyString(result?.error)) {
    texts.push(result.error);
  }
  if (Array.isArray(result?.failures)) {
    texts.push(...result.failures.map((failure) => String(failure)));
  } else if (nonEmptyString(result?.failures)) {
    texts.push(result.failures);
  }

  const assistantResponse = result?.assistant_response ?? result?.assistantResponse;
  if (typeof assistantResponse === "string" && assistantResponse.trim() === "") {
    texts.push("");
  }

  return texts.length > 0 ? texts : ["unknown failure"];
}

function orderedCounts(counts) {
  const ordered = {};
  for (const category of orderedCategories(counts)) {
    ordered[category] = counts[category];
  }
  return ordered;
}

function orderedCategories(counts) {
  const known = CATEGORY_ORDER.filter((category) => counts[category] > 0);
  const unknown = Object.keys(counts)
    .filter((category) => !CATEGORY_ORDER.includes(category) && counts[category] > 0)
    .sort();
  return [...known, ...unknown];
}

function recommendationsFor(categoryCounts) {
  const categories = orderedCategories(categoryCounts);
  if (categories.length === 0) {
    return ["No model health issues detected in failed eval results."];
  }
  return categories.map((category) => RECOMMENDATIONS[category] ?? RECOMMENDATIONS.other);
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() !== "";
}

function arrayHasValues(value) {
  return Array.isArray(value) && value.length > 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const exitCode = await runCli();
  process.exitCode = exitCode;
}
