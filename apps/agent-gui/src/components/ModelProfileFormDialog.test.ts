import { describe, expect, it } from "vitest";
import modelProfileFormDialogSource from "./ModelProfileFormDialog.vue?raw";
import modelParameterControlsSource from "./ModelParameterControls.vue?raw";

describe("ModelProfileFormDialog", () => {
  it("uses shared form fields and controls for profile inputs", () => {
    expect(modelProfileFormDialogSource).toContain("KxFormField");
    expect(modelProfileFormDialogSource).toContain("kx-form-control");
    expect(modelProfileFormDialogSource).not.toContain(".model-form input {");
    expect(modelProfileFormDialogSource).not.toContain(".model-form input:focus");
  });

  it("uses the same shared controls for advanced numeric parameters", () => {
    expect(modelParameterControlsSource).toContain("KxFormField");
    expect(modelParameterControlsSource).toContain("kx-form-control");
    expect(modelParameterControlsSource).not.toContain("input {");
    expect(modelParameterControlsSource).not.toContain("input:focus");
  });
});
