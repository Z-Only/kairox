/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- hooks commands ---- */

var _savedHooks = {
  user: [],
  project: []
};

var _hookTemplates = [
  {
    id: "stop-validation",
    name: "Run workspace tests on stop",
    description: "Runs the workspace test suite after an agent turn completes.",
    event: "Stop",
    matcher: "*",
    command: "cargo test --workspace --all-targets",
    statusMessage: "Running workspace tests",
    timeoutSecs: 600
  },
  {
    id: "prompt-secret-scan",
    name: "Scan prompts for secrets",
    description: "Checks submitted prompts before the agent sees them.",
    event: "UserPromptSubmit",
    matcher: "*",
    command: "python3 .kairox/hooks/prompt_secret_scan.py",
    statusMessage: "Scanning prompt",
    timeoutSecs: 30
  }
];

function hooksBucket(scope) {
  return scope === "Project" ? _savedHooks.project : _savedHooks.user;
}

function setHooksBucket(scope, hooks) {
  if (scope === "Project") {
    _savedHooks.project = hooks;
  } else {
    _savedHooks.user = hooks;
  }
}

registerCommandHandlers({
  get_hooks_settings: function () {
    return Promise.resolve({
      user: _savedHooks.user,
      project: _savedHooks.project,
      templates: _hookTemplates
    });
  },
  upsert_hook_settings: function (args) {
    var input = args.input;
    var hook = {
      id: input.id,
      scope: input.scope,
      event: input.event,
      matcher: input.matcher,
      command: input.command,
      statusMessage: input.statusMessage,
      timeoutSecs: input.timeoutSecs,
      enabled: input.enabled
    };
    var hooks = hooksBucket(input.scope).filter(function (existing) {
      return !(existing.id === input.id && existing.event === input.event);
    });
    hooks.push(hook);
    setHooksBucket(input.scope, hooks);
    return Promise.resolve(null);
  },
  delete_hook_settings: function (args) {
    var hooks = hooksBucket(args.scope).filter(function (existing) {
      return !(existing.id === args.id && existing.event === args.event);
    });
    setHooksBucket(args.scope, hooks);
    return Promise.resolve(null);
  }
});
