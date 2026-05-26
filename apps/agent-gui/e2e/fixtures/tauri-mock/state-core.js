/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * Core state scaffold and shared utilities.
 * MUST load first — all other fixture files add properties to `state`.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/** Shared ID counter for mock data generation */
let idCounter = 0;
function nextId(prefix) {
  return prefix + "_" + ++idCounter;
}

const persistedStateKey = "__kairox_mock_state__";

const state = {
  initialized: false,
  workspace: null,
  currentSessionId: null,
  currentProfile: "fast",
  currentReasoningEffort: null,
  currentApprovalPolicy: "on_request",
  currentSandboxPolicy: '{"kind":"workspace_write"}',
  sentMessages: [],
  responseDelayScale: 1,
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
  /** Callback registry for transformCallback */
  callbacks: new Map(),
  nextCallbackId: 1
};
