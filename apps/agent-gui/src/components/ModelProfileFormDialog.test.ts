import { describe, it } from "vitest";
import modelProfileFormDialogSource from "./ModelProfileFormDialog.vue?raw";
import modelParameterControlsSource from "./ModelParameterControls.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("ModelProfileFormDialog", () => {
  it("uses shared form fields and controls for profile inputs", () => {
    expectSourceMigration(modelProfileFormDialogSource, {
      required: ["KxFormField", "KxInput"],
      forbidden: ["kx-form-control", ".model-form input {", ".model-form input:focus"]
    });
  });

  it("uses the same shared controls for advanced numeric parameters", () => {
    expectSourceMigration(modelParameterControlsSource, {
      required: ["KxFormField", "KxInput"],
      forbidden: ["kx-form-control", "input {", "input:focus"]
    });
  });
});
