/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

const commandHandlers = new Map();

function registerCommandHandlers(handlers) {
  Object.keys(handlers).forEach(function (command) {
    commandHandlers.set(command, handlers[command]);
  });
}

function invoke(cmd, args) {
  args = args || {};
  var handler = commandHandlers.get(cmd);
  if (!handler) {
    console.warn("[tauri-mock] Unknown invoke: " + cmd, args);
    return Promise.resolve(undefined);
  }
  try {
    return handler(args);
  } catch (error) {
    return Promise.reject(error);
  }
}
