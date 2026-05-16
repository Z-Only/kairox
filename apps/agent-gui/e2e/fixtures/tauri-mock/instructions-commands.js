/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- instructions commands ---- */

registerCommandHandlers({
  get_instructions: function (args) {
    return Promise.resolve({
      system: "You are Kairox, an AI coding assistant.\n\nFollow the Memory Protocol.",
      user: null,
      project: null
    });
  },
  upsert_instructions: function (args) {
    return Promise.resolve(null);
  },
  get_system_prompt: function (args) {
    return Promise.resolve(
      "You are Kairox, an AI coding assistant.\n\nFollow the Memory Protocol."
    );
  }
});
