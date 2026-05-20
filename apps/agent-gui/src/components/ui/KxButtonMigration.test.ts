import { describe, expect, it } from "vitest";

import sharedComponentsCss from "../../styles/components.css?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

const vueSources = import.meta.glob("../**/*.vue", {
  eager: true,
  import: "default",
  query: "?raw"
}) as Record<string, string>;

const legacyClassToken = /(?:^|[\s"'`[\],:])btn(?:$|[\s"'`\],}])/;
const legacyCssSelector = /\.btn(?:$|[\s.#:{,[>-])/;

describe("legacy button class migration", () => {
  it("keeps feature components off the global btn compatibility classes", () => {
    const offenders = Object.entries(vueSources)
      .filter(([, source]) => legacyClassToken.test(source) || legacyCssSelector.test(source))
      .map(([path]) => path);

    expect(offenders).toEqual([]);
  });

  it("does not keep global btn selector aliases in shared components CSS", () => {
    expectSourceMigration(sharedComponentsCss, {
      forbiddenPatterns: [legacyCssSelector]
    });
  });
});
