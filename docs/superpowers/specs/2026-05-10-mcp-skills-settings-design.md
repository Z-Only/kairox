# MCP and Skills Settings Design

## Summary

Kairox will replace the standalone marketplace-oriented settings flow with two first-class settings pages: `MCP` and `Skills`. The `MCP` page will focus on configured MCP servers first, with marketplace discovery embedded as a secondary area. The `Skills` page will manage locally available skills across project, user, and built-in scopes, and will add remote discovery, GitHub installation, and update operations through a replaceable package manager abstraction.

The first implementation will use a hybrid approach for remote Skills operations: Kairox exposes stable Rust facade types and commands while the initial package manager implementation shells out to `npx skills`. This gets the full workflow working quickly without coupling the GUI or facade to CLI-specific behavior.

## Goals

- Replace the settings `Marketplace` tab with `MCP` and keep marketplace discovery inside the MCP page.
- Show configured MCP servers as the primary MCP view.
- Support MCP server enable/disable, start/stop/restart, trust/revoke trust, tool refresh, add, edit, delete, and open config file operations.
- Show installed and discovered Skills in a dedicated settings page.
- Support project-level, user-level, and built-in Skills with priority `project > user > built-in`.
- Support Skills enable/disable, activation mode, detail view, edit, delete, open file/directory, update, skills.sh search/install, and GitHub installation.
- Keep remote Skills implementation replaceable through a Rust trait so `npx skills` can be replaced by a native implementation later.
- Use `specta` for all new Tauri command bindings and regenerate TypeScript types with `just gen-types`.

## Non-goals

- Do not implement a native skills.sh or GitHub protocol client in the first version.
- Do not auto-start newly added or newly enabled MCP servers.
- Do not auto-install or auto-update Skills without an explicit user action.
- Do not make built-in Skills editable or deletable.
- Do not expose `npx skills`, TOML mutation details, or filesystem layout details directly to Vue components.

## Architecture

### `agent-core`

`agent-core` will define stable settings DTOs and facade methods for MCP and Skills management. These types form the GUI/TUI-facing boundary and must not expose implementation details such as `npx skills` command lines or raw TOML mutation internals.

Key concepts include:

- `McpServerSettingsView`: merged static configuration and runtime state for one server.
- `McpServerSettingsInput`: structured add/edit payload for stdio and SSE servers.
- `SkillSettingsView`: merged skill metadata, scope, enabled state, shadowing state, validity, and update state.
- `SkillSettingsDetail`: detailed information for one skill, including source chain and parse errors.
- `RemoteSkillSearchResult`: normalized remote search result from skills.sh-compatible sources.
- `SkillInstallTarget`: project or user installation target.

The `AppFacade` trait will gain methods for listing and mutating MCP settings, listing and mutating Skill settings, and invoking remote Skill search/install/update operations.

### `agent-runtime`

`agent-runtime` will orchestrate settings operations. It will merge static configuration, runtime state, trust state, tool counts, parse errors, and remote package manager results into facade DTOs.

For MCP, runtime owns the workflow for:

- Reading writable MCP configuration.
- Upserting, deleting, and enabling/disabling server entries.
- Stopping a running server before disabling or deleting it.
- Merging `McpServerManager` lifecycle state with config state.
- Opening the configuration file.

For Skills, runtime owns the workflow for:

- Discovering project, user, and built-in Skills.
- Applying priority and shadowing rules.
- Reading and writing per-scope Skills state files.
- Dispatching remote operations to `SkillPackageManager`.
- Returning degraded views when local state files or individual Skills are invalid.

### `agent-skills`

`agent-skills` remains responsible for local Skill discovery and parsing. It will add focused, testable support for local state and manifest operations.

Each writable scope will have a state file, for example:

- `<workspace>/.kairox/skills/skills-state.toml`
- `~/.kairox/skills/skills-state.toml`

The state file records enabled/disabled overrides, activation modes, install records, last update checks, and remote source metadata. `SKILL.md` remains the source of Skill content and metadata; runtime state is stored separately so third-party Skill files are not mutated.

### `SkillPackageManager`

A new trait will abstract remote package management:

- `search(query)`
- `install_from_registry(request)`
- `install_from_github(request)`
- `check_updates(request)`
- `update(request)`

The first implementation, `NpxSkillsPackageManager`, will execute `npx skills` commands. It will normalize results into facade DTOs and classify failures such as missing `npx`, non-zero exit status, network failures, and output parse failures. Tests should use a fake implementation for runtime behavior and separate fixtures for CLI output parsing.

### `agent-gui`

`SettingsView` will only expose `General`, `MCP`, and `Skills` top-level tabs. The existing standalone marketplace route may continue to redirect to settings, but marketplace UI is no longer a separate settings page.

Vue components call Pinia stores, Pinia stores call generated Tauri command bindings, and stores should not duplicate backend business rules. The backend returns views that are ready for rendering.

## MCP Page Design

`Settings -> MCP` is the primary MCP management page. It defaults to a server list and includes marketplace discovery as a secondary tab or section.

### Sections

- `Servers`: default section for configured MCP servers.
- `Marketplace`: embedded catalog/marketplace area reusing existing catalog UI where possible.

### Server list fields

Each server row or card shows:

- Name.
- Source, when identifiable.
- Transport type such as `stdio` or `sse`.
- Config enabled state.
- Runtime state such as stopped, starting, running, or failed.
- Trust state.
- Tool count and refresh state.
- Last error summary.

The list includes search and filters for enabled state, runtime state, and failed servers. Page-level actions include `Add server`, `Open config file`, and `Refresh all`.

### Server operations

- Enable/disable modifies config state. Disabling a running server stops it first. Enabling does not start it automatically.
- Start/stop/restart uses existing runtime lifecycle management.
- Trust/revoke trust reuses existing trust operations.
- Refresh tools updates one server without blocking the rest of the list.
- Edit opens a structured form for core fields such as name, transport, command/url, args, env, enabled, and description. Complex fields may use JSON/TOML text areas.
- Delete confirms the server name and source. If running, the server is stopped before deletion.
- Open config file opens the writable TOML file or shows a useful fallback message if no writable file can be located.

### Add server

The add form supports:

- Stdio: name, command, args, env, enabled, description.
- SSE: name, url, headers, enabled, description.

Saving writes config, reloads settings, refreshes the list, and does not auto-start the new server.

## Skills Page Design

`Settings -> Skills` is the unified Skills management page. It shows installed/discovered local Skills first, then supports remote discovery and installation.

### Scopes and priority

Skills are discovered from:

- Project scope: `<workspace>/.kairox/skills`.
- User scope: `~/.kairox/skills`.
- Built-in scope: bundled system Skills.

Priority is `project > user > built-in`. If the same Skill name exists in multiple scopes, the highest-priority one is the effective Skill. Lower-priority versions remain visible in detail as shadowed entries.

### Sections

- `Installed`: default list of local and built-in Skills.
- `Discover`: skills.sh search and install flow.
- `Install from GitHub`: direct install from GitHub shorthand or URL.

### Installed list fields

Each Skill row or card shows:

- Name and description from `SKILL.md` frontmatter.
- Scope: project, user, or built-in.
- Path, available in a collapsed or detail view.
- Enabled state.
- Activation mode: manual, suggest, or auto.
- Install source: local, skills.sh, GitHub, built-in, or unknown.
- Version or revision when available.
- Update state: up to date, update available, unknown, or check failed.
- Effective or shadowed state.
- Valid or invalid state with parse error details.

### Skill operations

- Enable/disable writes to the scope state file, not to `SKILL.md`.
- Activation mode changes persist in the state file.
- Detail view shows full metadata, path, source chain, shadowing reason, and parse errors.
- Edit is allowed for project and user Skills. Built-in Skills are read-only.
- Delete is allowed for project and user Skills after confirmation. Built-in Skills cannot be deleted.
- Update is shown only for remotely installed Skills.
- Open file and open directory help users inspect or manually repair a Skill.

### skills.sh discovery

Discover calls `search_remote_skills(query)`. Runtime dispatches to `SkillPackageManager::search`, and the first implementation runs `npx skills find <query>`. Results are normalized to show name, repository, description, install count, source link, and install summary.

Users explicitly choose the install target:

- User level: `~/.kairox/skills`.
- Project level: `<workspace>/.kairox/skills`.

After installation, Kairox refreshes the installed list.

### GitHub installation

The GitHub install form accepts source strings such as:

- `owner/repo`
- `owner/repo@skill-name`
- `https://github.com/owner/repo`
- `https://github.com/owner/repo/tree/main/skills/foo`

Runtime passes structured input to `SkillPackageManager::install_from_github`. The initial adapter delegates to `npx skills add <source>` while keeping the facade independent from CLI syntax.

## Error Handling

Errors should be recoverable, local, and actionable.

- MCP TOML parse errors show the config path, error summary, and `Open config file`. The page remains available.
- MCP single-server failures are shown on the affected row and do not block other servers.
- MCP config writes are atomic. Failed writes leave the old config intact.
- Invalid `SKILL.md` files appear as invalid Skills with detail errors instead of breaking the list.
- Broken `skills-state.toml` files produce a degraded view: discovered Skills remain visible, state is marked unknown, and the user can open the state file.
- Missing `npx` shows a Node.js/npm installation hint.
- Failed `npx skills` commands show exit status and a stderr summary.
- CLI output parse failures show a clear parse error and allow the user to try GitHub installation or inspect raw details.
- Delete operations include the affected path and scope in confirmation text.

## Testing Strategy

Implementation will follow TDD.

### Rust tests

`agent-skills` tests cover:

- Scope merging and priority rules.
- Shadowed Skill detection.
- State file read/write for disabled Skills, activation modes, install records, and update metadata.
- Degraded behavior for invalid state files.
- Invalid Skill parsing without breaking discovery.

`agent-runtime` tests cover:

- MCP settings view merging config, runtime status, trust status, tool count, and last error.
- Disabling a running MCP server stops it before writing disabled state.
- Enabling does not auto-start.
- MCP upsert and delete using temporary TOML fixtures.
- Skills list/detail/enable/disable/delete/search/install/update with fake discovery and fake package manager.

`NpxSkillsPackageManager` tests cover output parsing and error classification with fixtures, not live network calls.

### Tauri and type generation

Every new command uses `#[specta::specta]`, is registered in `collect_commands![]`, and is registered in `generate_handler![]`. After command changes, run `just gen-types` and update generated bindings.

### GUI tests

Pinia tests cover loading, mutation actions, errors, and no stale optimistic state after failures.

Vue tests cover:

- `SettingsView` only showing `General`, `MCP`, and `Skills`.
- `McpSettingsPane` rendering server-first UI and embedded marketplace.
- `SkillSettingsPane` rendering installed/discover/GitHub install flows.
- Shadowed, invalid, update-available, and built-in read-only states.

Playwright mock updates cover new Tauri commands, MCP management flows, Skills search/install happy path, and missing `npx` error state.

## Implementation Slices

1. Add `agent-core` settings DTOs and facade methods.
2. Implement MCP settings backend in `agent-runtime`.
3. Add Skills local state, manifest, and scope merge support in `agent-skills`.
4. Add `SkillPackageManager`, fake package manager, and `NpxSkillsPackageManager`.
5. Add Tauri commands, specta registration, type generation, and Playwright mock support.
6. Extend Pinia stores for MCP and Skills settings operations.
7. Rework `SettingsView` and add `McpSettingsPane` and `SkillSettingsPane` components.
8. Run focused Rust, GUI, type-generation, and E2E verification.

## Risks and Mitigations

- `npx skills` output may change. Mitigate by isolating parsing and returning useful parse errors without changing facade DTOs.
- Config writes may target the wrong file. Mitigate by writing only to explicit writable config paths and disabling edit/delete for non-writable sources.
- Skills path conventions may differ from existing defaults. Mitigate by preserving existing discovery behavior while adding new state files.
- GUI scope is large. Mitigate by keeping components focused and making stores consume backend-ready views.
- Existing GUI tests may fail until generated command bindings exist. Mitigate by running `just gen-types` after command registration.
