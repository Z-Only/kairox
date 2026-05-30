/**
 * Browser-side Tauri mock fragment — skill fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.skills = [
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
];
state.activeSkills = [];
state.skillSettings = [
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
    tools: ["fs.read"],
    can_request_tools: ["registry"],
    permission_summary: "tools: fs.read; can request: registry",
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
    tools: [],
    can_request_tools: [],
    permission_summary: "no tool permissions declared",
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
    tools: [],
    can_request_tools: [],
    permission_summary: "no tool permissions declared",
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
    tools: [],
    can_request_tools: [],
    permission_summary: "no tool permissions declared",
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
    tools: ["registry.search"],
    can_request_tools: [],
    permission_summary: "tools: registry.search",
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
    tools: [],
    can_request_tools: ["github"],
    permission_summary: "can request: github",
    editable: true,
    deletable: true
  }
];
state.remoteSkillResults = [
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
];
state.skillCatalog = [
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
];
state.skillCatalogSources = [
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
];
