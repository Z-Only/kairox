/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- general settings + plugin commands ---- */

var _guiSettings = {
  devtools_enabled: false,
  default_devtools_enabled: false,
  requires_restart: false
};

registerCommandHandlers({
  get_gui_settings: function (args) {
    return Promise.resolve(clone(_guiSettings));
  },
  set_gui_devtools_enabled: function (args) {
    var enabled = args.enabled;
    var changed = _guiSettings.devtools_enabled !== enabled;
    _guiSettings.devtools_enabled = enabled;
    _guiSettings.requires_restart = changed;
    return Promise.resolve(clone(_guiSettings));
  },
  "plugin:app|version": function (args) {
    return Promise.resolve("0.37.0");
  },
  "plugin:updater|check": function (args) {
    // Return null = no update available (default safe behaviour)
    return Promise.resolve(null);
  },
  "plugin:process|restart": function (args) {
    return Promise.resolve(undefined);
  }
});
