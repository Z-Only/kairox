import { describe, it } from "vitest";
import mcpServerFormDialogSource from "./McpServerFormDialog.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("McpServerFormDialog", () => {
  it("uses shared form fields and controls instead of local input chrome", () => {
    expectSourceMigration(mcpServerFormDialogSource, {
      required: ["KxFormField", "KxInput"],
      forbidden: ["kx-form-control", ".form label + input", ".form label + input:focus"]
    });
  });
});
