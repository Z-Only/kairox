import { describe, it, expect } from "vitest";
import { mountWithPlugins } from "./mount";

describe("test-utils/mount", () => {
  it("exports mountWithPlugins as a function", () => {
    expect(typeof mountWithPlugins).toBe("function");
  });
});
