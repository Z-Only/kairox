# Plugin System Marketplace Design

## Goal

Add a Kairox plugin layer that can discover, configure, and install reusable bundles of agent capabilities at user and project scope, while staying compatible with the useful parts of Claude Code and Codex plugin layouts.

## References

- Claude Code plugins use `.claude-plugin/plugin.json` and may contribute skills, agents, hooks, MCP servers, LSP servers, monitors, `bin/`, and default settings.
- Claude Code marketplaces use `.claude-plugin/marketplace.json` with `name`, `owner`, and `plugins[]`, where each plugin has a `name` and a `source` such as a relative path, GitHub repo, Git URL, git subdirectory, or npm package.
- Claude Code plugin scopes include user, project, local, and managed. Kairox already has user and project settings flows, so the first Kairox implementation supports user and project only.
- Codex plugins use `.codex-plugin/plugin.json`, with metadata, `skills`, optional app connector metadata, optional `.mcp.json`, assets, and interface metadata.

## Scope

The first version supports:

- Built-in, user, and project plugin discovery.
- Plugin settings in a dedicated GUI Settings tab.
- Installed plugin enable/disable and delete for user/project scopes.
- Installing plugins from marketplace entries into user/project plugin directories.
- Marketplace source management for Claude-style marketplace JSON files.
- Parsing Kairox, Codex, and Claude plugin manifests.
- Discovering plugin-contributed skills under `skills/`.
- Discovering plugin-contributed MCP server definitions from `.mcp.json` and manifest `mcpServers` for inventory display.

The first version intentionally does not execute plugin hooks, monitors, `bin/` executables, LSP servers, or default settings. These are high-trust behaviors and need separate permission and runtime designs.

## Storage

Kairox plugin files live under:

- Built-in: application packaged directory, read-only.
- User: `~/.config/kairox/plugins/<plugin-name>/`.
- Project: `<workspace>/.kairox/plugins/<plugin-name>/`.

Each mutable scope owns `plugins-state.toml` next to installed plugin folders:

```toml
[plugins."github"]
enabled = true
install_source = "marketplace"
marketplace = "claude-plugins-official"
version = "0.1.0"
```

Marketplace sources live in `plugin_sources.toml` under the user config directory for now. Project-specific marketplace subscriptions can be added later if teams need shared source lists.

## Manifest

Kairox resolves manifests in this order:

1. `.kairox-plugin/plugin.json`
2. `.codex-plugin/plugin.json`
3. `.claude-plugin/plugin.json`

Supported fields:

- `name`, `version`, `description`, `author`, `homepage`, `repository`, `license`, `keywords`
- `skills`
- `mcpServers`
- `apps`
- `interface`

Unknown fields are preserved by ignoring them, not failing. Invalid required metadata marks the plugin invalid but keeps it visible in Settings.

## Runtime Integration

Plugin-contributed skills are converted into additional `SkillRoot`s. Skill names are namespaced as `<plugin-name>:<skill-name>` to avoid collisions with standalone Kairox skills.

Plugin-contributed MCP servers are shown in plugin detail inventory but not auto-started in this version. A future iteration can copy selected MCP server definitions into the existing MCP settings flow after explicit user approval.

## GUI

Settings adds a `Plugins` tab with:

- Installed: grouped by scope, showing metadata, enabled state, component inventory, path, and validation errors.
- Discover: searches enabled marketplace sources and installs to the selected user/project config source.
- Marketplaces: lists configured marketplace sources, supports add/remove/enable/disable/refresh.

The existing `ConfigSourceBar` is shown on the Plugins tab. User installs write to the user plugin directory; project installs write to the selected project's `.kairox/plugins` directory.

## Marketplace

Kairox supports Claude-style marketplace JSON because it is simple and already has ecosystem momentum. It also seeds safe, disabled-by-default recommended sources:

- `claude-plugins-official`: Anthropic official marketplace.
- `anthropics-claude-code`: Anthropic demo marketplace.

Third-party directory sites can be useful for discovery but are not reliable enough as built-in install sources. Users can add any compatible marketplace URL or GitHub shorthand from the GUI.

## Safety

Installing a plugin copies files into the Kairox plugin directory. Loading a plugin reads manifests, skills, and MCP JSON only. No executable component runs automatically.

Before adding support for executable components, Kairox needs:

- Per-component permission prompts.
- Trusted-source policy.
- Component sandboxing rules.
- Clear uninstall and cache cleanup behavior.

## Testing

Core tests cover manifest parsing, scope precedence, invalid plugin visibility, state persistence, and marketplace JSON parsing. GUI tests cover the Settings tab, source switching, marketplace source management, and install/enable/disable flows through the Tauri mock.
