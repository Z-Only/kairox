/**
 * Browser-side Tauri mock fragment — agent settings fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.agentSettings = [
  {
    settingsId: "Builtin:worker",
    name: "worker",
    description: "Execution-focused agent.",
    scope: "Builtin",
    path: "builtin://worker",
    tools: [],
    modelProfile: null,
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
];
