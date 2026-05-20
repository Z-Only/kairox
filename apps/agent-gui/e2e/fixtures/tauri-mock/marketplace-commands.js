/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- marketplace commands ---- */

registerCommandHandlers({
  list_catalog: function (args) {
    return state.catalog;
  },
  get_catalog_entry: function (args) {
    var ce = state.catalog.find(function (e) {
      return e.id === args.id;
    });
    return ce || null;
  },
  refresh_catalog: function (args) {
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
  },
  install_catalog_entry: function (args) {
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
    var installSpec = JSON.parse(entry.install_spec_json);
    state.installedCatalog.push({
      server_id: req.catalog_id,
      catalog_id: req.catalog_id,
      source: req.source,
      display_name: entry.display_name,
      installed_at: new Date().toISOString(),
      running: !!req.auto_start
    });
    if (!findMcpSettingsServer(req.catalog_id)) {
      state.mcpSettingsServers.push({
        id: req.catalog_id,
        name: entry.display_name,
        transport: installSpec.transport || "stdio",
        enabled: true,
        runtime_status: req.auto_start ? "running" : "stopped",
        trusted: !!req.trust_grant,
        tool_count: req.auto_start ? 1 : null,
        last_error: null,
        writable: true,
        config_path: "/mock/workspace/kairox.toml",
        description: entry.description
      });
    }
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
  },
  uninstall_catalog_entry: function (args) {
    var uninstSession = state.currentSessionId;
    state.installedCatalog = state.installedCatalog.filter(function (e) {
      return e.server_id !== args.serverId;
    });
    state.mcpSettingsServers = state.mcpSettingsServers.filter(function (server) {
      return server.id !== args.serverId;
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
  },
  list_installed_entries: function (args) {
    return state.installedCatalog;
  },
  list_catalog_sources: function (args) {
    return state.catalogSources.slice();
  },
  add_catalog_source: function (args) {
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
  },
  remove_catalog_source: function (args) {
    var removeId = args.id;
    if (removeId === "builtin") return null;
    state.catalogSources = state.catalogSources.filter(function (s) {
      return s.id !== removeId;
    });
    return null;
  },
  set_catalog_source_enabled: function (args) {
    var setId = args.id;
    var setEnabled = args.enabled;
    state.catalogSources = state.catalogSources.map(function (s) {
      return s.id === setId ? Object.assign({}, s, { enabled: setEnabled }) : s;
    });
    return null;
  }
});
