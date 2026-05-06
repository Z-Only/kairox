import type { RouteRecordRaw } from "vue-router";

// TODO(Task 5): swap these placeholder components for real lazy imports of
// `@/views/WorkbenchView.vue`, `@/views/MarketplaceView.vue`, and
// `@/views/SettingsView.vue` once those SFCs exist. Vite's build cannot
// statically resolve dynamic imports to non-existent modules, so we ship
// runtime stubs in Task 2 to keep the build green.
const placeholderComponent = (name: string) => () =>
  Promise.resolve({
    default: { name, template: `<div>view-placeholder:${name}</div>` }
  });

export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: placeholderComponent("WorkbenchView"),
    props: true
  },
  {
    path: "/marketplace",
    name: "marketplace",
    component: placeholderComponent("MarketplaceView")
  },
  {
    path: "/settings",
    name: "settings",
    component: placeholderComponent("SettingsView")
  },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
