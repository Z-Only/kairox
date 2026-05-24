import { defineComponent } from "vue";
import { describe, expect, it } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import modelProfileFormDialogSource from "./ModelProfileFormDialog.vue?raw";
import modelParameterControlsSource from "./ModelParameterControls.vue?raw";
import ModelProfileFormDialog from "./ModelProfileFormDialog.vue";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("ModelProfileFormDialog", () => {
  const Host = defineComponent({
    components: { ModelProfileFormDialog },
    data() {
      return {
        open: true,
        mode: "add" as "add" | "edit",
        loading: false,
        canTest: false,
        alias: "",
        provider: "",
        modelId: "",
        contextWindow: "",
        outputLimit: "",
        temperature: "",
        topP: "",
        topK: "",
        maxTokens: "",
        baseUrl: "",
        apiKeyEnv: "",
        advancedOpen: false
      };
    },
    template: `
      <ModelProfileFormDialog
        :open="open"
        :mode="mode"
        :loading="loading"
        :can-test="canTest"
        v-model:alias="alias"
        v-model:provider="provider"
        v-model:model-id="modelId"
        v-model:context-window="contextWindow"
        v-model:output-limit="outputLimit"
        v-model:temperature="temperature"
        v-model:top-p="topP"
        v-model:top-k="topK"
        v-model:max-tokens="maxTokens"
        v-model:base-url="baseUrl"
        v-model:api-key-env="apiKeyEnv"
        v-model:advanced-open="advancedOpen"
      />
    `
  });

  function mountDialog(initialData: Partial<ReturnType<(typeof Host)["data"]>> = {}) {
    return mountWithPlugins(Host, {
      mount: {
        data: () => initialData,
        global: {
          stubs: {
            ModalDialog: {
              props: ["open"],
              template: `
                <section v-if="open" data-test="modal-dialog-stub">
                  <slot />
                  <footer>
                    <slot name="footer" />
                  </footer>
                </section>
              `
            },
            ModelParameterControls: {
              template: '<section data-test="model-parameter-controls-stub" />'
            }
          }
        }
      }
    }).wrapper;
  }

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

  it("disables add-mode save until alias, provider, and model id are present", async () => {
    const wrapper = mountDialog();
    const saveButton = () => wrapper.get<HTMLButtonElement>('[data-test="model-save-button"]');

    expect(saveButton().element.disabled).toBe(true);

    await wrapper.get('[data-test="model-form-alias"]').setValue("local");
    expect(saveButton().element.disabled).toBe(true);

    await wrapper.get('[data-test="model-form-provider"]').setValue("ollama");
    expect(saveButton().element.disabled).toBe(true);

    await wrapper.get('[data-test="model-form-model-id"]').setValue("llama3");
    expect(saveButton().element.disabled).toBe(false);
  });

  it("keeps edit-mode alias readonly", () => {
    const wrapper = mountDialog({
      mode: "edit",
      alias: "default",
      provider: "openai",
      modelId: "gpt-4.1"
    });

    const aliasInput = wrapper.get<HTMLInputElement>('[data-test="model-edit-alias"]');

    expect(aliasInput.element.value).toBe("default");
    expect(aliasInput.element.readOnly).toBe(true);
  });

  it("enables add-mode connectivity testing only when a base URL is present", async () => {
    const wrapper = mountDialog();
    const testButton = () => wrapper.get<HTMLButtonElement>('[data-test="model-test-form-btn"]');

    expect(testButton().element.disabled).toBe(true);

    await wrapper.get('[data-test="model-form-base-url"]').setValue("   ");
    expect(testButton().element.disabled).toBe(true);

    await wrapper.get('[data-test="model-form-base-url"]').setValue("http://localhost:11434");
    expect(testButton().element.disabled).toBe(false);
  });

  it("uses canTest to enable edit-mode connectivity testing", async () => {
    const wrapper = mountDialog({
      mode: "edit",
      alias: "default",
      provider: "openai",
      modelId: "gpt-4.1",
      baseUrl: "https://example.test"
    });
    const testButton = () => wrapper.get<HTMLButtonElement>('[data-test="model-edit-test-btn"]');

    expect(testButton().element.disabled).toBe(true);

    await wrapper.setData({ canTest: true });
    expect(testButton().element.disabled).toBe(false);

    await wrapper.setData({ canTest: false });
    expect(testButton().element.disabled).toBe(true);
  });
});
