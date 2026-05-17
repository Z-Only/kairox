/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- instructions commands ---- */

var _savedInstructions = {
  user: null,
  project: null
};

registerCommandHandlers({
  get_instructions: function (args) {
    var scope = args.scope;
    return Promise.resolve({
      system: "You are Kairox, an AI coding assistant.\n\nFollow the Memory Protocol.",
      user: _savedInstructions.user,
      project: _savedInstructions.project
    });
  },
  upsert_instructions: function (args) {
    var input = args.input;
    var scope = input.scope;
    if (scope === "User") {
      _savedInstructions.user = input.text || null;
    } else if (scope === "Project") {
      _savedInstructions.project = input.text || null;
    }
    return Promise.resolve(null);
  },
  get_system_prompt: function (args) {
    return Promise.resolve(
      "You are Kairox, an AI coding assistant.\n\nFollow the Memory Protocol."
    );
  }
});
