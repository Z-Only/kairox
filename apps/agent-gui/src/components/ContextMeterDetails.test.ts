import { describe, it, expect } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import ContextMeterDetails from "./ContextMeterDetails.vue";

function mountDetails(props: Partial<InstanceType<typeof ContextMeterDetails>["$props"]> = {}) {
  return mountWithPlugins(ContextMeterDetails, {
    props: {
      bySource: [["history", 5000]],
      outputReservation: 2000,
      displayBudgetTokens: 10000,
      compacting: false,
      compressionRatioTooLow: false,
      needsAutoCompression: false,
      ...props
    }
  });
}

describe("ContextMeterDetails", () => {
  describe("by-source table rows", () => {
    it("renders a row for each source in bySource", () => {
      const wrapper = mountDetails({
        bySource: [
          ["history", 3000],
          ["system", 2000],
          ["memory", 1000]
        ]
      });
      expect(wrapper.find('[data-test="context-meter-row-history"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="context-meter-row-system"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="context-meter-row-memory"]').exists()).toBe(true);
    });

    it("renders no source rows when bySource is empty", () => {
      const wrapper = mountDetails({ bySource: [] });
      // Only the reserved row should remain
      const rows = wrapper.findAll("tr");
      expect(rows).toHaveLength(1);
      expect(wrapper.find('[data-test="context-meter-reserved"]').exists()).toBe(true);
    });

    it("formats token counts using k notation for large values", () => {
      const wrapper = mountDetails({
        bySource: [["history", 5000]],
        outputReservation: 2000
      });
      const historyRow = wrapper.find('[data-test="context-meter-row-history"]');
      expect(historyRow.text()).toContain("5.0k");
    });

    it("shows raw number for small token counts", () => {
      const wrapper = mountDetails({
        bySource: [["history", 500]],
        outputReservation: 200
      });
      const historyRow = wrapper.find('[data-test="context-meter-row-history"]');
      expect(historyRow.text()).toContain("500");
    });

    it("renders a colored swatch for each source", () => {
      const wrapper = mountDetails({
        bySource: [["history", 1000]]
      });
      const swatch = wrapper.find('[data-test="context-meter-row-history"] .swatch');
      expect(swatch.exists()).toBe(true);
      expect(swatch.attributes("style")).toContain("background");
    });

    it("renders percentage of budget for each source", () => {
      const wrapper = mountDetails({
        bySource: [["history", 5000]],
        displayBudgetTokens: 10000
      });
      const historyRow = wrapper.find('[data-test="context-meter-row-history"]');
      // 5000/10000 = 50%
      expect(historyRow.text()).toContain("50");
    });
  });

  describe("reserved row", () => {
    it("always renders the reserved-for-response row", () => {
      const wrapper = mountDetails({ outputReservation: 4000 });
      const reserved = wrapper.find('[data-test="context-meter-reserved"]');
      expect(reserved.exists()).toBe(true);
      expect(reserved.text()).toContain("4.0k");
    });

    it("renders reservation in raw number form when below 1000", () => {
      const wrapper = mountDetails({ outputReservation: 500 });
      const reserved = wrapper.find('[data-test="context-meter-reserved"]');
      expect(reserved.text()).toContain("500");
    });
  });

  describe("compact button", () => {
    it("renders the compact button", () => {
      const wrapper = mountDetails();
      expect(wrapper.find('[data-test="context-meter-compact"]').exists()).toBe(true);
    });

    it("shows 'Compact now' text in default state", () => {
      const wrapper = mountDetails({
        compacting: false,
        compressionRatioTooLow: false,
        needsAutoCompression: false
      });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      expect(btn.text()).toMatch(/compact.*now/i);
    });

    it("shows compacting-in-progress text when compacting", () => {
      const wrapper = mountDetails({ compacting: true });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      // The button text should reflect in-progress state
      expect(btn.text()).not.toMatch(/compact.*now$/i);
    });

    it("shows auto-compressing text when needsAutoCompression is true", () => {
      const wrapper = mountDetails({
        compacting: false,
        needsAutoCompression: true
      });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      expect(btn.text()).not.toMatch(/compact.*now$/i);
    });

    it("disables button when compacting is true", () => {
      const wrapper = mountDetails({ compacting: true });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("disables button when compressionRatioTooLow is true", () => {
      const wrapper = mountDetails({ compressionRatioTooLow: true });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("enables button in default state", () => {
      const wrapper = mountDetails({
        compacting: false,
        compressionRatioTooLow: false
      });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      expect((btn.element as HTMLButtonElement).disabled).toBe(false);
    });

    it("emits compact event when clicked", async () => {
      const wrapper = mountDetails();
      await wrapper.find('[data-test="context-meter-compact"]').trigger("click");
      expect(wrapper.emitted("compact")).toHaveLength(1);
    });

    it("sets appropriate title when compressionRatioTooLow", () => {
      const wrapper = mountDetails({ compressionRatioTooLow: true });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      // Should have a title explaining why it's disabled
      expect(btn.attributes("title")).toBeTruthy();
    });

    it("sets appropriate title when needsAutoCompression", () => {
      const wrapper = mountDetails({
        needsAutoCompression: true,
        compressionRatioTooLow: false
      });
      const btn = wrapper.find('[data-test="context-meter-compact"]');
      expect(btn.attributes("title")).toBeTruthy();
    });
  });

  describe("edge cases", () => {
    it("handles zero displayBudgetTokens without crashing", () => {
      const wrapper = mountDetails({
        bySource: [["history", 500]],
        displayBudgetTokens: 0
      });
      // Should render 0% instead of NaN/Infinity
      const historyRow = wrapper.find('[data-test="context-meter-row-history"]');
      expect(historyRow.exists()).toBe(true);
      expect(historyRow.text()).toContain("0");
    });

    it("renders multiple sources with correct data-test keys", () => {
      const sources: [string, number][] = [
        ["system", 1000],
        ["history", 2000],
        ["tool_result", 500],
        ["memory", 300]
      ];
      const wrapper = mountDetails({ bySource: sources });
      for (const [source] of sources) {
        expect(wrapper.find(`[data-test="context-meter-row-${source}"]`).exists()).toBe(true);
      }
    });
  });

  describe("table structure", () => {
    it("renders a table with the by-source-table class", () => {
      const wrapper = mountDetails();
      expect(wrapper.find("table.by-source-table").exists()).toBe(true);
    });

    it("renders actions container after the table", () => {
      const wrapper = mountDetails();
      expect(wrapper.find(".context-meter-actions").exists()).toBe(true);
    });
  });
});
