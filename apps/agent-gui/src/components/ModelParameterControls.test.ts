import { describe, it, expect } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import ModelParameterControls from "./ModelParameterControls.vue";

function mountControls(props: Partial<InstanceType<typeof ModelParameterControls>["$props"]> = {}) {
  return mountWithPlugins(ModelParameterControls, {
    props: {
      idPrefix: "test",
      open: false,
      ...props
    }
  });
}

describe("ModelParameterControls", () => {
  describe("collapsed state (open=false)", () => {
    it("renders the toggle button with collapsed indicator", () => {
      const wrapper = mountControls({ open: false });
      const toggle = wrapper.find(".model-form__toggle");
      expect(toggle.exists()).toBe(true);
      expect(toggle.text()).toContain("▸");
    });

    it("does not render the parameter grid when closed", () => {
      const wrapper = mountControls({ open: false });
      expect(wrapper.find(".model-form__grid").exists()).toBe(false);
    });

    it("does not render any input fields when closed", () => {
      const wrapper = mountControls({ open: false });
      expect(wrapper.findAll("input")).toHaveLength(0);
    });
  });

  describe("expanded state (open=true)", () => {
    it("renders the toggle button with expanded indicator", () => {
      const wrapper = mountControls({ open: true });
      const toggle = wrapper.find(".model-form__toggle");
      expect(toggle.text()).toContain("▾");
    });

    it("renders the 3-column parameter grid", () => {
      const wrapper = mountControls({ open: true });
      const grid = wrapper.find(".model-form__grid");
      expect(grid.exists()).toBe(true);
      expect(grid.classes()).toContain("model-form__grid--3col");
    });

    it("renders all six parameter input fields", () => {
      const wrapper = mountControls({ open: true, idPrefix: "p" });
      expect(wrapper.find('[data-test="p-ctx"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="p-out"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="p-temp"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="p-top-p"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="p-top-k"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="p-max-tokens"]').exists()).toBe(true);
    });
  });

  describe("prop values flow to inputs", () => {
    it("passes contextWindow value to the context window input", () => {
      const wrapper = mountControls({
        open: true,
        idPrefix: "m",
        contextWindow: "128000"
      });
      const input = wrapper.find('[data-test="m-ctx"]');
      expect((input.element as HTMLInputElement).value).toBe("128000");
    });

    it("passes outputLimit value to the output limit input", () => {
      const wrapper = mountControls({
        open: true,
        idPrefix: "m",
        outputLimit: "4096"
      });
      const input = wrapper.find('[data-test="m-out"]');
      expect((input.element as HTMLInputElement).value).toBe("4096");
    });

    it("passes temperature value to the temperature input", () => {
      const wrapper = mountControls({
        open: true,
        idPrefix: "m",
        temperature: "0.7"
      });
      const input = wrapper.find('[data-test="m-temp"]');
      expect((input.element as HTMLInputElement).value).toBe("0.7");
    });

    it("passes topP value to the top-p input", () => {
      const wrapper = mountControls({
        open: true,
        idPrefix: "m",
        topP: "0.9"
      });
      const input = wrapper.find('[data-test="m-top-p"]');
      expect((input.element as HTMLInputElement).value).toBe("0.9");
    });

    it("passes topK value to the top-k input", () => {
      const wrapper = mountControls({
        open: true,
        idPrefix: "m",
        topK: "40"
      });
      const input = wrapper.find('[data-test="m-top-k"]');
      expect((input.element as HTMLInputElement).value).toBe("40");
    });

    it("passes maxTokens value to the max-tokens input", () => {
      const wrapper = mountControls({
        open: true,
        idPrefix: "m",
        maxTokens: "2048"
      });
      const input = wrapper.find('[data-test="m-max-tokens"]');
      expect((input.element as HTMLInputElement).value).toBe("2048");
    });

    it("renders empty inputs when props use default empty strings", () => {
      const wrapper = mountControls({ open: true, idPrefix: "m" });
      const input = wrapper.find('[data-test="m-ctx"]');
      expect((input.element as HTMLInputElement).value).toBe("");
    });
  });

  describe("toggle emit", () => {
    it("emits toggle when the toggle button is clicked", async () => {
      const wrapper = mountControls({ open: false });
      await wrapper.find(".model-form__toggle").trigger("click");
      expect(wrapper.emitted("toggle")).toHaveLength(1);
    });

    it("emits toggle from expanded state as well", async () => {
      const wrapper = mountControls({ open: true });
      await wrapper.find(".model-form__toggle").trigger("click");
      expect(wrapper.emitted("toggle")).toHaveLength(1);
    });
  });

  describe("update emits from input changes", () => {
    it("emits update:contextWindow when context window input changes", async () => {
      const wrapper = mountControls({ open: true, idPrefix: "u" });
      const input = wrapper.find('[data-test="u-ctx"]');
      await input.setValue("64000");
      const emitted = wrapper.emitted("update:contextWindow");
      expect(emitted).toBeTruthy();
      expect(emitted![emitted!.length - 1]).toEqual(["64000"]);
    });

    it("emits update:outputLimit when output limit input changes", async () => {
      const wrapper = mountControls({ open: true, idPrefix: "u" });
      const input = wrapper.find('[data-test="u-out"]');
      await input.setValue("8192");
      const emitted = wrapper.emitted("update:outputLimit");
      expect(emitted).toBeTruthy();
      expect(emitted![emitted!.length - 1]).toEqual(["8192"]);
    });

    it("emits update:temperature when temperature input changes", async () => {
      const wrapper = mountControls({ open: true, idPrefix: "u" });
      const input = wrapper.find('[data-test="u-temp"]');
      await input.setValue("1.5");
      const emitted = wrapper.emitted("update:temperature");
      expect(emitted).toBeTruthy();
      expect(emitted![emitted!.length - 1]).toEqual(["1.5"]);
    });

    it("emits update:topP when top-p input changes", async () => {
      const wrapper = mountControls({ open: true, idPrefix: "u" });
      const input = wrapper.find('[data-test="u-top-p"]');
      await input.setValue("0.5");
      const emitted = wrapper.emitted("update:topP");
      expect(emitted).toBeTruthy();
      expect(emitted![emitted!.length - 1]).toEqual(["0.5"]);
    });

    it("emits update:topK when top-k input changes", async () => {
      const wrapper = mountControls({ open: true, idPrefix: "u" });
      const input = wrapper.find('[data-test="u-top-k"]');
      await input.setValue("50");
      const emitted = wrapper.emitted("update:topK");
      expect(emitted).toBeTruthy();
      expect(emitted![emitted!.length - 1]).toEqual(["50"]);
    });

    it("emits update:maxTokens when max-tokens input changes", async () => {
      const wrapper = mountControls({ open: true, idPrefix: "u" });
      const input = wrapper.find('[data-test="u-max-tokens"]');
      await input.setValue("1024");
      const emitted = wrapper.emitted("update:maxTokens");
      expect(emitted).toBeTruthy();
      expect(emitted![emitted!.length - 1]).toEqual(["1024"]);
    });
  });

  describe("idPrefix propagation", () => {
    it("prefixes all input data-test attributes with the given idPrefix", () => {
      const wrapper = mountControls({ open: true, idPrefix: "custom" });
      const tests = [
        "custom-ctx",
        "custom-out",
        "custom-temp",
        "custom-top-p",
        "custom-top-k",
        "custom-max-tokens"
      ];
      for (const t of tests) {
        expect(wrapper.find(`[data-test="${t}"]`).exists()).toBe(true);
      }
    });

    it("uses idPrefix in the native input id attributes", () => {
      const wrapper = mountControls({ open: true, idPrefix: "my" });
      expect(wrapper.find("#my-ctx").exists()).toBe(true);
      expect(wrapper.find("#my-temp").exists()).toBe(true);
    });
  });

  describe("input type attributes", () => {
    it("sets all inputs to type=number", () => {
      const wrapper = mountControls({ open: true, idPrefix: "t" });
      const inputs = wrapper.findAll("input");
      for (const input of inputs) {
        expect(input.attributes("type")).toBe("number");
      }
    });

    it("sets step/min/max on temperature input", () => {
      const wrapper = mountControls({ open: true, idPrefix: "t" });
      const temp = wrapper.find('[data-test="t-temp"]');
      expect(temp.attributes("step")).toBe("0.1");
      expect(temp.attributes("min")).toBe("0");
      expect(temp.attributes("max")).toBe("2");
    });

    it("sets step/min/max on top-p input", () => {
      const wrapper = mountControls({ open: true, idPrefix: "t" });
      const topP = wrapper.find('[data-test="t-top-p"]');
      expect(topP.attributes("step")).toBe("0.1");
      expect(topP.attributes("min")).toBe("0");
      expect(topP.attributes("max")).toBe("1");
    });

    it("sets min on top-k input", () => {
      const wrapper = mountControls({ open: true, idPrefix: "t" });
      const topK = wrapper.find('[data-test="t-top-k"]');
      expect(topK.attributes("min")).toBe("0");
    });
  });

  describe("fieldset semantics", () => {
    it("wraps controls in a fieldset with a legend", () => {
      const wrapper = mountControls();
      expect(wrapper.find("fieldset.model-form__section").exists()).toBe(true);
      expect(wrapper.find("legend").exists()).toBe(true);
    });
  });
});
