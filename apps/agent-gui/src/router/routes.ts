export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: () => import("@/views/WorkbenchView.vue"),
    props: true
  },
  // Legacy redirect for old /marketplace URL
  { path: "/marketplace", redirect: { name: "settings-marketplace" } },
  {
    path: "/settings",
    name: "settings",
    component: () => import("@/views/SettingsView.vue"),
    children: [
      {
        path: "marketplace",
        name: "settings-marketplace",
        component: () => import("@/views/MarketplaceView.vue")
      }
    ]
  },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
