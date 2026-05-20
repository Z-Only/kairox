import { describe, it } from "vitest";

import confirmDialogSource from "./ConfirmDialog.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("ConfirmDialog", () => {
  it("uses KxButton for confirm actions instead of global btn variants", () => {
    expectSourceMigration(confirmDialogSource, {
      required: ["KxButton"],
      forbidden: ['class="btn"', "btn-danger", "btn-primary"]
    });
  });
});
