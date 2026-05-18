/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- profile commands ---- */

registerCommandHandlers({
  list_profiles: function (args) {
    return Promise.resolve(
      state.profiles.map(function (p) {
        return p.alias;
      })
    );
  },
  list_profiles_with_limits: function (args) {
    return Promise.resolve(
      state.profiles.map(function (p) {
        var window;
        var output;
        if (p.alias === "fast") {
          window = 128000;
          output = 16384;
        } else if (p.alias === "smart") {
          window = 200000;
          output = 16384;
        } else {
          window = 4096;
          output = 2048;
        }
        return {
          alias: p.alias,
          provider: p.provider,
          model_id: p.model_id,
          context_window: window,
          output_limit: output,
          limit_source: "builtin_registry",
          has_api_key: p.has_api_key,
          supports_reasoning: p.supports_reasoning === true
        };
      })
    );
  },
  get_profile_info: function (args) {
    return Promise.resolve(state.profiles);
  },
  get_profile_detail: function (args) {
    var profile = args.profile || "fast";
    var found = state.profiles.find(function (p) {
      return p.alias === profile;
    });
    if (!found) return Promise.reject(new Error("Profile '" + profile + "' not found"));
    return Promise.resolve(found);
  },
  list_profile_settings: function (args) {
    return state.profiles.map(function (p) {
      return {
        alias: p.alias,
        provider: p.provider,
        model_id: p.model_id,
        enabled: p.enabled !== false,
        context_window: p.context_window ?? null,
        output_limit: p.output_limit ?? null,
        temperature: p.temperature ?? null,
        top_p: p.top_p ?? null,
        top_k: p.top_k ?? null,
        max_tokens: p.max_tokens ?? null,
        base_url: p.base_url ?? null,
        api_key_env: p.api_key_env ?? null,
        has_api_key: p.has_api_key !== false,
        writable: p.writable !== false,
        config_path: p.config_path ?? null,
        source: p.source ?? "profiles_toml"
      };
    });
  },
  upsert_profile_settings: function (args) {
    var upsertInput = args && (args.input || args);
    var existing = state.profiles.find(function (p) {
      return p.alias === upsertInput.alias;
    });
    if (existing) {
      Object.assign(existing, upsertInput);
    } else {
      state.profiles.push(Object.assign({ writable: true, source: "profiles_toml" }, upsertInput));
    }
    return state.profiles.find(function (p) {
      return p.alias === upsertInput.alias;
    });
  },
  set_profile_enabled: function (args) {
    var target = state.profiles.find(function (p) {
      return p.alias === args.alias;
    });
    if (target) target.enabled = args.enabled;
    return null;
  },
  delete_profile_settings: function (args) {
    state.profiles = state.profiles.filter(function (p) {
      return p.alias !== args.alias;
    });
    return null;
  },
  move_profile_in_order: function (args) {
    return null;
  }
});
