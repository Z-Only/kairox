import { afterEach, describe, expect, it } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import type { SkillCatalogEntry, SkillInstallTarget } from "@/generated/commands";
import SkillCatalogDetail from "./SkillCatalogDetail.vue";

const fullEntry: SkillCatalogEntry = {
  catalog_id: "skillhub/docs-helper",
  name: "Docs Helper",
  description: "Summarize documentation automatically.",
  source: "skillhub",
  source_url: "https://registry.example/docs-helper",
  install_count: 1234,
  github_stars: 2500,
  security_score: 91,
  rating: 4.8,
  package: "skillhub/docs-helper",
  package_url: "https://registry.example/download/docs-helper"
};

function mountDetail(
  overrides: {
    entry?: Partial<SkillCatalogEntry>;
    installTarget?: SkillInstallTarget;
    installing?: boolean;
  } = {}
) {
  const entry: SkillCatalogEntry = { ...fullEntry, ...overrides.entry };
  return mountWithPlugins(SkillCatalogDetail, {
    props: {
      entry,
      installTarget: overrides.installTarget ?? "user",
      installing: overrides.installing ?? false
    },
    attachTo: document.body
  });
}

/** Query the teleported drawer panel via document.body. */
function panel(): Element | null {
  return document.body.querySelector('[data-test="skill-catalog-detail"]');
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("SkillCatalogDetail", () => {
  it("renders the entry name as drawer title", () => {
    mountDetail();
    expect(panel()?.querySelector(".kx-drawer__title")?.textContent).toBe("Docs Helper");
  });

  it("displays the description when provided", () => {
    mountDetail();
    expect(panel()?.querySelector(".description")?.textContent).toBe(
      "Summarize documentation automatically."
    );
  });

  it("shows fallback text when description is empty", () => {
    mountDetail({ entry: { description: "" } });
    expect(panel()?.querySelector(".description")?.textContent).toBe("No description provided.");
  });

  it("renders all metadata fields when present", () => {
    mountDetail();
    const text = panel()?.textContent ?? "";
    expect(text).toContain("Source");
    expect(text).toContain("skillhub");
    expect(text).toContain("Install target");
    expect(text).toContain("Downloads");
    expect(text).toContain("1,234");
    expect(text).toContain("Stars");
    expect(text).toContain("2,500");
    expect(text).toContain("Security");
    expect(text).toContain("91");
    expect(text).toContain("Rating");
    expect(text).toContain("4.8");
  });

  it("hides optional metadata fields when null", () => {
    mountDetail({
      entry: {
        install_count: null as unknown as number,
        github_stars: null as unknown as number,
        security_score: null,
        rating: null
      }
    });
    const text = panel()?.textContent ?? "";
    expect(text).not.toContain("Downloads");
    expect(text).not.toContain("Stars");
    expect(text).not.toContain("Security");
    expect(text).not.toContain("Rating");
  });

  it("renders source and package links when URLs are provided", () => {
    mountDetail();
    const links = panel()?.querySelectorAll("a") ?? [];
    expect(links).toHaveLength(2);

    const sourceLink = links[0] as HTMLAnchorElement;
    expect(sourceLink.textContent).toBe("View source");
    expect(sourceLink.href).toBe("https://registry.example/docs-helper");
    expect(sourceLink.target).toBe("_blank");
    expect(sourceLink.rel).toBe("noopener noreferrer");

    const packageLink = links[1] as HTMLAnchorElement;
    expect(packageLink.textContent).toBe("Download package");
    expect(packageLink.href).toBe("https://registry.example/download/docs-helper");
  });

  it("hides links when URLs are absent", () => {
    mountDetail({
      entry: { source_url: "", package_url: null }
    });
    const links = panel()?.querySelectorAll(".detail-links a") ?? [];
    expect(links).toHaveLength(0);
  });

  it("shows project label when installTarget is project", () => {
    mountDetail({ installTarget: "project" });
    expect(panel()?.textContent).toContain("Project");
  });

  it("shows user label when installTarget is user", () => {
    mountDetail({ installTarget: "user" });
    expect(panel()?.textContent).toContain("User");
  });

  it("displays install button with target in label", () => {
    mountDetail({ installTarget: "user" });
    const installBtn = panel()?.querySelector('[data-test="skill-catalog-detail-install"]');
    expect(installBtn?.textContent?.trim()).toBe("Install to User");
  });

  it("disables install button and shows installing text when installing", () => {
    mountDetail({ installing: true });
    const installBtn = panel()?.querySelector(
      '[data-test="skill-catalog-detail-install"]'
    ) as HTMLButtonElement | null;
    expect(installBtn?.textContent?.trim()).toBe("Installing…");
    expect(installBtn?.disabled).toBe(true);
  });

  it("emits install event with entry when install button is clicked", async () => {
    const wrapper = mountDetail();
    const installBtn = panel()?.querySelector(
      '[data-test="skill-catalog-detail-install"]'
    ) as HTMLButtonElement;
    installBtn.click();
    await wrapper.vm.$nextTick();

    expect(wrapper.emitted("install")).toHaveLength(1);
    expect(wrapper.emitted("install")![0]).toEqual([fullEntry]);
  });

  it("emits close when the drawer header close button is clicked", async () => {
    const wrapper = mountDetail();
    const closeBtn = document.body.querySelector(".drawer-close-btn") as HTMLButtonElement;
    closeBtn.click();
    await wrapper.vm.$nextTick();

    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("emits close when the footer close button is clicked", async () => {
    const wrapper = mountDetail();
    const footer = panel()?.querySelector(".kx-drawer__footer");
    // The second button in the footer is the Close button
    const buttons = footer?.querySelectorAll("button") ?? [];
    const closeBtn = Array.from(buttons).find(
      (b) => b.textContent?.trim() === "Close"
    ) as HTMLButtonElement;
    expect(closeBtn).toBeDefined();
    closeBtn.click();
    await wrapper.vm.$nextTick();

    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("sets the drawer panel data-test attribute", () => {
    mountDetail();
    expect(panel()).not.toBeNull();
  });
});
