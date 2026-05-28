import { describe, it, expect } from "vitest";
import { formatError, isCommandResult, unwrapCommandResult } from "./utils";
import type { CommandResult } from "./utils";

describe("mcp store utils", () => {
  describe("formatError", () => {
    it("returns message from Error instance", () => {
      expect(formatError(new Error("something broke"))).toBe("something broke");
    });

    it("converts string to string", () => {
      expect(formatError("raw string")).toBe("raw string");
    });

    it("converts number to string", () => {
      expect(formatError(42)).toBe("42");
    });

    it("converts null to string", () => {
      expect(formatError(null)).toBe("null");
    });

    it("converts undefined to string", () => {
      expect(formatError(undefined)).toBe("undefined");
    });

    it("converts object to string", () => {
      expect(formatError({ key: "value" })).toBe("[object Object]");
    });
  });

  describe("isCommandResult", () => {
    it("returns true for ok CommandResult", () => {
      const result: CommandResult<string> = { status: "ok", data: "hello" };
      expect(isCommandResult(result)).toBe(true);
    });

    it("returns true for error CommandResult", () => {
      const result: CommandResult<string> = { status: "error", error: "fail" };
      expect(isCommandResult(result)).toBe(true);
    });

    it("returns false for plain string", () => {
      expect(isCommandResult("hello")).toBe(false);
    });

    it("returns false for number", () => {
      expect(isCommandResult(123)).toBe(false);
    });

    it("returns false for null", () => {
      expect(isCommandResult(null)).toBe(false);
    });

    it("returns false for object without status field", () => {
      expect(isCommandResult({ data: "something" })).toBe(false);
    });

    it("returns false for object with invalid status value", () => {
      expect(isCommandResult({ status: "pending", data: "x" })).toBe(false);
    });

    it("returns false for array", () => {
      expect(isCommandResult([1, 2, 3])).toBe(false);
    });
  });

  describe("unwrapCommandResult", () => {
    it("returns data from ok CommandResult", async () => {
      const promise = Promise.resolve<CommandResult<string>>({ status: "ok", data: "payload" });
      expect(await unwrapCommandResult(promise)).toBe("payload");
    });

    it("throws on error CommandResult", async () => {
      const promise = Promise.resolve<CommandResult<string>>({
        status: "error",
        error: "went wrong"
      });
      await expect(unwrapCommandResult(promise)).rejects.toThrow("went wrong");
    });

    it("passes through plain value when not a CommandResult", async () => {
      const promise = Promise.resolve("direct value");
      expect(await unwrapCommandResult(promise)).toBe("direct value");
    });

    it("passes through plain object without status field", async () => {
      const obj = { name: "test" };
      const promise = Promise.resolve(obj);
      expect(await unwrapCommandResult(promise)).toEqual(obj);
    });

    it("passes through null value", async () => {
      const promise = Promise.resolve(null);
      expect(await unwrapCommandResult(promise)).toBeNull();
    });
  });
});
