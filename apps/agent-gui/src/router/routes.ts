export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: () => import("@/views/WorkbenchView.vue"),
    props: true
  },
  {
    path: "/marketplace",
    name: "marketplace",
    component: () => import("@/views/MarketplaceView.vue")
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("@/views/SettingsView.vue")
  },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
