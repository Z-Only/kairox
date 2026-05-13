import SettingsLayout from "@/layouts/SettingsLayout.vue";

export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: () => import("@/views/WorkbenchView.vue"),
    props: true
  },
  // Legacy redirect for old /marketplace URL
  { path: "/marketplace", redirect: "/settings" },
  {
    path: "/settings",
    component: SettingsLayout,
    children: [
      { path: "", redirect: { name: "settings-general" } },
      {
        path: "general",
        name: "settings-general",
        component: () => import("@/views/settings/GeneralSettings.vue")
      },
      {
        path: "mcp",
        name: "settings-mcp",
        component: () => import("@/components/McpSettingsPane.vue")
      },
      {
        path: "skills",
        name: "settings-skills",
        component: () => import("@/components/SkillSettingsPane.vue")
      },
      {
        path: "models",
        name: "settings-models",
        component: () => import("@/components/ModelSettingsPane.vue")
      },
      {
        path: "archive",
        name: "settings-archive",
        component: () => import("@/components/ArchiveSettingsPane.vue")
      }
    ]
  },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
