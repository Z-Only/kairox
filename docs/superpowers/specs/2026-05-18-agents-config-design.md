# Agents Config Design

## Goal

Add Kairox-native configurable agents for specialized roles such as review, testing, exploration, and debugging. Agents are configurable at user and project scope, include built-in defaults, and are managed from a dedicated GUI settings tab.

## External References

Claude Code stores subagents as Markdown files with YAML frontmatter under `~/.claude/agents/` and `.claude/agents/`. Required fields are `name` and `description`; optional `tools` limits tool access; the Markdown body is the agent prompt.

Codex stores custom agents as TOML files under `~/.codex/agents/` and `.codex/agents/`. Required fields are `name`, `description`, and `developer_instructions`; optional fields can override model, reasoning effort, sandbox mode, MCP servers, and skills. Codex ships built-in `default`, `worker`, and `explorer` agents.

Claude Code has plugin marketplaces that can distribute agents as part of plugins. Codex has plugins, skills, and subagents documentation, but no separate first-party third-party agents marketplace equivalent was found. Kairox should reuse its existing skill/catalog patterns first and leave a dedicated agent marketplace as a later extension.

## Kairox File Format

Kairox uses Markdown with YAML frontmatter:

```md
---
name: code-reviewer
description: Review code for correctness, security, regressions, and missing tests.
tools: ["fs.read", "search", "shell"]
model_profile: "fast"
permission_mode: "read_only"
skills: ["kairox-dev-workflow"]
nickname_candidates: ["Reviewer", "Audit"]
enabled: true
---

Lead with findings. Focus on bugs, behavior regressions, security issues, and missing tests.
```

This keeps Claude-style hand editing while carrying Codex-style model, permission, and skill hints.

## Scopes And Precedence

Agents load from:

- Built-in defaults in code: `default`, `worker`, `explorer`, `code-reviewer`, `test-runner`.
- User scope: `~/.config/kairox/agents/*.md`.
- Project scope: `.kairox/agents/*.md`.

Precedence is project over user over built-in by agent `name`. All definitions remain visible in settings; the highest-priority enabled definition is marked effective, and shadowed lower-priority definitions show the active source.

## GUI

Settings gets a dedicated `Agents` tab using the existing `ConfigSourceBar`. The page includes:

- Installed list with source, enabled, effective, valid, model profile, permission mode, path, and actions.
- Editor form for creating and editing user/project agents.
- Read-only built-in entries with a copy-to-scope action.

Discovery/marketplace is intentionally not a separate first-party surface in this pass. The tab can show an explanatory empty state that agent marketplace support will build on existing catalog plumbing later.

## Runtime Integration

This pass exposes agent definitions and persists settings. It does not change task scheduling or automatic delegation. Future work can wire these definitions into DAG strategies and explicit agent invocation.

## Validation

Backend validation covers frontmatter parsing, required fields, name format, precedence, built-in immutability, user/project write paths, and delete safety. GUI tests cover tab navigation, list rendering, editor save, and scope-aware commands.
