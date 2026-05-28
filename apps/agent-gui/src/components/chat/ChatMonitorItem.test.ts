import { describe, it, expect } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import ChatMonitorItem from "@/components/chat/ChatMonitorItem.vue";

function mount(props: InstanceType<typeof ChatMonitorItem>["$props"]) {
  return mountWithPlugins(ChatMonitorItem, { props });
}

const BASE_PROPS = {
  monitorId: "mon_1",
  description: "watching build logs",
  status: "running" as const
};

describe("ChatMonitorItem", () => {
  it("renders description and running status", () => {
    const wrapper = mount(BASE_PROPS);

    expect(wrapper.find('[data-test="chat-monitor-description"]').text()).toBe(
      "watching build logs"
    );
    expect(wrapper.find('[data-test="chat-monitor-status"]').text()).toBe("Running");
    expect(wrapper.attributes("data-status")).toBe("running");
  });

  it("renders completed status with stop reason", () => {
    const wrapper = mount({
      ...BASE_PROPS,
      status: "completed",
      stopReason: "timeout"
    });

    expect(wrapper.attributes("data-status")).toBe("completed");
    expect(wrapper.find('[data-test="chat-monitor-status"]').text()).toBe("Completed");
  });

  it("renders failed status", () => {
    const wrapper = mount({
      ...BASE_PROPS,
      status: "failed"
    });

    expect(wrapper.attributes("data-status")).toBe("failed");
    expect(wrapper.find('[data-test="chat-monitor-status"]').text()).toBe("Failed");
  });

  it("hides details by default and shows chevron when details exist", () => {
    const wrapper = mount({
      ...BASE_PROPS,
      command: "tail -f /var/log/app.log"
    });

    expect(wrapper.find('[data-test="chat-monitor-details"]').exists()).toBe(false);
    expect(wrapper.find(".chat-monitor-chevron").exists()).toBe(true);
  });

  it("hides chevron when no details exist", () => {
    const wrapper = mount(BASE_PROPS);

    expect(wrapper.find(".chat-monitor-chevron").exists()).toBe(false);
  });

  it("expands details on header click", async () => {
    const wrapper = mount({
      ...BASE_PROPS,
      command: "tail -f /var/log/app.log",
      lastLine: "ERROR: connection reset"
    });

    await wrapper.find('[data-test="chat-monitor-header"]').trigger("click");

    const details = wrapper.find('[data-test="chat-monitor-details"]');
    expect(details.exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-monitor-command"]').text()).toContain(
      "tail -f /var/log/app.log"
    );
    expect(wrapper.find('[data-test="chat-monitor-last-line"]').text()).toContain(
      "ERROR: connection reset"
    );
  });

  it("collapses details on second click", async () => {
    const wrapper = mount({
      ...BASE_PROPS,
      command: "echo test"
    });

    const header = wrapper.find('[data-test="chat-monitor-header"]');
    await header.trigger("click");
    expect(wrapper.find('[data-test="chat-monitor-details"]').exists()).toBe(true);

    await header.trigger("click");
    expect(wrapper.find('[data-test="chat-monitor-details"]').exists()).toBe(false);
  });

  it("expands on Enter keydown", async () => {
    const wrapper = mount({
      ...BASE_PROPS,
      command: "echo test"
    });

    await wrapper.find('[data-test="chat-monitor-header"]').trigger("keydown", { key: "Enter" });

    expect(wrapper.find('[data-test="chat-monitor-details"]').exists()).toBe(true);
  });

  it("expands on Space keydown", async () => {
    const wrapper = mount({
      ...BASE_PROPS,
      command: "echo test"
    });

    await wrapper.find('[data-test="chat-monitor-header"]').trigger("keydown", { key: " " });

    expect(wrapper.find('[data-test="chat-monitor-details"]').exists()).toBe(true);
  });

  it("shows stop reason in details when completed", async () => {
    const wrapper = mount({
      ...BASE_PROPS,
      status: "completed",
      stopReason: "timeout after 300s"
    });

    await wrapper.find('[data-test="chat-monitor-header"]').trigger("click");

    expect(wrapper.find('[data-test="chat-monitor-stop-reason"]').text()).toContain(
      "timeout after 300s"
    );
  });

  it("sets accessible aria attributes on header when details exist", () => {
    const wrapper = mount({
      ...BASE_PROPS,
      command: "echo test"
    });

    const header = wrapper.find('[data-test="chat-monitor-header"]');
    expect(header.attributes("role")).toBe("button");
    expect(header.attributes("tabindex")).toBe("0");
    expect(header.attributes("aria-expanded")).toBe("false");
  });

  it("omits interactive aria attributes when no details exist", () => {
    const wrapper = mount(BASE_PROPS);

    const header = wrapper.find('[data-test="chat-monitor-header"]');
    expect(header.attributes("role")).toBeUndefined();
    expect(header.attributes("tabindex")).toBeUndefined();
    expect(header.attributes("aria-expanded")).toBeUndefined();
  });
});
