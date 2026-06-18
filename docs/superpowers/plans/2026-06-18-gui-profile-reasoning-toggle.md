# GUI Profile Reasoning Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users explicitly set `supports_reasoning = true/false` from the GUI model profile settings form, with the value persisted through the Rust profile settings contract.

**Architecture:** Add `supports_reasoning: Option<bool>` to the profile settings input/view DTOs and the runtime row/write/list paths. Then surface the option as a checkbox in `ModelProfileFormDialog.vue`, preserving `null` on new profiles and using the existing profile value when editing. Generated Tauri/Specta TypeScript bindings are refreshed with `just gen-types`; generated files are not edited by hand.

**Tech Stack:** Rust (`agent-core`, `agent-runtime`, Tauri command tests), Vue 3 + Pinia + Vitest (`apps/agent-gui`), Specta generated bindings.

---

## File Structure

- Modify `crates/agent-core/src/facade/settings.rs`: add `supports_reasoning: Option<bool>` to `ProfileSettingsInput` and `ProfileSettingsView`.
- Modify `crates/agent-runtime/src/profile_settings/row.rs`: parse `supports_reasoning` from TOML into `ProfileSettingsRow`.
- Modify `crates/agent-runtime/src/profile_settings/view.rs`: populate the new view field from defaults, profiles TOML, user config, and project config.
- Modify `crates/agent-runtime/src/profile_settings/write.rs`: seed, write, remove, and return `supports_reasoning`.
- Modify Rust tests that instantiate `ProfileSettingsInput` / `ProfileSettingsView`.
- Modify `apps/agent-gui/src/components/ModelProfileFormDialog.vue`: add a checkbox model and form control.
- Modify `apps/agent-gui/src/components/ModelSettingsPane.vue`: hold/reset/prefill/build the `supports_reasoning` value.
- Modify `apps/agent-gui/src/components/ModelSettingsPane.test.ts`: assert add/edit form behavior.
- Modify `crates/agent-tui/src/components/types.rs`, `crates/agent-tui/src/app/commands/models.rs`, `crates/agent-tui/src/components/model_overlay/types.rs`, and related tests: preserve existing explicit reasoning overrides when the TUI saves profile settings, without adding new TUI controls in this PR.
- Generate `apps/agent-gui/src/generated/commands.ts` and `apps/agent-gui/src/generated/events.ts` using `just gen-types`.

## Task 1: Rust Contract And Persistence

**Files:**

- Modify: `crates/agent-core/src/facade/settings.rs`
- Modify: `crates/agent-runtime/src/profile_settings/row.rs`
- Modify: `crates/agent-runtime/src/profile_settings/view.rs`
- Modify: `crates/agent-runtime/src/profile_settings/write.rs`
- Test: `crates/agent-runtime/src/profile_settings/mod_tests.rs`
- Test: `crates/agent-core/src/facade/mcp_tests.rs`
- Test: `apps/agent-gui/src-tauri/src/commands/settings/profiles.rs`

- [ ] **Step 1: Write the failing Rust test**

Add `supports_reasoning: Some(true),` to the `ProfileSettingsInput` in `upsert_writes_profile_settings`, then assert both returned view and TOML:

```rust
assert_eq!(view.supports_reasoning, Some(true));
assert!(raw.contains("supports_reasoning = true"));
```

Add a focused list test in `crates/agent-runtime/src/profile_settings/mod_tests.rs`:

```rust
#[tokio::test]
async fn list_profile_settings_exposes_supports_reasoning_override() {
    let config_path = write_profiles_config_fixture(
        "[profiles.reasoning]\nprovider = \"anthropic\"\nmodel_id = \"claude-opus-4-6\"\nsupports_reasoning = true\n",
    );
    let views = list_profile_settings(
        &agent_config::Config::defaults(),
        Some(&config_path),
        None,
        None,
        None,
    )
    .await
    .expect("profile settings should list");
    let profile = views
        .iter()
        .find(|profile| profile.alias == "reasoning")
        .expect("profile should be visible");
    assert_eq!(profile.supports_reasoning, Some(true));
}
```

- [ ] **Step 2: Run Rust RED**

Run:

```bash
cargo test -p agent-runtime profile_settings -- --nocapture
```

Expected: compile failure or test failure because `ProfileSettingsInput` and `ProfileSettingsView` do not yet expose `supports_reasoning`.

- [ ] **Step 3: Implement minimal Rust support**

Add this field to both DTO structs in `crates/agent-core/src/facade/settings.rs`:

```rust
pub supports_reasoning: Option<bool>,
```

Add this field to `ProfileSettingsRow` and parse it in `profile_row_from_toml_table`:

```rust
pub(crate) supports_reasoning: Option<bool>,
```

```rust
supports_reasoning: table
    .and_then(|t| t.get("supports_reasoning"))
    .and_then(Item::as_bool),
```

Propagate it into every `ProfileSettingsRow` literal in `view.rs`:

```rust
supports_reasoning: def.supports_reasoning,
```

and into every `ProfileSettingsView` literal:

```rust
supports_reasoning: row.supports_reasoning,
```

In `write.rs`, seed and write the option:

```rust
if let Some(v) = def.supports_reasoning {
    table["supports_reasoning"] = value(v);
}
```

```rust
set_optional_bool(profile_table, "supports_reasoning", input.supports_reasoning);
```

```rust
fn set_optional_bool(table: &mut Table, key: &str, val: Option<bool>) {
    match val {
        Some(v) => table[key] = value(v),
        None => {
            table.remove(key);
        }
    }
}
```

Add the new field to all local test literals with `None` unless the test is explicitly checking the new behavior.

- [ ] **Step 4: Run Rust GREEN**

Run:

```bash
cargo test -p agent-runtime profile_settings -- --nocapture
cargo test -p agent-core facade::mcp_tests::upsert_profile_settings_returns_error -- --exact
cargo test -p agent-gui-tauri commands::settings::profiles -- --nocapture
```

Expected: non-zero test counts and all pass.

## Task 2: GUI Form Behavior

**Files:**

- Modify: `apps/agent-gui/src/components/ModelProfileFormDialog.vue`
- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue`
- Test: `apps/agent-gui/src/components/ModelSettingsPane.test.ts`

- [ ] **Step 1: Write the failing GUI tests**

Add `supports_reasoning: null,` to the profile fixtures, then add tests:

```ts
it("saves explicit reasoning support from the add dialog", async () => {
  const wrapper = mountPane("user");
  await flushPromises();

  await wrapper.find('[data-test="model-add-profile"]').trigger("click");
  await flushPromises();
  await wrapper.find('[data-test="model-form-alias"]').setValue("reasoning-model");
  await wrapper.find('[data-test="model-form-provider"]').setValue("anthropic");
  await wrapper.find('[data-test="model-form-model-id"]').setValue("claude-opus-4-6");
  await wrapper.find('[data-test="model-form-supports-reasoning"]').setValue(true);
  await wrapper.find('[data-test="model-save-button"]').trigger("click");
  await flushPromises();

  expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(
    expect.objectContaining({
      alias: "reasoning-model",
      supports_reasoning: true
    })
  );
});
```

```ts
it("prefills and preserves explicit reasoning support when editing", async () => {
  mockedCommands.listProfileSettings.mockResolvedValueOnce(
    ok([{ ...writableProfile, supports_reasoning: true }])
  );
  const wrapper = mountPane("user");
  await flushPromises();

  await wrapper.find('[data-test="model-edit-my-model"]').trigger("click");
  await flushPromises();

  const checkbox = wrapper.find('[data-test="model-edit-supports-reasoning"]');
  expect((checkbox.element as HTMLInputElement).checked).toBe(true);
  await wrapper.find('[data-test="model-edit-save-button"]').trigger("click");
  await flushPromises();

  expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(
    expect.objectContaining({ supports_reasoning: true })
  );
});
```

- [ ] **Step 2: Run GUI RED**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test src/components/ModelSettingsPane.test.ts -- --runInBand
```

Expected: the new tests fail because the checkbox selector and `supports_reasoning` payload do not exist yet.

- [ ] **Step 3: Implement minimal GUI support**

In `ModelProfileFormDialog.vue`, add:

```ts
const supportsReasoning = defineModel<boolean>("supportsReasoning", { required: true });
```

Add a checkbox beside the existing Claude Code identity checkbox:

```vue
<label class="model-form__checkbox">
  <input
    v-model="supportsReasoning"
    type="checkbox"
    :data-test="
      isAddMode ? 'model-form-supports-reasoning' : 'model-edit-supports-reasoning'
    "
  />
  <span>{{ t("models.supportsReasoning") }}</span>
</label>
```

In `ModelSettingsPane.vue`, add state:

```ts
const formSupportsReasoning = ref(false);
const formSupportsReasoningExplicit = ref(false);
```

Reset it to `false`, prefill it in edit mode:

```ts
formSupportsReasoning.value = profile.supports_reasoning === true;
formSupportsReasoningExplicit.value = profile.supports_reasoning !== null;
```

and include it in `buildProfileInput`:

```ts
supports_reasoning: formSupportsReasoningExplicit.value ? formSupportsReasoning.value : null,
```

Bind both dialogs with:

```vue
v-model:supports-reasoning="formSupportsReasoning"
```

When the checkbox changes, set `formSupportsReasoningExplicit.value = true` via a small handler passed from the dialog emit or a `watch(formSupportsReasoning, ...)` guarded by dialog open state.

Add `models.supportsReasoning` to locale messages if missing.

- [ ] **Step 4: Run GUI GREEN**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test src/components/ModelSettingsPane.test.ts -- --runInBand
```

Expected: non-zero test count and all pass.

## Task 3: Generated Types, Gates, Dev App, PR

**Files:**

- Generate: `apps/agent-gui/src/generated/commands.ts`
- Generate: `apps/agent-gui/src/generated/events.ts`
- Modify if needed: generated type source registration in `apps/agent-gui/src-tauri/src/specta.rs` only if the generator fails because the type is not registered.

- [ ] **Step 1: Generate bindings**

Run:

```bash
just gen-types
```

Expected: generated TypeScript includes `supports_reasoning: boolean | null` on `ProfileSettingsInput` and `ProfileSettingsView`.

- [ ] **Step 2: Format and focused gates**

Run:

```bash
cargo fmt --all
cargo fmt --all --check
bun run format:check
bun run lint:no-inline-tests
cargo clippy -p agent-core --all-targets -- -D warnings
cargo clippy -p agent-runtime --all-targets -- -D warnings
cargo clippy -p agent-gui-tauri --all-targets -- -D warnings
cargo test -p agent-core
cargo test -p agent-runtime
cargo test -p agent-gui-tauri
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test src/components/ModelSettingsPane.test.ts -- --runInBand
```

Expected: all commands exit 0; tests report non-zero counts.

- [ ] **Step 3: Dev App verification**

Run the GUI in pilot mode from this worktree:

```bash
bun --filter agent-gui tauri dev --features pilot
```

Verify in the local app:

1. Open Settings -> Models.
2. Add a profile and enable the reasoning support checkbox.
3. Save; reopen the profile; verify the checkbox remains checked.
4. Disable the checkbox on edit; save; reopen; verify unchecked.
5. Confirm the app remains responsive and no console/runtime error appears.

- [ ] **Step 4: Commit and PR**

Run:

```bash
git status --short
git add crates/agent-core/src/facade/settings.rs crates/agent-runtime/src/profile_settings/row.rs crates/agent-runtime/src/profile_settings/view.rs crates/agent-runtime/src/profile_settings/write.rs crates/agent-runtime/src/profile_settings/mod_tests.rs crates/agent-core/src/facade/mcp_tests.rs apps/agent-gui/src-tauri/src/commands/settings/profiles.rs apps/agent-gui/src/components/ModelProfileFormDialog.vue apps/agent-gui/src/components/ModelSettingsPane.vue apps/agent-gui/src/components/ModelSettingsPane.test.ts apps/agent-gui/src/generated/commands.ts apps/agent-gui/src/generated/events.ts docs/superpowers/plans/2026-06-18-gui-profile-reasoning-toggle.md
git commit -m "feat(gui): expose profile reasoning toggle"
git fetch origin main
git rebase origin/main
git push -u origin feat/gui-profile-reasoning-toggle
gh pr create --base main --head feat/gui-profile-reasoning-toggle --title "feat(gui): expose profile reasoning toggle" --body-file /tmp/kairox-profile-reasoning-toggle-pr.md
gh pr merge <pr-number> --auto --squash --delete-branch
```

Expected: PR includes test evidence and Dev App verification evidence, auto-merge is enabled, and watcher follows it to merge before cleanup.

## Self-Review

- Spec coverage: Rust DTO, persistence, listing, Tauri generated types, Vue form, tests, Dev App verification, PR lifecycle are covered.
- Placeholder scan: no TBD/TODO/fill-in instructions remain.
- Type consistency: the field name is `supports_reasoning` in Rust/TOML/generated TypeScript payloads and `supportsReasoning` only for Vue model binding.
