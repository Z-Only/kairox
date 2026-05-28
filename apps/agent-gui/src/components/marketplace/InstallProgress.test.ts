import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { flushPromises } from "@vue/test-utils";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import { useCatalogStore } from "@/stores/catalog";
import InstallProgress from "./InstallProgress.vue";

// ── helpers ──────────────────────────────────────────────────────────

function mountProgress(catalogId = "test-server") {
  const opts: MountWithPluginsOptions<typeof InstallProgress> = {
    reusePinia: true,
    mount: {
      attachTo: document.body,
      props: { catalogId }
    }
  };
  return mountWithPlugins(InstallProgress, opts).wrapper;
}

// ── tests ────────────────────────────────────────────────────────────

describe("InstallProgress.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    document.body.innerHTML = "";
  });

  // ── 1. Render smoke ──

  describe("render smoke", () => {
    it("renders the install-progress modal", async () => {
      const wrapper = mountProgress();
      await flushPromises();

      expect(document.body.querySelector('[data-test="install-progress"]')).not.toBeNull();
      wrapper.unmount();
    });

    it("shows spinner while install is in flight (no outcome)", async () => {
      const wrapper = mountProgress();
      await flushPromises();

      expect(document.body.querySelector(".spinner")).not.toBeNull();
      wrapper.unmount();
    });

    it("shows step list with three steps", async () => {
      const wrapper = mountProgress();
      await flushPromises();

      const steps = document.body.querySelectorAll(".steps li");
      expect(steps.length).toBe(3);
      wrapper.unmount();
    });
  });

  // ── 2. Successful install outcome ──

  describe("successful install", () => {
    it("marks all steps as ok when install succeeds with auto_start", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "installed",
        server_id: "test-server",
        started: true
      };

      const wrapper = mountProgress();
      await flushPromises();

      const okSteps = document.body.querySelectorAll(".steps li.ok");
      expect(okSteps.length).toBe(3);
      expect(document.body.querySelector(".spinner")).toBeNull();
      wrapper.unmount();
    });

    it("shows success alert for installed outcome", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "installed",
        server_id: "test-server",
        started: true
      };

      const wrapper = mountProgress();
      await flushPromises();

      const text = document.body.textContent ?? "";
      // The i18n message says "Install complete" or similar success text
      expect(text).toMatch(/install/i);
      wrapper.unmount();
    });
  });

  // ── 3. Already installed outcome ──

  describe("already installed outcome", () => {
    it("shows info alert for already_installed", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "already_installed",
        server_id: "test-server"
      };

      const wrapper = mountProgress();
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toMatch(/already/i);
      wrapper.unmount();
    });
  });

  // ── 4. Runtime missing failure ──

  describe("runtime missing failure", () => {
    it("marks runtime step as fail and shows error alert", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "runtime_missing",
        missing_runtimes: ["node", "python"]
      };

      const wrapper = mountProgress();
      await flushPromises();

      const failSteps = document.body.querySelectorAll(".steps li.fail");
      expect(failSteps.length).toBeGreaterThanOrEqual(1);
      const text = document.body.textContent ?? "";
      expect(text).toContain("node");
      expect(text).toContain("python");
      wrapper.unmount();
    });
  });

  // ── 5. Invalid env failure ──

  describe("invalid env failure", () => {
    it("marks write-config step as fail and shows error alert", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "invalid_env",
        missing_env_keys: ["API_KEY", "SECRET"]
      };

      const wrapper = mountProgress();
      await flushPromises();

      const failSteps = document.body.querySelectorAll(".steps li.fail");
      expect(failSteps.length).toBeGreaterThanOrEqual(1);
      const text = document.body.textContent ?? "";
      expect(text).toContain("API_KEY");
      expect(text).toContain("SECRET");
      wrapper.unmount();
    });
  });

  // ── 6. Close event ──

  describe("close event", () => {
    it("emits close when close button is clicked", async () => {
      const wrapper = mountProgress();
      await flushPromises();

      const closeBtn = document.body.querySelector<HTMLButtonElement>(
        '[data-test="install-close"]'
      );
      expect(closeBtn).not.toBeNull();
      closeBtn?.click();
      await flushPromises();

      expect(wrapper.emitted("close")).toBeTruthy();
      wrapper.unmount();
    });
  });

  // ── 7. Title tracks outcome ──

  describe("modal title", () => {
    it("shows installing title when in flight", async () => {
      const wrapper = mountProgress();
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toMatch(/install/i);
      wrapper.unmount();
    });

    it("shows complete title on success", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "installed",
        server_id: "test-server",
        started: true
      };

      const wrapper = mountProgress();
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toMatch(/complete/i);
      wrapper.unmount();
    });

    it("shows failed title on runtime_missing", async () => {
      const catalog = useCatalogStore();
      catalog.installState["test-server"] = {
        kind: "runtime_missing",
        missing_runtimes: ["node"]
      };

      const wrapper = mountProgress();
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toMatch(/fail/i);
      wrapper.unmount();
    });
  });
});
