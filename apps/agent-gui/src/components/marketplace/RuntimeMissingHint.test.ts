import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";

// ── tests ────────────────────────────────────────────────────────────

describe("RuntimeMissingHint.vue", () => {
  // ── 1. Render smoke ──

  describe("render smoke", () => {
    it("renders the runtime-hint container", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [{ kind: "node", min_version: ">=18.0.0", install_hint: null }]
        }
      });
      expect(wrapper.find('[data-test="runtime-hint"]').exists()).toBe(true);
    });

    it("renders one list item per requirement", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [
            { kind: "node", min_version: ">=18.0.0", install_hint: "https://nodejs.org" },
            { kind: "python", min_version: ">=3.10", install_hint: null },
            { kind: "uv", min_version: null, install_hint: null }
          ]
        }
      });
      const items = wrapper.findAll("li");
      expect(items.length).toBe(3);
    });
  });

  // ── 2. Content rendering ──

  describe("content rendering", () => {
    it("shows the runtime kind name", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [{ kind: "node", min_version: null, install_hint: null }]
        }
      });
      expect(wrapper.text()).toContain("node");
    });

    it("shows min_version when provided", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [{ kind: "node", min_version: ">=18.0.0", install_hint: null }]
        }
      });
      expect(wrapper.text()).toContain(">=18.0.0");
    });

    it("does not show version text when min_version is null", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [{ kind: "uv", min_version: null, install_hint: null }]
        }
      });
      expect(wrapper.text()).toContain("uv");
      // Only the kind text, no parenthetical version
      expect(wrapper.text()).not.toContain("(");
    });
  });

  // ── 3. Install hint link ──

  describe("install hint link", () => {
    it("renders install link when install_hint is provided", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [
            { kind: "node", min_version: ">=18.0.0", install_hint: "https://nodejs.org" }
          ]
        }
      });
      const link = wrapper.find('a[href="https://nodejs.org"]');
      expect(link.exists()).toBe(true);
      expect(link.attributes("target")).toBe("_blank");
      expect(link.attributes("rel")).toContain("noopener");
    });

    it("does not render install link when install_hint is null", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [{ kind: "python", min_version: null, install_hint: null }]
        }
      });
      expect(wrapper.find("a").exists()).toBe(false);
    });
  });

  // ── 4. Multiple requirements ──

  describe("multiple requirements", () => {
    it("renders all requirement details correctly", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: {
          requirements: [
            { kind: "node", min_version: ">=18.0.0", install_hint: "https://nodejs.org" },
            { kind: "python", min_version: ">=3.10", install_hint: "https://python.org" }
          ]
        }
      });
      const items = wrapper.findAll("li");
      expect(items[0].text()).toContain("node");
      expect(items[0].text()).toContain(">=18.0.0");
      expect(items[1].text()).toContain("python");
      expect(items[1].text()).toContain(">=3.10");

      const links = wrapper.findAll("a");
      expect(links.length).toBe(2);
      expect(links[0].attributes("href")).toBe("https://nodejs.org");
      expect(links[1].attributes("href")).toBe("https://python.org");
    });
  });

  // ── 5. Empty requirements ──

  describe("empty requirements", () => {
    it("renders no list items when requirements array is empty", () => {
      const wrapper = mount(RuntimeMissingHint, {
        props: { requirements: [] }
      });
      expect(wrapper.findAll("li").length).toBe(0);
    });
  });
});
