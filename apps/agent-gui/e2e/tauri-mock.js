/**
 * Tauri IPC mock for Playwright E2E tests (PURE JAVASCRIPT).
 *
 * This file is injected via Playwright's addInitScript BEFORE any page JS runs.
 * It MUST be valid JavaScript (no TypeScript) because the browser evaluates it directly.
 *
 * It replaces @tauri-apps/api by providing a full __TAURI_INTERNALS__ shim
 * that the Tauri v2 API library uses internally, plus __TAURI_EVENT_PLUGIN_INTERNALS__.
 */

// @ts-nocheck

/* ---- State ---- */

const persistedStateKey = "__kairox_mock_state__";

let idCounter = 0;
function nextId(prefix) {
  return prefix + "_" + ++idCounter;
}

const state = {
  initialized: false,
  workspace: null,
  sessions: [],
  projects: [],
  projectSessions: new Map(),
  archivedSessions: [],
  gitStatuses: new Map(),
  currentSessionId: null,
  currentProfile: "fast",
  projections: new Map(),
  traces: new Map(),
  memories: [],
  permissionRequests: new Map(),
  agents: new Map(),
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

/* ---- Helpers ---- */

function getSession(sessionId) {
  const id = sessionId || state.currentSessionId;
  return state.sessions.find(function (s) {
    return s.id === id;
  });
}

function getProject(projectId) {
  return state.projects.find(function (project) {
    return project.project_id === projectId;
  });
}

function makeSessionInfo(id, title, profile, projectId, worktreePath, branch, visibility) {
  return {
    id: id,
    title: title,
    profile: profile,
    project_id: projectId || null,
    worktree_path: worktreePath || null,
    branch: branch || null,
    visibility: visibility || "visible"
  };
}

function makeProjectGitStatus(project) {
  return {
    kind: state.gitStatuses.get(project.project_id) || "not_initialized",
    branch: null,
    worktree_path: project.root_path,
    message: null
  };
}

function getProjectSessionList(projectId) {
  return state.projectSessions.get(projectId) || [];
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function snapshotMap(map) {
  return Array.from(map.entries()).map(function (entry) {
    return [entry[0], clone(entry[1])];
  });
}

function persistMockState() {
  try {
    localStorage.setItem(
      persistedStateKey,
      JSON.stringify({
        idCounter: idCounter,
        initialized: state.initialized,
        workspace: state.workspace,
        sessions: state.sessions,
        projects: state.projects,
        projectSessions: snapshotMap(state.projectSessions),
        archivedSessions: state.archivedSessions,
        gitStatuses: snapshotMap(state.gitStatuses),
        currentSessionId: state.currentSessionId,
        currentProfile: state.currentProfile,
        projections: snapshotMap(state.projections),
        traces: snapshotMap(state.traces),
        drafts: snapshotMap(state.drafts)
      })
    );
  } catch {
    // The mock can be evaluated in non-origin contexts where localStorage is unavailable.
  }
}

function restorePersistedMockState() {
  try {
    var raw = localStorage.getItem(persistedStateKey);
    if (!raw) return;
    var snapshot = JSON.parse(raw);
    idCounter = snapshot.idCounter || 0;
    state.initialized = Boolean(snapshot.initialized);
    state.workspace = snapshot.workspace || null;
    state.sessions = snapshot.sessions || [];
    state.projects = snapshot.projects || [];
    state.projectSessions = new Map(snapshot.projectSessions || []);
    state.archivedSessions = snapshot.archivedSessions || [];
    state.gitStatuses = new Map(snapshot.gitStatuses || []);
    state.currentSessionId = snapshot.currentSessionId || null;
    state.currentProfile = snapshot.currentProfile || "fast";
    state.projections = new Map(snapshot.projections || []);
    state.traces = new Map(snapshot.traces || []);
    state.drafts = new Map(snapshot.drafts || []);
  } catch {
    try {
      localStorage.removeItem(persistedStateKey);
    } catch {}
  }
}

function slugify(value) {
  return String(value)
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function findMcpSettingsServer(serverId) {
  return state.mcpSettingsServers.find(function (server) {
    return server.id === serverId;
  });
}

function findSkillSetting(skillId) {
  var settingsIdMatches = state.skillSettings.filter(function (skill) {
    return skill.settings_id === skillId;
  });
  if (settingsIdMatches.length === 1) {
    return settingsIdMatches[0];
  }
  if (settingsIdMatches.length > 1) {
    throw new Error("ambiguous skill settings id: " + skillId);
  }

  var legacyIdMatches = state.skillSettings.filter(function (skill) {
    return skill.id === skillId;
  });
  if (legacyIdMatches.length === 1) {
    return legacyIdMatches[0];
  }
  if (legacyIdMatches.length > 1) {
    throw new Error("ambiguous skill id: " + skillId);
  }

  return null;
}

function createMcpSettingsServer(input) {
  var serverId = slugify(input.name || "mcp-server");
  var transport =
    input.transport && input.transport.transport ? input.transport.transport : "stdio";
  return {
    id: serverId,
    name: input.name,
    transport: transport,
    enabled: input.enabled,
    runtime_status: input.enabled ? "running" : "stopped",
    trusted: false,
    tool_count: transport === "stdio" ? 1 : null,
    last_error: null,
    writable: true,
    config_path: "/mock/workspace/kairox.toml",
    description: input.description || null
  };
}

function createSkillSettingFromInstall(name, source, target, installSource) {
  var skillId = slugify(name);
  return {
    settings_id: target + ":" + skillId,
    id: skillId,
    name: name,
    description: "Installed from " + source + ".",
    version: "0.1.0",
    scope: target,
    path:
      target === "user"
        ? "/Users/mock/.kairox/skills/" + skillId + "/SKILL.md"
        : "/mock/workspace/.kairox/skills/" + skillId + "/SKILL.md",
    enabled: true,
    activation_mode: "manual",
    install_source: installSource,
    update_state: "up_to_date",
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    editable: true,
    deletable: true
  };
}

function configScopeFromSource(source) {
  switch (source) {
    case "builtin":
    case "defaults":
      return "Builtin";
    case "project":
    case "project_config":
      return "Project";
    case "local":
      return "Local";
    case "user":
    case "user_config":
    default:
      return "User";
  }
}

function effectiveMcpServerView(server) {
  var source = configScopeFromSource(server.source || "user_config");
  return {
    value: {
      id: server.id,
      name: server.name,
      transport: server.transport,
      enabled: server.enabled,
      runtime_status: server.runtime_status,
      trusted: server.trusted,
      tool_count: server.tool_count,
      last_error: server.last_error,
      writable: server.writable,
      config_path: server.config_path,
      description: server.description,
      source: server.source || "user_config",
      verified: server.verified ?? true
    },
    source: source,
    overrides: null,
    enabled: server.enabled,
    disabledBy: null,
    writable: server.writable,
    deletable: server.writable
  };
}

function effectiveSkillView(skill) {
  var source = configScopeFromSource(skill.scope);
  return {
    value: clone(skill),
    source: source,
    overrides: null,
    enabled: skill.enabled,
    disabledBy: null,
    writable: skill.editable,
    deletable: skill.deletable
  };
}

function getProjection(sessionId) {
  if (!state.projections.has(sessionId)) {
    state.projections.set(sessionId, {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    });
  }
  return state.projections.get(sessionId);
}

function getTrace(sessionId) {
  if (!state.traces.has(sessionId)) {
    state.traces.set(sessionId, []);
  }
  return state.traces.get(sessionId);
}

function makeEvent(sessionId, payload) {
  return {
    schema_version: 1,
    workspace_id: state.workspace ? state.workspace.workspace_id : "wrk_mock",
    session_id: sessionId,
    timestamp: new Date().toISOString(),
    source_agent_id: "agent_system",
    privacy: "full_trace",
    event_type: payload.type,
    payload: payload
  };
}

/**
 * Emit a Tauri-style event to all registered listeners.
 * In our mock, the handler was registered via transformCallback and stored
 * in our callbacks map. We invoke it directly.
 */
function emitEvent(eventName, payload) {
  var listeners = state.eventListeners.get(eventName);
  if (listeners) {
    listeners.forEach(function (handler, eventId) {
      setTimeout(function () {
        try {
          // Tauri v2 event handlers receive { event, id, payload }
          handler({ event: eventName, id: eventId, payload: payload });
        } catch (e) {
          console.error("[tauri-mock] Error in event listener for " + eventName + ":", e);
        }
      }, 10);
    });
  }
}

/* ---- transformCallback / unregisterCallback (Tauri v2 core) ---- */

function transformCallback(callback, once) {
  if (!callback) return 0;
  var id = state.nextCallbackId++;
  state.callbacks.set(id, { callback: callback, once: !!once });
  return id;
}

function unregisterCallback(id) {
  state.callbacks.delete(id);
}

function invokeCallback(id, args) {
  var entry = state.callbacks.get(id);
  if (entry) {
    if (entry.once) {
      state.callbacks.delete(id);
    }
    try {
      entry.callback.apply(null, args || []);
    } catch (e) {
      console.error("[tauri-mock] Error in callback " + id + ":", e);
    }
  }
}

/* ---- __TAURI_EVENT_PLUGIN_INTERNALS__ ---- */

var tauriEventPluginInternals = {
  unregisterListener: function (event, eventId) {
    var listeners = state.eventListeners.get(event);
    if (listeners) {
      listeners.delete(eventId);
    }
  }
};

/* ---- invoke handler ---- */

function invoke(cmd, args) {
  args = args || {};

  switch (cmd) {
    /* ─── Tauri v2 Event Plugin ──────────────────────────────── */
    case "plugin:event|listen": {
      var eventName = args.event;
      var handlerId = args.handler;
      if (!state.eventListeners.has(eventName)) {
        state.eventListeners.set(eventName, new Map());
      }
      // Store the wrapped handler that will be called when we emitEvent
      state.eventListeners.get(eventName).set(handlerId, function (evt) {
        // Invoke the callback that was registered via transformCallback
        invokeCallback(handlerId, [evt]);
      });
      // Return the handlerId as the eventId (this is what Tauri v2 returns)
      return Promise.resolve(handlerId);
    }

    case "plugin:event|unlisten": {
      var eventName = args.event;
      var eventId = args.eventId;
      var listeners = state.eventListeners.get(eventName);
      if (listeners) {
        listeners.delete(eventId);
      }
      return Promise.resolve(undefined);
    }

    /* ─── App commands ───────────────────────────────────────── */

    case "initialize_workspace": {
      if (state.initialized) return Promise.reject(new Error("Workspace already initialized"));
      var ws = { workspace_id: "wrk_mock", path: "/mock/workspace" };
      state.workspace = ws;
      state.initialized = true;
      // Auto-create a first session
      var sid = nextId("ses");
      var session = makeSessionInfo(sid, "Session using fast", "fast", null, null, null, "visible");
      state.sessions.push(session);
      state.currentSessionId = sid;
      state.projections.set(sid, {
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false
      });
      state.traces.set(sid, []);
      return Promise.resolve(ws);
    }

    case "list_profiles":
      return Promise.resolve(
        state.profiles.map(function (p) {
          return p.alias;
        })
      );

    case "list_profiles_with_limits":
      return Promise.resolve(
        state.profiles.map(function (p) {
          var window;
          var output;
          if (p.alias === "fast") {
            window = 128000;
            output = 16384;
          } else if (p.alias === "smart") {
            window = 200000;
            output = 16384;
          } else {
            window = 4096;
            output = 2048;
          }
          return {
            alias: p.alias,
            provider: p.provider,
            model_id: p.model_id,
            context_window: window,
            output_limit: output,
            limit_source: "builtin_registry",
            has_api_key: p.has_api_key
          };
        })
      );

    case "get_profile_info":
      return Promise.resolve(state.profiles);

    case "get_profile_detail": {
      var profile = args.profile || "fast";
      var found = state.profiles.find(function (p) {
        return p.alias === profile;
      });
      if (!found) return Promise.reject(new Error("Profile '" + profile + "' not found"));
      return Promise.resolve(found);
    }

    case "start_session": {
      var profile = args.profile || "fast";
      var sid = nextId("ses");
      var session = makeSessionInfo(
        sid,
        "Session using " + profile,
        profile,
        null,
        null,
        null,
        "visible"
      );
      state.sessions.push(session);
      state.currentSessionId = sid;
      state.projections.set(sid, {
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false
      });
      state.traces.set(sid, []);
      // Emit SessionInitialized event
      var event = makeEvent(sid, {
        type: "SessionInitialized",
        model_profile: profile
      });
      getTrace(sid).push(event);
      emitEvent("session-event", event);
      return Promise.resolve(session);
    }

    case "send_message": {
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var content = args.content;
      var projection = getProjection(sessionId);
      var trace = getTrace(sessionId);
      // UserMessageAdded
      var userMsgId = nextId("msg");
      var userEvent = makeEvent(sessionId, {
        type: "UserMessageAdded",
        message_id: userMsgId,
        content: content
      });
      trace.push(userEvent);
      emitEvent("session-event", userEvent);
      // Simulate agent response asynchronously
      setTimeout(function () {
        var ctxEvent = makeEvent(sessionId, {
          type: "ContextAssembled",
          usage: {
            total_tokens: 50000,
            budget_tokens: 100000,
            context_window: 128000,
            output_reservation: 28000,
            by_source: [
              ["system", 25000],
              ["history", 25000]
            ],
            estimator: "cl100k_base",
            corrected_by_real_usage: false
          }
        });
        trace.push(ctxEvent);
        emitEvent("session-event", ctxEvent);
        var modelEvent = makeEvent(sessionId, {
          type: "ModelRequestStarted",
          model_profile: state.currentProfile,
          model_id: "gpt-4o-mini"
        });
        trace.push(modelEvent);
        emitEvent("session-event", modelEvent);
        var tokens = ["Hello! ", "I'm a mock ", "assistant."];
        var delay = 50;
        for (var i = 0; i < tokens.length; i++) {
          (function (token, d) {
            setTimeout(function () {
              var deltaEvent = makeEvent(sessionId, {
                type: "ModelTokenDelta",
                delta: token
              });
              trace.push(deltaEvent);
              emitEvent("session-event", deltaEvent);
            }, d);
          })(tokens[i], delay);
          delay += 100;
        }
        setTimeout(function () {
          var assistantMsgId = nextId("msg");
          var fullContent = "Hello! I'm a mock assistant.";
          var completedEvent = makeEvent(sessionId, {
            type: "AssistantMessageCompleted",
            message_id: assistantMsgId,
            content: fullContent
          });
          trace.push(completedEvent);
          emitEvent("session-event", completedEvent);
        }, delay + 50);
      }, 30);
      return Promise.resolve(undefined);
    }

    case "switch_session": {
      var sessionId = args.sessionId || args.session_id;
      if (!sessionId) return Promise.reject(new Error("sessionId is required"));
      state.currentSessionId = sessionId;
      var session = getSession(sessionId);
      if (session) state.currentProfile = session.profile;
      return Promise.resolve(getProjection(sessionId));
    }

    case "get_trace": {
      var sessionId = args.sessionId || args.session_id;
      if (!sessionId) return Promise.reject(new Error("sessionId is required"));
      return Promise.resolve(
        getTrace(sessionId).map(function (e) {
          return JSON.stringify(e);
        })
      );
    }

    case "list_sessions":
      return Promise.resolve(
        state.sessions.filter(function (session) {
          return !session.project_id && session.visibility === "visible";
        })
      );

    case "list_projects":
      return Promise.resolve(state.projects);

    case "create_blank_project": {
      var projectId = nextId("prj");
      var displayName = args.displayName || args.display_name || "New Project";
      var project = {
        project_id: projectId,
        display_name: displayName,
        root_path: "/mock/workspace/" + displayName.replace(/\s+/g, "-").toLowerCase(),
        removed_at: null,
        sort_order: state.projects.length,
        expanded: true
      };
      state.projects.push(project);
      state.projectSessions.set(projectId, []);
      return Promise.resolve(project);
    }

    case "add_existing_project": {
      var projectPath = args.path || "/mock/workspace/existing-project";
      var parts = projectPath.split(/[\\/]/).filter(Boolean);
      var projectName = parts.length > 0 ? parts[parts.length - 1] : "Existing Project";
      var existingProjectId = nextId("prj");
      var existingProject = {
        project_id: existingProjectId,
        display_name: projectName,
        root_path: projectPath,
        removed_at: null,
        sort_order: state.projects.length,
        expanded: true
      };
      state.projects.push(existingProject);
      state.projectSessions.set(existingProjectId, []);
      return Promise.resolve(existingProject);
    }

    case "remove_project": {
      var removeProjectId = args.projectId || args.project_id;
      state.projects = state.projects.map(function (project) {
        return project.project_id === removeProjectId
          ? Object.assign({}, project, { removed_at: new Date().toISOString() })
          : project;
      });
      return Promise.resolve(undefined);
    }

    case "restore_project_session": {
      var restoreSessionId = args.sessionId || args.session_id;
      var archivedSession = state.archivedSessions.find(function (session) {
        return session.id === restoreSessionId;
      });
      if (!archivedSession) return Promise.reject(new Error("Archived session not found"));
      archivedSession.visibility = "visible";
      state.archivedSessions = state.archivedSessions.filter(function (session) {
        return session.id !== restoreSessionId;
      });
      var restoredProject = getProject(archivedSession.project_id);
      if (!restoredProject) return Promise.reject(new Error("Project not found"));
      var restoredProjectSessions = getProjectSessionList(restoredProject.project_id);
      restoredProjectSessions.push(archivedSession);
      state.projectSessions.set(restoredProject.project_id, restoredProjectSessions);
      return Promise.resolve(restoredProject);
    }

    case "create_project_draft_session": {
      var draftProjectId = args.projectId || args.project_id;
      var draftProject = getProject(draftProjectId);
      if (!draftProject) return Promise.reject(new Error("Project not found"));
      var draftSessionId = nextId("ses");
      var draftSession = makeSessionInfo(
        draftSessionId,
        "New conversation",
        "fast",
        draftProjectId,
        draftProject.root_path,
        null,
        "draft_hidden"
      );
      state.sessions.push(draftSession);
      var projectSessions = getProjectSessionList(draftProjectId);
      projectSessions.unshift(draftSession);
      state.projectSessions.set(draftProjectId, projectSessions);
      state.currentSessionId = draftSessionId;
      state.currentProfile = draftSession.profile;
      state.projections.set(draftSessionId, {
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false
      });
      state.traces.set(draftSessionId, []);
      return Promise.resolve(draftSessionId);
    }

    case "list_project_sessions": {
      var listProjectId = args.projectId || args.project_id;
      return Promise.resolve(
        getProjectSessionList(listProjectId).filter(function (session) {
          return session.visibility !== "archived";
        })
      );
    }

    case "list_archived_sessions":
      return Promise.resolve(state.archivedSessions);

    case "get_project_git_status": {
      var statusProjectId = args.projectId || args.project_id;
      var statusProject = getProject(statusProjectId);
      if (!statusProject) return Promise.reject(new Error("Project not found"));
      return Promise.resolve(makeProjectGitStatus(statusProject));
    }

    case "get_session_git_status": {
      var statusSessionId = args.sessionId || args.session_id;
      var statusSession = getSession(statusSessionId);
      if (!statusSession) return Promise.reject(new Error("Session not found"));
      return Promise.resolve({
        kind: "not_initialized",
        branch: statusSession.branch,
        worktree_path: statusSession.worktree_path || "/mock/workspace",
        message: null
      });
    }

    case "init_project_git": {
      var initProjectId = args.projectId || args.project_id;
      var initProject = getProject(initProjectId);
      if (!initProject) return Promise.reject(new Error("Project not found"));
      state.gitStatuses.set(initProjectId, "clean");
      return Promise.resolve(makeProjectGitStatus(initProject));
    }

    case "get_project_instruction_summary":
      return Promise.resolve({ source_paths: [], warning: null });

    case "list_workspaces":
      return Promise.resolve(state.workspace ? [state.workspace] : []);

    case "restore_workspace": {
      var _workspaceId = args.workspaceId || args.workspace_id;
      if (state.sessions.length > 0) {
        state.currentSessionId = state.sessions[0].id;
      }
      return Promise.resolve(undefined);
    }

    case "resolve_permission": {
      var requestId = args.requestId || args.request_id;
      var decision = args.decision;
      var request = state.permissionRequests.get(requestId);
      if (!request)
        return Promise.reject(new Error("Permission request " + requestId + " not found"));
      var sessionId = state.currentSessionId;
      if (sessionId) {
        if (decision === "grant") {
          var event = makeEvent(sessionId, {
            type: "PermissionGranted",
            request_id: requestId
          });
          getTrace(sessionId).push(event);
          emitEvent("session-event", event);
          setTimeout(function () {
            var invId = nextId("inv");
            var startEvent = makeEvent(sessionId, {
              type: "ToolInvocationStarted",
              invocation_id: invId,
              tool_id: request.tool_id
            });
            getTrace(sessionId).push(startEvent);
            emitEvent("session-event", startEvent);
            setTimeout(function () {
              var compEvent = makeEvent(sessionId, {
                type: "ToolInvocationCompleted",
                invocation_id: invId,
                tool_id: request.tool_id,
                output_preview: "Output of " + request.tool_id,
                exit_code: 0,
                duration_ms: 150,
                truncated: false
              });
              getTrace(sessionId).push(compEvent);
              emitEvent("session-event", compEvent);
            }, 100);
          }, 50);
        } else {
          var event = makeEvent(sessionId, {
            type: "PermissionDenied",
            request_id: requestId,
            reason: args.reason || "User denied"
          });
          getTrace(sessionId).push(event);
          emitEvent("session-event", event);
        }
      }
      state.permissionRequests.delete(requestId);
      return Promise.resolve(undefined);
    }

    case "query_memories": {
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
    }

    case "delete_memory": {
      var id = args.id;
      state.memories = state.memories.filter(function (m) {
        return m.id !== id;
      });
      return Promise.resolve(undefined);
    }

    case "rename_session": {
      var sessionId = args.sessionId || args.session_id;
      var title = args.title;
      var session = getSession(sessionId);
      if (session) session.title = title;
      return Promise.resolve(undefined);
    }

    case "delete_session": {
      var sessionId = args.sessionId || args.session_id;
      state.sessions = state.sessions.filter(function (s) {
        return s.id !== sessionId;
      });
      state.projections.delete(sessionId);
      state.traces.delete(sessionId);
      if (state.currentSessionId === sessionId) {
        state.currentSessionId = state.sessions.length > 0 ? state.sessions[0].id : null;
      }
      return Promise.resolve(undefined);
    }

    case "cancel_session": {
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var projection = getProjection(sessionId);
      projection.cancelled = true;
      var event = makeEvent(sessionId, {
        type: "SessionCancelled",
        reason: "User cancelled"
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return Promise.resolve(undefined);
    }

    case "compact_session": {
      var sid = state.currentSessionId;
      if (!sid) return Promise.reject(new Error("No active session"));
      var startedEvent = makeEvent(sid, {
        type: "ContextCompactionStarted",
        reason: { type: "UserRequested" },
        before_tokens: 12000,
        candidate_event_count: 4
      });
      getTrace(sid).push(startedEvent);
      emitEvent("session-event", startedEvent);
      setTimeout(function () {
        var summaryEvent = makeEvent(sid, {
          type: "CompactionSummary",
          summary_id: "sum_mock_1",
          content: "## User goal\nMock summary content for E2E.",
          replaces_event_range: [new Date().toISOString(), new Date().toISOString()],
          reason: { type: "UserRequested" },
          before_tokens: 12000,
          after_tokens: 3000,
          summarised_by_profile: state.currentProfile
        });
        getTrace(sid).push(summaryEvent);
        emitEvent("session-event", summaryEvent);
        var completedEvent = makeEvent(sid, {
          type: "ContextCompactionCompleted",
          summary_id: "sum_mock_1",
          after_tokens: 3000,
          fallback_used: false
        });
        getTrace(sid).push(completedEvent);
        emitEvent("session-event", completedEvent);
      }, 100);
      return Promise.resolve(undefined);
    }

    case "switch_model": {
      var alias = args && (args.profileAlias || args.profile_alias);
      var switchSid = (args && (args.sessionId || args.session_id)) || state.currentSessionId;
      if (!alias) {
        return Promise.reject(new Error("profileAlias required"));
      }
      if (!switchSid) {
        return Promise.reject(new Error("No active session"));
      }
      var fromProfile = state.currentProfile;
      if (fromProfile === alias) {
        return Promise.resolve(null); // same-profile: silent no-op (mirrors runtime)
      }
      // Resolve new limits from the same table list_profiles_with_limits uses.
      var newWindow;
      var newOutput;
      if (alias === "fast") {
        newWindow = 128000;
        newOutput = 16384;
      } else if (alias === "smart") {
        newWindow = 200000;
        newOutput = 16384;
      } else {
        // Unknown alias → reject like the real runtime does
        // (agent-core::CoreError::InvalidState).
        return Promise.reject(new Error("Unknown model profile: " + alias));
      }
      state.currentProfile = alias;
      var switchedEvent = makeEvent(switchSid, {
        type: "ModelProfileSwitched",
        from_profile: fromProfile,
        to_profile: alias,
        effective_at: new Date().toISOString(),
        context_window: newWindow,
        output_limit: newOutput,
        limit_source: "builtin_registry"
      });
      getTrace(switchSid).push(switchedEvent);
      emitEvent("session-event", switchedEvent);
      return Promise.resolve(null);
    }

    case "get_task_graph": {
      var sessionId = args.sessionId || args.session_id;
      if (!sessionId) return Promise.reject(new Error("sessionId is required"));
      return Promise.resolve(getProjection(sessionId).task_graph.tasks);
    }

    case "get_permission_mode":
      return Promise.resolve("Interactive");

    case "get_build_info":
      return Promise.resolve({
        version: "0.12.0-e2e",
        git_hash: "mock",
        build_time: "2026-05-05"
      });

    case "list_skills":
      return Promise.resolve(state.skills);

    case "get_skill_detail": {
      var detailSkill = state.skills.find(function (skill) {
        return skill.id === args.skillId;
      });
      if (!detailSkill) return Promise.reject(new Error("Skill not found: " + args.skillId));
      return Promise.resolve({
        view: detailSkill,
        body_markdown: "# " + detailSkill.name + "\n\n" + detailSkill.description
      });
    }

    case "activate_skill": {
      var skillToActivate = state.skills.find(function (skill) {
        return skill.id === args.skillId;
      });
      if (!skillToActivate) return Promise.reject(new Error("Skill not found: " + args.skillId));
      var activeSkill = {
        skill_id: skillToActivate.id,
        name: skillToActivate.name,
        source: skillToActivate.source,
        activation_mode: skillToActivate.activation_mode
      };
      state.activeSkills = state.activeSkills.filter(function (skill) {
        return skill.skill_id !== activeSkill.skill_id;
      });
      state.activeSkills.push(activeSkill);
      return Promise.resolve(activeSkill);
    }

    case "deactivate_skill":
      state.activeSkills = state.activeSkills.filter(function (skill) {
        return skill.skill_id !== args.skillId;
      });
      return Promise.resolve(null);

    case "list_active_skills":
      return Promise.resolve(state.activeSkills);

    case "list_mcp_server_settings":
      return clone(state.mcpSettingsServers);

    case "get_effective_mcp_servers":
      return clone(state.mcpSettingsServers.map(effectiveMcpServerView));

    case "upsert_mcp_server_settings": {
      var savedServer = createMcpSettingsServer(args.input);
      state.mcpSettingsServers = state.mcpSettingsServers.filter(function (server) {
        return server.id !== savedServer.id;
      });
      state.mcpSettingsServers.push(savedServer);
      return clone(savedServer);
    }

    case "set_mcp_server_enabled": {
      var serverToToggle = findMcpSettingsServer(args.serverId);
      if (!serverToToggle)
        return Promise.reject(new Error("MCP server not found: " + args.serverId));
      serverToToggle.enabled = args.enabled;
      serverToToggle.runtime_status = args.enabled ? "running" : "stopped";
      return null;
    }

    case "delete_mcp_server_settings":
      state.mcpSettingsServers = state.mcpSettingsServers.filter(function (server) {
        return server.id !== args.serverId;
      });
      return null;

    case "open_mcp_config_file":
      if (window.__MCP_OPEN_CONFIG_SHOULD_FAIL__) {
        return Promise.reject(new Error("mock failure"));
      }
      return "/mock/workspace";

    case "list_skill_settings":
      return clone(state.skillSettings);

    case "get_effective_skills":
      return clone(state.skillSettings.map(effectiveSkillView));

    case "get_skill_settings_detail": {
      var detailSetting = findSkillSetting(args.skillId);
      if (!detailSetting)
        return Promise.reject(new Error("Skill setting not found: " + args.skillId));
      return {
        view: clone(detailSetting),
        content: "# " + detailSetting.name + "\n\n" + detailSetting.description,
        source_chain: [clone(detailSetting)]
      };
    }

    case "set_skill_enabled": {
      var skillToToggle = findSkillSetting(args.skillId);
      if (!skillToToggle)
        return Promise.reject(new Error("Skill setting not found: " + args.skillId));
      skillToToggle.enabled = args.enabled;
      return null;
    }

    case "delete_skill_settings": {
      var skillToDelete = findSkillSetting(args.skillId);
      if (!skillToDelete)
        return Promise.reject(new Error("Skill setting not found: " + args.skillId));
      state.skillSettings = state.skillSettings.filter(function (skill) {
        return skill.settings_id !== skillToDelete.settings_id;
      });
      return null;
    }

    case "search_remote_skills": {
      var query = String(args.query || "").toLowerCase();
      return clone(
        state.remoteSkillResults.filter(function (result) {
          return (
            result.name.toLowerCase().indexOf(query) !== -1 ||
            result.description.toLowerCase().indexOf(query) !== -1 ||
            result.package.toLowerCase().indexOf(query) !== -1
          );
        })
      );
    }

    case "install_remote_skill": {
      var remoteRequest = args.request;
      var remoteResult = state.remoteSkillResults.find(function (result) {
        return result.package === remoteRequest.package;
      });
      var remoteName = remoteResult ? remoteResult.name : remoteRequest.package;
      var remoteSkill = createSkillSettingFromInstall(
        remoteName,
        remoteRequest.source,
        remoteRequest.target,
        "registry"
      );
      state.skillSettings = state.skillSettings.filter(function (skill) {
        return skill.settings_id !== remoteSkill.settings_id;
      });
      state.skillSettings.push(remoteSkill);
      return clone(remoteSkill);
    }

    case "install_github_skill": {
      var githubRequest = args.request;
      var githubName = githubRequest.source.split("/").pop() || "GitHub Skill";
      githubName = githubName.replace(/\.git$/, "").replace(/[-_]+/g, " ");
      var githubSkill = createSkillSettingFromInstall(
        githubName,
        githubRequest.source,
        githubRequest.target,
        "github"
      );
      state.skillSettings = state.skillSettings.filter(function (skill) {
        return skill.settings_id !== githubSkill.settings_id;
      });
      state.skillSettings.push(githubSkill);
      return clone(githubSkill);
    }

    case "update_skill": {
      var skillToUpdate = findSkillSetting(args.skillId);
      if (!skillToUpdate)
        return Promise.reject(new Error("Skill setting not found: " + args.skillId));
      skillToUpdate.update_state = "up_to_date";
      return clone(skillToUpdate);
    }

    case "list_mcp_servers":
      return state.mcpSettingsServers.map(function (server) {
        return {
          id: server.id,
          status: server.runtime_status,
          tool_count: server.tool_count
        };
      });
    case "start_mcp_server": {
      var serverToStart = findMcpSettingsServer(args.serverId);
      if (serverToStart) serverToStart.runtime_status = "running";
      return null;
    }
    case "stop_mcp_server": {
      var serverToStop = findMcpSettingsServer(args.serverId);
      if (serverToStop) serverToStop.runtime_status = "stopped";
      return null;
    }
    case "trust_mcp_server": {
      var serverToTrust = findMcpSettingsServer(args.serverId);
      if (serverToTrust) serverToTrust.trusted = true;
      return null;
    }
    case "revoke_mcp_trust": {
      var serverToRevoke = findMcpSettingsServer(args.serverId);
      if (serverToRevoke) serverToRevoke.trusted = false;
      return null;
    }
    case "refresh_mcp_tools": {
      var serverToRefresh = findMcpSettingsServer(args.serverId);
      if (serverToRefresh) serverToRefresh.tool_count = (serverToRefresh.tool_count || 0) + 1;
      return [{ name: "echo", description: "Echo tool", input_schema: null }];
    }
    case "check_mcp_health": {
      var serverToCheck = findMcpSettingsServer(args.serverId);
      if (!serverToCheck)
        return Promise.reject(new Error("MCP server not found: " + args.serverId));
      return {
        tools: [{ name: "echo", description: "Echo tool", input_schema: null }],
        healthy: true,
        error: null
      };
    }
    case "get_mcp_tool_states":
      return { disabled_tools: [] };
    case "set_mcp_tool_disabled":
      return null;
    case "test_mcp_connectivity": {
      var serverToTest = findMcpSettingsServer(args.serverId);
      if (!serverToTest) return Promise.reject(new Error("MCP server not found: " + args.serverId));
      return { status: "connected", tool_count: serverToTest.tool_count || 1 };
    }
    case "list_mcp_resources":
      return [];
    case "list_mcp_prompts":
      return [];
    case "read_mcp_resource":
      return [];

    /* ─── Marketplace commands ───────────────────────────────── */

    case "list_catalog": {
      return state.catalog;
    }

    case "get_catalog_entry": {
      var ce = state.catalog.find(function (e) {
        return e.id === args.id;
      });
      return ce || null;
    }

    case "refresh_catalog": {
      var refreshSource = args.source || "aggregate";
      var refreshSession = state.currentSessionId;
      if (refreshSession) {
        var refreshEvent = makeEvent(refreshSession, {
          type: "CatalogRefreshed",
          source: refreshSource,
          entry_count: state.catalog.length
        });
        getTrace(refreshSession).push(refreshEvent);
        emitEvent("session-event", refreshEvent);
      }
      return null;
    }

    case "install_catalog_entry": {
      var req = args.request;
      var entry = state.catalog.find(function (e) {
        return e.id === req.catalog_id;
      });
      if (!entry) {
        return Promise.reject(new Error("catalog entry not found: " + req.catalog_id));
      }
      var reqs = JSON.parse(entry.requirements_json);
      var baseMissing = reqs
        .filter(function (r) {
          return !state.catalogRuntimePresent[r.kind];
        })
        .map(function (r) {
          return r.kind;
        });
      // Test hook: e2e specs may set window.__MARKETPLACE_FORCE_MISSING__
      // to a string[] of runtime kinds to force a runtime_missing outcome.
      var forced = (typeof window !== "undefined" && window.__MARKETPLACE_FORCE_MISSING__) || null;
      var missing = forced && Array.isArray(forced) && forced.length > 0 ? forced : baseMissing;
      var sessionId = state.currentSessionId;
      if (missing.length > 0) {
        if (sessionId) {
          var missingEvent = makeEvent(sessionId, {
            type: "CatalogRuntimeMissing",
            catalog_id: req.catalog_id,
            missing: missing
          });
          getTrace(sessionId).push(missingEvent);
          emitEvent("session-event", missingEvent);
        }
        return {
          kind: "runtime_missing",
          server_id: null,
          started: null,
          missing_runtimes: missing,
          missing_env_keys: []
        };
      }
      var defaults = JSON.parse(entry.default_env_json);
      var missingEnv = defaults
        .filter(function (d) {
          return d.required && !req.env_overrides[d.key] && !d.default;
        })
        .map(function (d) {
          return d.key;
        });
      if (missingEnv.length > 0) {
        return {
          kind: "invalid_env",
          server_id: null,
          started: null,
          missing_runtimes: [],
          missing_env_keys: missingEnv
        };
      }
      if (
        state.installedCatalog.find(function (e) {
          return e.server_id === req.catalog_id;
        })
      ) {
        return {
          kind: "already_installed",
          server_id: req.catalog_id,
          started: null,
          missing_runtimes: [],
          missing_env_keys: []
        };
      }
      state.installedCatalog.push({
        server_id: req.catalog_id,
        catalog_id: req.catalog_id,
        source: req.source,
        display_name: entry.display_name,
        installed_at: new Date().toISOString(),
        running: !!req.auto_start
      });
      if (sessionId) {
        var installingEvent = makeEvent(sessionId, {
          type: "CatalogEntryInstalling",
          catalog_id: req.catalog_id,
          source: req.source
        });
        getTrace(sessionId).push(installingEvent);
        emitEvent("session-event", installingEvent);
        var installedEvent = makeEvent(sessionId, {
          type: "CatalogEntryInstalled",
          catalog_id: req.catalog_id,
          source: req.source,
          server_id: req.catalog_id
        });
        getTrace(sessionId).push(installedEvent);
        emitEvent("session-event", installedEvent);
      }
      return {
        kind: "installed",
        server_id: req.catalog_id,
        started: !!req.auto_start,
        missing_runtimes: [],
        missing_env_keys: []
      };
    }

    case "uninstall_catalog_entry": {
      var uninstSession = state.currentSessionId;
      state.installedCatalog = state.installedCatalog.filter(function (e) {
        return e.server_id !== args.serverId;
      });
      if (uninstSession) {
        var uninstEvent = makeEvent(uninstSession, {
          type: "CatalogEntryUninstalled",
          server_id: args.serverId
        });
        getTrace(uninstSession).push(uninstEvent);
        emitEvent("session-event", uninstEvent);
      }
      return null;
    }

    case "list_installed_entries": {
      return state.installedCatalog;
    }

    case "retry_task": {
      var taskId = args.taskId || args.task_id;
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
        return t.id === taskId;
      });
      if (task) {
        task.state = "Running";
        task.retry_count = (task.retry_count || 0) + 1;
      }
      var event = makeEvent(sessionId, {
        type: "TaskRetried",
        task_id: taskId,
        attempt: task ? task.retry_count : 1
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return Promise.resolve(undefined);
    }

    case "cancel_task": {
      var taskId = args.taskId || args.task_id;
      var sessionId = state.currentSessionId;
      if (!sessionId) return Promise.reject(new Error("No active session"));
      var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
        return t.id === taskId;
      });
      if (task) {
        task.state = "Cancelled";
      }
      return Promise.resolve(undefined);
    }

    /* ─── Phase 2: catalog source commands ───────────────────── */

    case "list_catalog_sources": {
      return state.catalogSources.slice();
    }

    case "add_catalog_source": {
      var addReq = args.request;
      if (
        state.catalogSources.find(function (s) {
          return s.id === addReq.id;
        })
      ) {
        return Promise.reject(new Error("source already exists: " + addReq.id));
      }
      state.catalogSources.push({
        id: addReq.id,
        display_name: addReq.display_name,
        kind: addReq.kind,
        url: addReq.url,
        api_key_env: addReq.api_key_env || null,
        priority: addReq.priority != null ? addReq.priority : 100,
        default_trust: addReq.default_trust || "community",
        enabled: addReq.enabled != null ? addReq.enabled : true,
        cache_ttl_seconds: addReq.cache_ttl_seconds || null,
        last_error: null
      });
      var addSession = state.currentSessionId;
      if (addSession) {
        var addEvent = makeEvent(addSession, {
          type: "CatalogSourceAdded",
          source: addReq.id,
          kind: addReq.kind
        });
        getTrace(addSession).push(addEvent);
        emitEvent("session-event", addEvent);
      }
      return null;
    }

    case "remove_catalog_source": {
      var removeId = args.id;
      if (removeId === "builtin") return null;
      state.catalogSources = state.catalogSources.filter(function (s) {
        return s.id !== removeId;
      });
      return null;
    }

    case "set_catalog_source_enabled": {
      var setId = args.id;
      var setEnabled = args.enabled;
      state.catalogSources = state.catalogSources.map(function (s) {
        return s.id === setId ? Object.assign({}, s, { enabled: setEnabled }) : s;
      });
      return null;
    }

    /* ─── Phase 3: skill catalog commands ──────────────────────── */

    case "list_skill_catalog": {
      var sq = args.query;
      var entries = state.skillCatalog;
      if (sq && sq.keyword) {
        var kw = sq.keyword.toLowerCase();
        entries = entries.filter(function (e) {
          return (
            e.name.toLowerCase().indexOf(kw) !== -1 ||
            e.description.toLowerCase().indexOf(kw) !== -1 ||
            e.package.toLowerCase().indexOf(kw) !== -1
          );
        });
      }
      return clone(entries);
    }

    case "list_skill_sources": {
      return state.skillCatalogSources.slice();
    }

    case "add_skill_source": {
      var addCfg = args.config;
      state.skillCatalogSources.push(Object.assign({}, addCfg, { last_error: null }));
      return null;
    }

    case "remove_skill_source": {
      state.skillCatalogSources = state.skillCatalogSources.filter(function (s) {
        return s.id !== args.id;
      });
      return null;
    }

    case "set_skill_source_enabled": {
      state.skillCatalogSources = state.skillCatalogSources.map(function (s) {
        return s.id === args.id ? Object.assign({}, s, { enabled: args.enabled }) : s;
      });
      return null;
    }

    case "refresh_skill_catalog": {
      return null;
    }

    case "list_profile_settings": {
      return state.profiles.map(function (p) {
        return {
          alias: p.alias,
          provider: p.provider,
          model_id: p.model_id,
          enabled: p.enabled !== false,
          context_window: p.context_window ?? null,
          output_limit: p.output_limit ?? null,
          temperature: p.temperature ?? null,
          top_p: p.top_p ?? null,
          top_k: p.top_k ?? null,
          max_tokens: p.max_tokens ?? null,
          base_url: p.base_url ?? null,
          api_key_env: p.api_key_env ?? null,
          has_api_key: p.has_api_key !== false,
          writable: p.writable !== false,
          config_path: p.config_path ?? null,
          source: p.source ?? "profiles_toml"
        };
      });
    }

    case "upsert_profile_settings": {
      var upsertInput = args && (args.input || args);
      var existing = state.profiles.find(function (p) {
        return p.alias === upsertInput.alias;
      });
      if (existing) {
        Object.assign(existing, upsertInput);
      } else {
        state.profiles.push(
          Object.assign({ writable: true, source: "profiles_toml" }, upsertInput)
        );
      }
      return state.profiles.find(function (p) {
        return p.alias === upsertInput.alias;
      });
    }

    case "set_profile_enabled": {
      var target = state.profiles.find(function (p) {
        return p.alias === args.alias;
      });
      if (target) target.enabled = args.enabled;
      return null;
    }

    case "delete_profile_settings": {
      state.profiles = state.profiles.filter(function (p) {
        return p.alias !== args.alias;
      });
      return null;
    }

    case "move_profile_in_order": {
      return null;
    }

    case "open_config_dir": {
      return "/mock/path/to/config";
    }

    case "list_workspace_files": {
      return Promise.resolve({ paths: state.workspaceFiles.slice() });
    }

    case "save_draft": {
      state.drafts.set(args.request.session_id, args.request.draft_text);
      return Promise.resolve(undefined);
    }

    case "get_draft": {
      var draftSessionId = args.sessionId || args.session_id;
      return Promise.resolve(state.drafts.get(draftSessionId) || "");
    }

    default:
      console.warn("[tauri-mock] Unknown invoke: " + cmd, args);
      return Promise.resolve(undefined);
  }
}

/* ---- Install mock into window ---- */

function installMock() {
  // __TAURI_INTERNALS__ — the core IPC bridge used by @tauri-apps/api
  window.__TAURI_INTERNALS__ = {
    invoke: function (cmd, args, _options) {
      return invoke(cmd, args);
    },
    transformCallback: function (callback, once) {
      return transformCallback(callback, once);
    },
    unregisterCallback: function (id) {
      unregisterCallback(id);
    },
    convertFileSrc: function (filePath, _protocol) {
      return "http://localhost/asset/" + filePath;
    }
  };

  // __TAURI_EVENT_PLUGIN_INTERNALS__ — used by @tauri-apps/api/event.js
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = tauriEventPluginInternals;

  // Expose for test hooks
  window.__KAIROX_MOCK__ = {
    state: state,
    simulatePermissionRequest: function (toolId, preview) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var requestId = nextId("prm");
      state.permissionRequests.set(requestId, {
        tool_id: toolId,
        preview: preview
      });
      var event = makeEvent(sessionId, {
        type: "PermissionRequested",
        request_id: requestId,
        tool_id: toolId,
        preview: preview
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateMemoryProposal: function (scope, key, content) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var memoryId = nextId("mem");
      var event = makeEvent(sessionId, {
        type: "MemoryProposed",
        memory_id: memoryId,
        scope: scope,
        key: key,
        content: content
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return memoryId;
    },
    simulateTaskCreated: function (title, role) {
      role = role || "Worker";
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var taskId = nextId("tsk");
      var event = makeEvent(sessionId, {
        type: "AgentTaskCreated",
        task_id: taskId,
        title: title,
        role: role,
        dependencies: []
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
      return taskId;
    },
    simulateAgentSpawned: function (agentId, role, taskId) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      state.agents.set(agentId, {
        id: agentId,
        role: role,
        taskId: taskId || null,
        status: "running",
        startedAt: Date.now(),
        completedAt: null
      });
      var event = makeEvent(sessionId, {
        type: "AgentSpawned",
        agent_id: agentId,
        role: role,
        task_id: taskId || ""
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateAgentIdle: function (agentId) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var agent = state.agents.get(agentId);
      if (agent) {
        agent.status = "idle";
        agent.completedAt = Date.now();
      }
      var event = makeEvent(sessionId, {
        type: "AgentIdle",
        agent_id: agentId
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateTaskDecomposed: function (parentId, subTaskIds) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var event = makeEvent(sessionId, {
        type: "TaskDecomposed",
        parent_task_id: parentId,
        sub_task_ids: subTaskIds
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    simulateTaskBlocked: function (taskId, blockingTaskId, reason) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var task = getProjection(sessionId).task_graph.tasks.find(function (t) {
        return t.id === taskId;
      });
      if (task) {
        task.state = "Blocked";
        task.error = reason || "Dependency failed";
      }
      var event = makeEvent(sessionId, {
        type: "TaskBlocked",
        task_id: taskId,
        blocking_task_id: blockingTaskId,
        reason: reason || "Dependency failed"
      });
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },

    simulateTaskTransition: function (taskId, eventType, error) {
      var sessionId = state.currentSessionId;
      if (!sessionId) return;
      var payload = { type: eventType, task_id: taskId };
      if (eventType === "AgentTaskFailed" && error) payload.error = error;
      var event = makeEvent(sessionId, payload);
      getTrace(sessionId).push(event);
      emitEvent("session-event", event);
    },
    persistForReload: persistMockState,
    reset: function () {
      state.initialized = false;
      state.workspace = null;
      state.sessions = [];
      state.projects = [];
      state.projectSessions.clear();
      state.archivedSessions = [];
      state.gitStatuses.clear();
      state.currentSessionId = null;
      state.currentProfile = "fast";
      state.projections.clear();
      state.traces.clear();
      state.memories = [];
      state.permissionRequests.clear();
      state.agents.clear();
      state.drafts.clear();
      state.callbacks.clear();
      state.eventListeners.clear();
      try {
        localStorage.removeItem(persistedStateKey);
      } catch {}
      idCounter = 0;
    }
  };
}

restorePersistedMockState();
installMock();
