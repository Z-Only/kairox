import { describe, expect, it } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import type { SkillCatalogEntry } from "@/generated/commands";
import SkillDiscoverCard from "./SkillDiscoverCard.vue";

const catalogEntry: SkillCatalogEntry = {
  catalog_id: "skillhub/docs-helper",
  name: "Docs Helper",
  description: "",
  source: "skillhub",
  source_url: "https://registry.example/docs-helper",
  install_count: 1234,
  github_stars: 2500,
  security_score: 91,
  rating: 4.8,
  package: "skillhub/docs-helper",
  package_url: "https://registry.example/download/docs-helper"
};

describe("SkillDiscoverCard", () => {
  it("renders catalog metadata with localized labels", () => {
    const wrapper = mountWithPlugins(SkillDiscoverCard, {
      props: {
        entry: catalogEntry,
        installing: false,
        installed: false
      }
    });

    expect(wrapper.text()).toContain("No description provided.");
    expect(wrapper.text()).toContain("1,234 installs");
    expect(wrapper.find(".sec-tag").attributes("title")).toBe("Security score: 91");
    expect(wrapper.find("a").text()).toBe("View source");
    expect(wrapper.find("button.btn-primary").text()).toBe("Install");
  });

  it("shows installed feedback and disables reinstall", () => {
    const wrapper = mountWithPlugins(SkillDiscoverCard, {
      props: {
        entry: catalogEntry,
        installing: false,
        installed: true
      }
    });

    const installButton = wrapper.find("button.btn-primary");
    expect(installButton.text()).toBe("Installed");
    expect(installButton.attributes("disabled")).toBeDefined();
  });
});
