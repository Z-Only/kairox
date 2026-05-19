import { describe, expect, it } from "vitest";

import marketplacePaneSource from "../MarketplacePane.vue?raw";
import skillDiscoverListSource from "../skills/SkillDiscoverList.vue?raw";

describe("source filter primitive migration", () => {
  it("keeps marketplace and skills source filters on shared chip primitives", () => {
    for (const source of [marketplacePaneSource, skillDiscoverListSource]) {
      expect(source).toContain("KxChipGroup");
      expect(source).toContain("KxChipButton");
      expect(source).not.toContain(":class=\"['chip'");
      expect(source).not.toContain(".source-filter .chip");
      expect(source).not.toContain(".chip.active");
    }
  });
});
