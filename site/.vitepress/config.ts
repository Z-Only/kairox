import { defineConfig, type DefaultTheme } from "vitepress";

const siteUrl = "https://z-only.github.io/kairox/";
const repoUrl = "https://github.com/Z-Only/kairox";

function enNav(): DefaultTheme.NavItem[] {
  return [
    { text: "Home", link: "/" },
    { text: "Get Started", link: "/guide/getting-started" },
    { text: "Architecture", link: "/guide/architecture" },
    { text: "GitHub", link: repoUrl }
  ];
}

function zhNav(): DefaultTheme.NavItem[] {
  return [
    { text: "首页", link: "/zh/" },
    { text: "快速开始", link: "/zh/guide/getting-started" },
    { text: "架构", link: "/zh/guide/architecture" },
    { text: "GitHub", link: repoUrl }
  ];
}

function enSidebar(): DefaultTheme.SidebarItem[] {
  return [
    {
      text: "Guide",
      items: [
        { text: "Getting Started", link: "/guide/getting-started" },
        { text: "Architecture", link: "/guide/architecture" }
      ]
    }
  ];
}

function zhSidebar(): DefaultTheme.SidebarItem[] {
  return [
    {
      text: "指南",
      items: [
        { text: "快速开始", link: "/zh/guide/getting-started" },
        { text: "架构", link: "/zh/guide/architecture" }
      ]
    }
  ];
}

export default defineConfig({
  title: "Kairox",
  description:
    "Local-first AI agent workbench with a shared Rust core, TUI, and Tauri desktop GUI.",
  base: "/kairox/",
  cleanUrls: true,
  lastUpdated: true,
  sitemap: {
    hostname: siteUrl
  },
  head: [
    ["link", { rel: "icon", href: "/kairox/logo.svg", type: "image/svg+xml" }],
    ["meta", { name: "theme-color", content: "#0f172a" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:image", content: `${siteUrl}banner.svg` }],
    ["meta", { property: "og:url", content: siteUrl }],
    ["meta", { name: "twitter:card", content: "summary_large_image" }]
  ],
  themeConfig: {
    logo: "/logo.svg",
    nav: enNav(),
    sidebar: {
      "/guide/": enSidebar()
    },
    search: {
      provider: "local"
    },
    socialLinks: [{ icon: "github", link: repoUrl }],
    outline: {
      label: "On this page"
    },
    docFooter: {
      prev: "Previous",
      next: "Next"
    },
    lastUpdated: {
      text: "Last updated"
    },
    langMenuLabel: "Change language",
    returnToTopLabel: "Return to top",
    sidebarMenuLabel: "Menu",
    darkModeSwitchLabel: "Theme"
  },
  locales: {
    root: {
      label: "English",
      lang: "en-US",
      title: "Kairox",
      description:
        "Local-first AI agent workbench with a shared Rust core, TUI, and Tauri desktop GUI."
    },
    zh: {
      label: "简体中文",
      lang: "zh-CN",
      link: "/zh/",
      title: "Kairox",
      description: "本地优先的 AI Agent 工作台，提供共享 Rust 核心、终端界面和 Tauri 桌面 GUI。",
      themeConfig: {
        nav: zhNav(),
        sidebar: {
          "/zh/guide/": zhSidebar()
        },
        outline: {
          label: "本页内容"
        },
        docFooter: {
          prev: "上一页",
          next: "下一页"
        },
        lastUpdated: {
          text: "最后更新"
        },
        langMenuLabel: "切换语言",
        returnToTopLabel: "返回顶部",
        sidebarMenuLabel: "菜单",
        darkModeSwitchLabel: "主题"
      }
    }
  }
});
