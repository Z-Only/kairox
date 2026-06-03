import { defineConfig, type DefaultTheme } from "vitepress";
import { withMermaid } from "vitepress-plugin-mermaid";

const siteUrl = "https://z-only.github.io/kairox/";
const repoUrl = "https://github.com/Z-Only/kairox";
const editPattern = "https://github.com/Z-Only/kairox/edit/main/site/:path";

function enNav(): DefaultTheme.NavItem[] {
  return [
    { text: "Home", link: "/" },
    {
      text: "Guide",
      items: [
        { text: "Getting Started", link: "/guide/getting-started" },
        { text: "Installation", link: "/guide/installation" },
        { text: "First Session", link: "/guide/first-session" },
        { text: "Troubleshooting & FAQ", link: "/guide/troubleshooting" }
      ]
    },
    {
      text: "Concepts",
      items: [
        { text: "Architecture", link: "/concepts/architecture" },
        { text: "Runtime & Sessions", link: "/concepts/runtime-and-sessions" },
        { text: "Memory & Context", link: "/concepts/memory-and-context" },
        { text: "Permissions & Tools", link: "/concepts/permissions-and-tools" },
        { text: "Monitors", link: "/concepts/monitors" },
        { text: "Extensibility", link: "/concepts/extensibility" }
      ]
    },
    {
      text: "Reference",
      items: [
        { text: "Configuration", link: "/reference/configuration" },
        { text: "Crate Index", link: "/reference/crate-index" },
        { text: "CLI & Keyboard", link: "/reference/cli-and-keyboard" },
        { text: "Eval Harness", link: "/reference/eval-harness" }
      ]
    },
    {
      text: "Community",
      items: [
        { text: "Roadmap", link: "/community/roadmap" },
        { text: "Contributing", link: "/community/contributing" },
        { text: "Releases & Security", link: "/community/releases-and-security" }
      ]
    },
    {
      text: "Project",
      items: [
        { text: "GitHub", link: repoUrl },
        { text: "Releases", link: `${repoUrl}/releases` },
        { text: "Discussions", link: `${repoUrl}/discussions` }
      ]
    }
  ];
}

function zhNav(): DefaultTheme.NavItem[] {
  return [
    { text: "首页", link: "/zh/" },
    {
      text: "指南",
      items: [
        { text: "快速开始", link: "/zh/guide/getting-started" },
        { text: "安装", link: "/zh/guide/installation" },
        { text: "首次会话", link: "/zh/guide/first-session" },
        { text: "故障排查与 FAQ", link: "/zh/guide/troubleshooting" }
      ]
    },
    {
      text: "概念",
      items: [
        { text: "架构", link: "/zh/concepts/architecture" },
        { text: "运行时与会话", link: "/zh/concepts/runtime-and-sessions" },
        { text: "记忆与上下文", link: "/zh/concepts/memory-and-context" },
        { text: "权限与工具", link: "/zh/concepts/permissions-and-tools" },
        { text: "监控器", link: "/zh/concepts/monitors" },
        { text: "可扩展性", link: "/zh/concepts/extensibility" }
      ]
    },
    {
      text: "参考",
      items: [
        { text: "配置", link: "/zh/reference/configuration" },
        { text: "Crate 索引", link: "/zh/reference/crate-index" },
        { text: "命令与快捷键", link: "/zh/reference/cli-and-keyboard" },
        { text: "评估工具", link: "/zh/reference/eval-harness" }
      ]
    },
    {
      text: "社区",
      items: [
        { text: "路线图", link: "/zh/community/roadmap" },
        { text: "贡献指南", link: "/zh/community/contributing" },
        { text: "发布与安全", link: "/zh/community/releases-and-security" }
      ]
    },
    {
      text: "项目",
      items: [
        { text: "GitHub", link: repoUrl },
        { text: "Releases", link: `${repoUrl}/releases` },
        { text: "Discussions", link: `${repoUrl}/discussions` }
      ]
    }
  ];
}

function enSidebar(): DefaultTheme.Sidebar {
  return {
    "/guide/": [
      {
        text: "Guide",
        items: [
          { text: "Getting Started", link: "/guide/getting-started" },
          { text: "Installation", link: "/guide/installation" },
          { text: "First Session", link: "/guide/first-session" },
          { text: "Troubleshooting & FAQ", link: "/guide/troubleshooting" }
        ]
      }
    ],
    "/concepts/": [
      {
        text: "Concepts",
        items: [
          { text: "Architecture", link: "/concepts/architecture" },
          { text: "Runtime & Sessions", link: "/concepts/runtime-and-sessions" },
          { text: "Memory & Context", link: "/concepts/memory-and-context" },
          { text: "Permissions & Tools", link: "/concepts/permissions-and-tools" },
          { text: "Monitors", link: "/concepts/monitors" },
          { text: "Extensibility: MCP / Skills / Plugins", link: "/concepts/extensibility" }
        ]
      }
    ],
    "/reference/": [
      {
        text: "Reference",
        items: [
          { text: "Configuration", link: "/reference/configuration" },
          { text: "Crate Index", link: "/reference/crate-index" },
          { text: "CLI & Keyboard", link: "/reference/cli-and-keyboard" },
          { text: "Eval Harness", link: "/reference/eval-harness" }
        ]
      }
    ],
    "/community/": [
      {
        text: "Community",
        items: [
          { text: "Roadmap", link: "/community/roadmap" },
          { text: "Contributing", link: "/community/contributing" },
          { text: "Releases & Security", link: "/community/releases-and-security" }
        ]
      }
    ]
  };
}

function zhSidebar(): DefaultTheme.Sidebar {
  return {
    "/zh/guide/": [
      {
        text: "指南",
        items: [
          { text: "快速开始", link: "/zh/guide/getting-started" },
          { text: "安装", link: "/zh/guide/installation" },
          { text: "首次会话", link: "/zh/guide/first-session" },
          { text: "故障排查与 FAQ", link: "/zh/guide/troubleshooting" }
        ]
      }
    ],
    "/zh/concepts/": [
      {
        text: "概念",
        items: [
          { text: "架构", link: "/zh/concepts/architecture" },
          { text: "运行时与会话", link: "/zh/concepts/runtime-and-sessions" },
          { text: "记忆与上下文", link: "/zh/concepts/memory-and-context" },
          { text: "权限与工具", link: "/zh/concepts/permissions-and-tools" },
          { text: "监控器", link: "/zh/concepts/monitors" },
          { text: "可扩展性: MCP / 技能 / 插件", link: "/zh/concepts/extensibility" }
        ]
      }
    ],
    "/zh/reference/": [
      {
        text: "参考",
        items: [
          { text: "配置", link: "/zh/reference/configuration" },
          { text: "Crate 索引", link: "/zh/reference/crate-index" },
          { text: "命令与快捷键", link: "/zh/reference/cli-and-keyboard" },
          { text: "评估工具", link: "/zh/reference/eval-harness" }
        ]
      }
    ],
    "/zh/community/": [
      {
        text: "社区",
        items: [
          { text: "路线图", link: "/zh/community/roadmap" },
          { text: "贡献指南", link: "/zh/community/contributing" },
          { text: "发布与安全", link: "/zh/community/releases-and-security" }
        ]
      }
    ]
  };
}

export default withMermaid(
  defineConfig({
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
      sidebar: enSidebar(),
      search: {
        provider: "local"
      },
      socialLinks: [{ icon: "github", link: repoUrl }],
      editLink: {
        pattern: editPattern,
        text: "Edit this page on GitHub"
      },
      outline: {
        label: "On this page",
        level: [2, 3]
      },
      docFooter: {
        prev: "Previous",
        next: "Next"
      },
      lastUpdated: {
        text: "Last updated"
      },
      footer: {
        message: "Released under the Apache-2.0 License.",
        copyright: `© ${new Date().getFullYear()} Kairox contributors`
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
          sidebar: zhSidebar(),
          editLink: {
            pattern: editPattern,
            text: "在 GitHub 上编辑此页"
          },
          outline: {
            label: "本页内容",
            level: [2, 3]
          },
          docFooter: {
            prev: "上一页",
            next: "下一页"
          },
          lastUpdated: {
            text: "最后更新"
          },
          footer: {
            message: "基于 Apache-2.0 协议发布。",
            copyright: `© ${new Date().getFullYear()} Kairox 贡献者`
          },
          langMenuLabel: "切换语言",
          returnToTopLabel: "返回顶部",
          sidebarMenuLabel: "菜单",
          darkModeSwitchLabel: "主题"
        }
      }
    },
    mermaid: {
      theme: "default"
    }
  })
);
