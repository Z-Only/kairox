import { describe, it } from "vitest";

import marketplacePaneSource from "../MarketplacePane.vue?raw";
import skillDiscoverListSource from "../skills/SkillDiscoverList.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("source filter primitive migration", () => {
  it("keeps marketplace and skills source filters on shared chip primitives", () => {
    for (const source of [marketplacePaneSource, skillDiscoverListSource]) {
      expectSourceMigration(source, {
        required: ["KxChipGroup", "KxChipButton"],
        forbidden: [":class=\"['chip'", ".source-filter .chip", ".chip.active"]
      });
    }
  });
});
