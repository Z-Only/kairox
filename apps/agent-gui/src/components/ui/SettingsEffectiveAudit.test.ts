import { describe, expect, it } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import SettingsEffectiveAudit from "./SettingsEffectiveAudit.vue";

describe("SettingsEffectiveAudit", () => {
  it("renders source, effective, override, disabled, and validity state with stable selectors", () => {
    const wrapper = mountWithPlugins(SettingsEffectiveAudit, {
      props: {
        source: "Project",
        sourceTone: "source-project",
        enabled: false,
        effective: false,
        shadowedBy: "User",
        overrides: "Builtin",
        disabledBy: "Project",
        valid: false,
        dataTest: "settings-audit-row"
      }
    });

    expect(wrapper.attributes("data-test")).toBe("settings-audit-row");
    expect(wrapper.text()).toContain("Source");
    expect(wrapper.text()).toContain("Project");
    expect(wrapper.text()).toContain("State");
    expect(wrapper.text()).toContain("Disabled");
    expect(wrapper.text()).toContain("Effective");
    expect(wrapper.text()).toContain("Shadowed by User");
    expect(wrapper.text()).toContain("Overrides");
    expect(wrapper.text()).toContain("Builtin");
    expect(wrapper.text()).toContain("Disabled by");
    expect(wrapper.text()).toContain("Validity");
    expect(wrapper.text()).toContain("Invalid");
    expect(wrapper.find('[data-test="settings-audit-row-source"]').classes()).toContain(
      "settings-status-tag--source-project"
    );
    expect(wrapper.find('[data-test="settings-audit-row-disabled-by"]').classes()).toContain(
      "settings-status-tag--disabled-by"
    );
  });

  it("omits unset optional state without leaving empty labels", () => {
    const wrapper = mountWithPlugins(SettingsEffectiveAudit, {
      props: {
        source: "User",
        enabled: true,
        effective: true,
        dataTest: "settings-audit-minimal"
      }
    });

    expect(wrapper.text()).toContain("User");
    expect(wrapper.text()).toContain("Enabled");
    expect(wrapper.text()).toContain("Active");
    expect(wrapper.text()).not.toContain("Overrides");
    expect(wrapper.text()).not.toContain("Disabled by");
    expect(wrapper.text()).not.toContain("Validity");
  });
});
