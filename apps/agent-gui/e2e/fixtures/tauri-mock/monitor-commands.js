/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- monitor commands ---- */

registerCommandHandlers({
  list_monitors: function () {
    return Promise.resolve(state.monitors || []);
  },
  stop_monitor: function (args) {
    var id = args.monitorId;
    if (state.monitors) {
      state.monitors = state.monitors.filter(function (m) {
        return m.monitor_id !== id;
      });
    }
    return Promise.resolve(undefined);
  }
});
