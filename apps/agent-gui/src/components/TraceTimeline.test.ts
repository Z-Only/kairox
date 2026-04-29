import { describe, expect, it } from "vitest";
import { traceLabels } from "./TraceTimeline";

describe("traceLabels", () => {
  it("renders event types in order", () => {
    expect([
      { event_type: "UserMessageAdded" },
      { event_type: "AssistantMessageCompleted" },
    ]).toEqual([
      { event_type: "UserMessageAdded" },
      { event_type: "AssistantMessageCompleted" },
    ]);

    expect(
      traceLabels([
        { event_type: "UserMessageAdded" },
        { event_type: "AssistantMessageCompleted" },
      ])
    ).toEqual(["UserMessageAdded", "AssistantMessageCompleted"]);
  });
});
