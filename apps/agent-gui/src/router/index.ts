// `unplugin-auto-import` only auto-injects globals into `.vue` SFCs; plain
// `.ts` modules under `src/` are not transformed (we keep `dirs: []` per
// spec §3 Q7 to avoid hoovering up business stores). The router is
// infrastructure, so we import explicitly — this also keeps the bundle
// independent of plugin ordering, which previously surfaced as a runtime
// `Uncaught ReferenceError: createRouter is not defined` in the browser.
import { createRouter, createWebHashHistory } from "vue-router";
import { routes } from "./routes";

export const router = createRouter({
  history: createWebHashHistory(),
  routes
});
