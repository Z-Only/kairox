/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- State fixtures ---- */

const persistedStateKey = "__kairox_mock_state__";

let idCounter = 0;
function nextId(prefix) {
  return prefix + "_" + ++idCounter;
}

const state = {
  initialized: false,
  workspace: null,
  sessions: [],
  projects: [
    {
      project_id: "prj_mock",
      display_name: "Mock Project",
      root_path: "/mock/workspace",
      removed_at: null,
      sort_order: 0,
      expanded: true
    }
  ],
  projectSessions: new Map(),
  archivedSessions: [],
  gitStatuses: new Map(),
  currentSessionId: null,
  currentProfile: "fast",
  currentPermissionMode: "suggest",
  projections: new Map(),
  traces: new Map(),
  memories: [],
  permissionRequests: new Map(),
  agents: new Map(),
  nextOpenDialogResult: null,
  /** Tauri v2 event system: eventName → Map<eventId, handler> */
  eventListeners: new Map(),
  drafts: new Map(),
  workspaceFiles: [
    "apps/agent-gui/src/components/ChatComposer.vue",
    "apps/agent-gui/src/components/FileMentionPalette.vue",
    "apps/agent-gui/e2e/chat-flow.spec.ts",
    "apps/agent-gui/e2e/tauri-mock.js",
    "crates/agent-core/src/lib.rs",
    "README.md"
  ],
  profiles: [
    {
      alias: "fast",
      provider: "openai",
      model_id: "gpt-4o-mini",
      local: false,
      has_api_key: true
    },
    {
      alias: "smart",
      provider: "openai",
      model_id: "gpt-4o",
      local: false,
      has_api_key: true
    },
    {
      alias: "fake",
      provider: "fake",
      model_id: "fake-model",
      local: true,
      has_api_key: false
    }
  ],
  /** Callback registry for transformCallback */
  callbacks: new Map(),
  nextCallbackId: 1,
  /** Marketplace fixtures (Phase 1 builtin catalog) */
  catalog: [
    {
      id: "filesystem",
      source: "builtin",
      display_name: "Filesystem",
      summary: "Read, write, and search files inside an allow-listed directory.",
      description: "Provides safe filesystem access scoped to a workspace path.",
      categories: ["filesystem", "dev-tools"],
      tags: ["files", "fs"],
      author: "MCP",
      homepage: "https://github.com/modelcontextprotocol/servers",
      version: "0.6.0",
      trust: "verified",
      icon: "📁",
      install_spec_json: JSON.stringify({
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", "${WORKSPACE_PATH}"],
        env: {},
        cwd: null
      }),
      requirements_json: JSON.stringify([
        {
          kind: "node",
          min_version: ">=18.0.0",
          install_hint: "https://nodejs.org"
        }
      ]),
      default_env_json: JSON.stringify([
        {
          key: "WORKSPACE_PATH",
          label: "Workspace path",
          description: "Directory the server can read",
          required: true,
          secret: false,
          default: "/tmp"
        }
      ])
    }
  ],
  installedCatalog: [],
  skills: [
    {
      id: "test-driven-rust",
      name: "test-driven-rust",
      description: "Write Rust changes test-first.",
      version: "1.0.0",
      source: "builtin:/skills/test-driven-rust",
      activation_mode: "manual",
      keywords: ["rust", "tdd"],
      tools: [],
      can_request_tools: [],
      valid: true,
      validation_error: null
    },
    {
      id: "broken-skill",
      name: "broken-skill",
      description: "Fixture for validation errors.",
      version: null,
      source: "workspace:/skills/broken-skill",
      activation_mode: "manual",
      keywords: [],
      tools: [],
      can_request_tools: [],
      valid: false,
      validation_error: "Missing required description"
    }
  ],
  activeSkills: [],
  agentSettings: [
    {
      settingsId: "Builtin:worker",
      name: "worker",
      description: "Execution-focused agent.",
      scope: "Builtin",
      path: "builtin://worker",
      tools: [],
      modelProfile: null,
      permissionMode: "workspace_write",
      skills: [],
      nicknameCandidates: ["Worker"],
      enabled: true,
      instructions: "Implement scoped changes.",
      effective: true,
      shadowedBy: null,
      valid: true,
      validationError: null,
      editable: false,
      deletable: false
    },
    {
      settingsId: "User:code-reviewer",
      name: "code-reviewer",
      description: "Review changed code before handoff.",
      scope: "User",
      path: "/Users/mock/.config/kairox/agents/code-reviewer.md",
      tools: ["fs.read", "search"],
      modelProfile: "smart",
      permissionMode: "read_only",
      skills: ["kairox-dev-workflow"],
      nicknameCandidates: ["Reviewer"],
      enabled: true,
      instructions: "Lead with concrete findings.",
      effective: true,
      shadowedBy: null,
      valid: true,
      validationError: null,
      editable: true,
      deletable: true
    }
  ],
  mcpSettingsServers: [
    {
      id: "github",
      name: "GitHub",
      transport: "stdio",
      enabled: true,
      runtime_status: "running",
      trusted: true,
      tool_count: 6,
      last_error: null,
      writable: true,
      config_path: "/mock/workspace/kairox.toml",
      description: "GitHub MCP server for repository automation."
    },
    {
      id: "builtin-docs",
      name: "Built-in Docs",
      transport: "sse",
      enabled: false,
      runtime_status: "stopped",
      trusted: false,
      tool_count: null,
      last_error: "Disabled by project policy",
      writable: false,
      config_path: null,
      description: "Read-only built-in documentation server."
    }
  ],
  skillSettings: [
    {
      settings_id: "project:project-review",
      id: "project-review",
      name: "Project Review",
      description: "Project-scoped code review guidance.",
      version: "1.0.0",
      scope: "project",
      path: "/mock/workspace/.kairox/skills/project-review/SKILL.md",
      enabled: true,
      activation_mode: "manual",
      install_source: "local",
      update_state: "up_to_date",
      effective: true,
      shadowed_by: null,
      valid: true,
      validation_error: null,
      editable: true,
      deletable: true
    },
    {
      settings_id: "user:user-planning",
      id: "user-planning",
      name: "User Planning",
      description: "User-scoped planning defaults.",
      version: "2.1.0",
      scope: "user",
      path: "/Users/mock/.kairox/skills/user-planning/SKILL.md",
      enabled: true,
      activation_mode: "auto",
      install_source: "local",
      update_state: "up_to_date",
      effective: true,
      shadowed_by: null,
      valid: true,
      validation_error: null,
      editable: true,
      deletable: true
    },
    {
      settings_id: "builtin:builtin-brainstorming",
      id: "builtin-brainstorming",
      name: "Built-in Brainstorming",
      description: "Built-in ideation workflow.",
      version: "5.1.0",
      scope: "builtin",
      path: "builtin:/skills/brainstorming/SKILL.md",
      enabled: true,
      activation_mode: "suggest",
      install_source: "builtin",
      update_state: "unknown",
      effective: false,
      shadowed_by: "project-review",
      valid: true,
      validation_error: null,
      editable: false,
      deletable: false
    },
    {
      settings_id: "project:invalid-workspace-skill",
      id: "invalid-workspace-skill",
      name: "Invalid Workspace Skill",
      description: "Fixture for parse errors.",
      version: null,
      scope: "project",
      path: "/mock/workspace/.kairox/skills/invalid/SKILL.md",
      enabled: false,
      activation_mode: "manual",
      install_source: "local",
      update_state: "check_failed",
      effective: false,
      shadowed_by: null,
      valid: false,
      validation_error: "Missing required description",
      editable: true,
      deletable: true
    },
    {
      settings_id: "project:registry-review",
      id: "registry-review",
      name: "Registry Review",
      description: "Registry-installed review helper.",
      version: "0.3.0",
      scope: "project",
      path: "/mock/workspace/.kairox/skills/registry-review/SKILL.md",
      enabled: true,
      activation_mode: "manual",
      install_source: "registry",
      update_state: "update_available",
      effective: true,
      shadowed_by: null,
      valid: true,
      validation_error: null,
      editable: true,
      deletable: true
    },
    {
      settings_id: "user:github-triage",
      id: "github-triage",
      name: "GitHub Triage",
      description: "GitHub-installed issue triage helper.",
      version: "0.1.0",
      scope: "user",
      path: "/Users/mock/.kairox/skills/github-triage/SKILL.md",
      enabled: true,
      activation_mode: "manual",
      install_source: "github",
      update_state: "up_to_date",
      effective: true,
      shadowed_by: null,
      valid: true,
      validation_error: null,
      editable: true,
      deletable: true
    }
  ],
  remoteSkillResults: [
    {
      name: "Code Review Assistant",
      description: "Reviews changes before handoff.",
      repository: "https://github.com/example/code-review-skill",
      install_count: 1240,
      source_url: "https://registry.example/skills/code-review-assistant",
      package: "@kairox/skill-code-review"
    },
    {
      name: "Planning Coach",
      description: "Turns requirements into implementation plans.",
      repository: "https://github.com/example/planning-coach",
      install_count: 860,
      source_url: "https://registry.example/skills/planning-coach",
      package: "@kairox/skill-planning-coach"
    },
    {
      name: "Code Review Assistant",
      description: "Reviews changes before handoff.",
      repository: "https://github.com/example/code-review-skill",
      install_count: 1240,
      source_url: "https://registry.example/skills/code-review-assistant",
      package: "skillhub/code-review-assistant"
    },
    {
      name: "Planning Coach",
      description: "Turns requirements into implementation plans.",
      repository: "https://github.com/example/planning-coach",
      install_count: 860,
      source_url: "https://registry.example/skills/planning-coach",
      package: "skillhub/planning-coach"
    }
  ],
  skillCatalog: [
    {
      catalog_id: "skillhub/code-review-assistant",
      name: "Code Review Assistant",
      description: "Reviews changes before handoff.",
      source: "skillhub",
      source_url: "https://registry.example/skills/code-review-assistant",
      install_count: 1240,
      github_stars: 450,
      security_score: 92,
      rating: 4.7,
      package: "skillhub/code-review-assistant"
    },
    {
      catalog_id: "skillhub/planning-coach",
      name: "Planning Coach",
      description: "Turns requirements into implementation plans.",
      source: "skillhub",
      source_url: "https://registry.example/skills/planning-coach",
      install_count: 860,
      github_stars: 220,
      security_score: 88,
      rating: 4.3,
      package: "skillhub/planning-coach"
    }
  ],
  catalogRuntimePresent: { node: true, python: true, uvx: true, docker: true },
  // Phase 2: catalog sources — only user-configured remote sources are listed here.
  // The builtin source is implicit (the GUI's source chip bar always renders a
  // "Built-in" chip in addition to whatever list_catalog_sources returns).
  catalogSources: [],
  skillCatalogSources: [
    {
      id: "skillhub",
      display_name: "SkillHub",
      kind: "skillhub",
      url: "https://skills.palebluedot.live",
      search_template: "/api/skills?q={{query}}&limit={{limit}}",
      list_template: "/api/skills?limit={{limit}}",
      field_mapping: {
        name_path: "name",
        description_path: "description",
        install_count_path: "downloadCount",
        github_stars_path: "githubStars",
        package_path: "id",
        source_url_path: null
      },
      enabled: true,
      priority: 20,
      cache_ttl_seconds: 900,
      last_error: null
    }
  ]
};
