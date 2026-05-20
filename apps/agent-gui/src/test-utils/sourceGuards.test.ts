import { describe, expect, it } from "vitest";
import {
  expectSourceMigration,
  expectSourceNotToContain,
  expectSourceNotToMatch,
  expectSourceToContain,
  expectSourceToMatch
} from "./sourceGuards";

describe("sourceGuards", () => {
  const source = `
    <SettingsCardItem density="compact">
      <KxToolbarAction />
    </SettingsCardItem>

    .settings-card-item {
      align-items: start;
    }
  `;

  it("checks required and forbidden source fragments", () => {
    expectSourceMigration(source, {
      required: ["SettingsCardItem", "KxToolbarAction"],
      forbidden: [".legacy-row", "btn-primary"]
    });
  });

  it("checks required and forbidden source patterns", () => {
    expectSourceMigration(source, {
      requiredPatterns: [/density="compact"/, /\.settings-card-item\s*\{[^}]*align-items:\s*start/],
      forbiddenPatterns: [/\.legacy-row\s*\{/, /\btag-warning\b/]
    });
  });

  it("keeps the single-purpose helpers available for narrow migration tests", () => {
    expectSourceToContain(source, ["SettingsCardItem"]);
    expectSourceNotToContain(source, ["LegacyCard"]);
    expectSourceToMatch(source, [/align-items:\s*start/]);
    expectSourceNotToMatch(source, [/display:\s*grid/]);
  });

  it("surfaces the missing fragment in assertion messages", () => {
    expect(() => expectSourceToContain(source, ["MissingComponent"])).toThrow(
      /source should contain MissingComponent/
    );
  });
});
