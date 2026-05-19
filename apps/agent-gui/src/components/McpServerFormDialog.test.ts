import { describe, expect, it } from "vitest";
import mcpServerFormDialogSource from "./McpServerFormDialog.vue?raw";

describe("McpServerFormDialog", () => {
  it("uses shared form fields and controls instead of local input chrome", () => {
    expect(mcpServerFormDialogSource).toContain("KxFormField");
    expect(mcpServerFormDialogSource).toContain("KxInput");
    expect(mcpServerFormDialogSource).not.toContain("kx-form-control");
    expect(mcpServerFormDialogSource).not.toContain(".form label + input");
    expect(mcpServerFormDialogSource).not.toContain(".form label + input:focus");
  });
});
