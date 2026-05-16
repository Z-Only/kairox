/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- event commands ---- */

registerCommandHandlers({
  "plugin:event|listen": function (args) {
    var eventName = args.event;
    var handlerId = args.handler;
    if (!state.eventListeners.has(eventName)) {
      state.eventListeners.set(eventName, new Map());
    }
    // Store the wrapped handler that will be called when we emitEvent
    state.eventListeners.get(eventName).set(handlerId, function (evt) {
      // Invoke the callback that was registered via transformCallback
      invokeCallback(handlerId, [evt]);
    });
    // Return the handlerId as the eventId (this is what Tauri v2 returns)
    return Promise.resolve(handlerId);
  },
  "plugin:event|unlisten": function (args) {
    var eventName = args.event;
    var eventId = args.eventId;
    var listeners = state.eventListeners.get(eventName);
    if (listeners) {
      listeners.delete(eventId);
    }
    return Promise.resolve(undefined);
  }
});
