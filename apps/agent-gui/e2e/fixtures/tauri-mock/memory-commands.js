/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- memory commands ---- */

registerCommandHandlers({
  query_memories: function (args) {
    var results = state.memories.slice();
    var scope = args.scope || null;
    if (scope)
      results = results.filter(function (m) {
        return m.scope === scope;
      });
    var keywords = args.keywords || null;
    if (keywords && keywords.length > 0) {
      results = results.filter(function (m) {
        return keywords.some(function (k) {
          return m.content.toLowerCase().indexOf(k.toLowerCase()) !== -1;
        });
      });
    }
    var limit = args.limit || 50;
    return Promise.resolve(results.slice(0, limit));
  },
  delete_memory: function (args) {
    var id = args.id;
    state.memories = state.memories.filter(function (m) {
      return m.id !== id;
    });
    return Promise.resolve(undefined);
  }
});
