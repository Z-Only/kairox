import { describe, expect, it } from "vitest";
import en from "./en.json";
import zhCN from "./zh-CN.json";

type Messages = Record<string, unknown>;

/**
 * Flatten a nested message bundle into dot-keyed entries so each leaf becomes
 * a single comparable key/value pair (e.g. `chatStream.toolCall.diffPreview`).
 *
 * Only plain objects are descended into. Arrays and primitives are treated as
 * leaf values — vue-i18n message bundles do not nest arrays of objects, so a
 * shallow leaf check is sufficient.
 */
function flatten(input: Messages, prefix = ""): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const key of Object.keys(input)) {
    const value = (input as Record<string, unknown>)[key];
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      Object.assign(out, flatten(value as Messages, fullKey));
    } else {
      out[fullKey] = value;
    }
  }
  return out;
}

const enFlat = flatten(en as Messages);
const zhFlat = flatten(zhCN as Messages);
const enKeys = Object.keys(enFlat).sort();
const zhKeys = Object.keys(zhFlat).sort();

/**
 * `en.json` is the source of truth (matches `fallbackLocale: "en"` in
 * `apps/agent-gui/src/locales/index.ts`). Any new string MUST be added to
 * `en.json` first, then mirrored to `zh-CN.json`; otherwise vue-i18n will
 * silently render the literal key path (e.g. `chatStream.toolCall.diffPreview`)
 * to end users.
 */
describe("locales — en ↔ zh-CN coverage", () => {
  it("zh-CN has the exact same key set as en (source of truth)", () => {
    const enSet = new Set(enKeys);
    const zhSet = new Set(zhKeys);
    const missingInZh = enKeys.filter((k) => !zhSet.has(k));
    const extraInZh = zhKeys.filter((k) => !enSet.has(k));

    expect(
      missingInZh,
      `Missing in apps/agent-gui/src/locales/zh-CN.json: ${JSON.stringify(missingInZh, null, 2)}`
    ).toEqual([]);
    expect(
      extraInZh,
      `Extra in apps/agent-gui/src/locales/zh-CN.json (not in en.json): ${JSON.stringify(extraInZh, null, 2)}`
    ).toEqual([]);

    // Belt-and-suspenders: structural equality of sorted key lists.
    expect(zhKeys).toEqual(enKeys);
  });

  it("every leaf value in en.json is a non-empty string", () => {
    const offenders: string[] = [];
    for (const [key, value] of Object.entries(enFlat)) {
      if (typeof value !== "string" || value.length === 0) {
        offenders.push(`${key} = ${JSON.stringify(value)}`);
      }
    }
    expect(offenders, `Empty / non-string leaves in en.json:\n${offenders.join("\n")}`).toEqual([]);
  });

  it("every leaf value in zh-CN.json is a non-empty string", () => {
    const offenders: string[] = [];
    for (const [key, value] of Object.entries(zhFlat)) {
      if (typeof value !== "string" || value.length === 0) {
        offenders.push(`${key} = ${JSON.stringify(value)}`);
      }
    }
    expect(offenders, `Empty / non-string leaves in zh-CN.json:\n${offenders.join("\n")}`).toEqual(
      []
    );
  });

  it("no value equals its own dot-key (catches forgotten translations)", () => {
    // When vue-i18n cannot find a key it returns the key path verbatim. If a
    // developer copy-pastes that fallback string into the locale file (or just
    // types the key as the value), this guard catches it.
    const enOffenders = Object.entries(enFlat)
      .filter(([k, v]) => v === k)
      .map(([k]) => k);
    const zhOffenders = Object.entries(zhFlat)
      .filter(([k, v]) => v === k)
      .map(([k]) => k);

    expect(
      enOffenders,
      `en.json values that equal their key path: ${JSON.stringify(enOffenders, null, 2)}`
    ).toEqual([]);
    expect(
      zhOffenders,
      `zh-CN.json values that equal their key path: ${JSON.stringify(zhOffenders, null, 2)}`
    ).toEqual([]);
  });

  it("flattens to a sensible number of keys (sanity check, not a snapshot of values)", () => {
    // Soft tripwire: if this falls drastically (e.g. someone deletes a whole
    // section) or balloons unexpectedly, the assertion message points at the
    // file. We deliberately do NOT snapshot the actual key list — that would
    // create churn on every legitimate string addition.
    expect(enKeys.length, `en.json key count looks suspicious: ${enKeys.length}`).toBeGreaterThan(
      100
    );
    expect(zhKeys.length).toBe(enKeys.length);
  });
});
