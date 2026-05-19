import { describe, expect, it } from "vitest";

import confirmDialogSource from "./ConfirmDialog.vue?raw";

describe("ConfirmDialog", () => {
  it("uses KxButton for confirm actions instead of global btn variants", () => {
    expect(confirmDialogSource).toContain("KxButton");
    expect(confirmDialogSource).not.toContain('class="btn"');
    expect(confirmDialogSource).not.toContain("btn-danger");
    expect(confirmDialogSource).not.toContain("btn-primary");
  });
});
