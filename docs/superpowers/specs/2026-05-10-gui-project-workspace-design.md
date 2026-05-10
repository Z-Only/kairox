# GUI Project Workspace Design

## Goal

Add first-class project workspace support to the Kairox desktop GUI while preserving the existing local-first, event-sourced, facade-driven architecture. Projects organize sessions, carry project-level context, expose Git/worktree state, and support safe removal and recovery without deleting user files or Git data.

## Context

Kairox currently has persistent sessions in the GUI, but the left sidebar is session-centric. The next step is to let users register projects, create project-scoped sessions, see Git context, and load project guidance such as `AGENTS.md` into agent conversations.

The accepted approach is **persistent project metadata + lightweight facade extensions + GUI-first delivery**. This keeps project state durable and recoverable without forcing a broad agent-loop rewrite in the first iteration.

## Non-goals

- Do not delete project directories, Git repositories, worktrees, or branches when removing a project from Kairox.
- Do not automatically initialize Git for existing folders without explicit user confirmation.
- Do not implement full directory-level instruction cascading in the first version.
- Do not replace the existing session model or break existing non-project sessions.
- Do not make drag-and-drop ordering mandatory for the first implementation; menu or button-based ordering is acceptable.

## Data model and lifecycle

### Project metadata

Introduce a durable `ProjectMeta` record owned by Kairox metadata storage.

Required fields:

- `project_id`: typed project identifier.
- `display_name`: user-facing project name. Defaults to the folder name and can be renamed in Kairox without renaming the disk directory.
- `root_path`: absolute project root path.
- `created_at` and `updated_at`.
- `removed_at`: nullable timestamp that marks a project as removed from the active project list.
- `sort_order` for project list ordering.
- `expanded` for sidebar expansion state.

Removing a project marks the Kairox registration as removed or archived. It does not touch the filesystem or Git repository.

### Project-session binding

Project sessions are represented by a durable binding between a session and project context.

Required fields:

- `session_id`.
- `project_id`.
- `worktree_path`.

A normal session has no `project_id`. A project session has both a `project_id` and a `worktree_path`. The branch is not stored as durable truth; it is read from the bound `worktree_path` when needed.

A project can contain sessions bound to different worktrees. A new project session defaults to `worktree_path = root_path`. A new worktree and branch are created only when the user explicitly asks for that action.

### Session visibility

Sessions need an explicit visibility state:

- `draft_hidden`: a persisted blank draft that is not shown in the sidebar.
- `visible`: a normal sidebar-visible session.
- `archived`: a session hidden from normal lists and shown in archive/history.

Existing sessions migrate to `visible` and remain ordinary non-project sessions.

### Blank draft lifecycle

1. The user creates a blank normal or project conversation.
2. Kairox persists a `draft_hidden` session so refresh or restart can recover the draft.
3. The sidebar does not show the draft before the first user message.
4. On first send, Kairox generates a temporary title from the first user message, truncating long input.
5. The session becomes `visible` and appears in the normal session list or under the bound project.
6. After the model response, Kairox can asynchronously generate a shorter title and replace the temporary title.

### Project removal and recovery lifecycle

1. The user removes a project from the sidebar.
2. Kairox shows a confirmation explaining that no files, repositories, worktrees, or branches will be deleted.
3. The project registration is marked removed.
4. Visible sessions under the project become `archived`.
5. The archive/history view can show those sessions with their original project name and path.
6. Restoring a project session restores the original project registration and places the session back under that project.
7. If the original path is missing, the project is still restored but marked `missing_path`.

## Backend architecture

### `agent-core`

Add project domain types in the core boundary so GUI, runtime, and store share the same language.

Core types:

- `ProjectId` typed newtype.
- `ProjectMeta`.
- `ProjectSessionBinding`.
- `ProjectSessionVisibility`.
- `ProjectGitStatus`.
- `ProjectInstructionSummary`.

Extend `AppFacade` with lightweight project metadata operations. These methods should manage project registration, session binding, visibility, and read-only Git status. They should not deeply couple Git worktree management into the agent loop.

### `agent-store`

Store project metadata in SQLite metadata tables alongside existing session metadata.

Responsibilities:

- Create and list project registrations.
- Rename projects.
- Persist project sort order and expansion state.
- Mark projects removed and restore them.
- Persist project-session bindings.
- Persist session visibility.
- Archive project sessions when their project is removed.
- Restore archived project sessions with the original project registration.

### `agent-runtime`

`LocalRuntime` exposes facade methods for project metadata and project session creation while reusing the existing session lifecycle.

When sending a project session, runtime should use the bound `worktree_path` as the project working context. Loading project instruction files is implemented as a context-assembly input before the agent request. Read failures should produce a non-blocking warning rather than blocking message send.

## Tauri IPC design

Add project commands in `apps/agent-gui/src-tauri/src/commands.rs`, register them in both `generate_handler![]` and `collect_commands![]`, and regenerate bindings with `just gen-types`.

Commands:

- `list_projects() -> Vec<ProjectInfoResponse>`.
- `create_blank_project(displayName?: string) -> ProjectInfoResponse`.
- `add_existing_project(path: String) -> ProjectInfoResponse`.
- `rename_project(projectId, displayName)`.
- `remove_project(projectId)`.
- `restore_project_session(sessionId)`.
- `update_project_order(projectIds)`.
- `update_project_expanded(projectId, expanded)`.
- `create_project_draft_session(projectId)`.
- `list_project_sessions(projectId)`.
- `list_archived_sessions()`.
- `create_project_worktree_session(projectId, branchName)`.
- `get_project_git_status(projectId)`.
- `get_session_git_status(sessionId)`.
- `init_project_git(projectId)`.

`SessionInfoResponse` is extended with nullable project fields while preserving all existing required fields so existing consumers continue to work:

- `projectId: string | null`.
- `worktreePath: string | null`.
- `branch: string | null`.
- `visibility: string | null`.

Existing `list_sessions` should keep its old semantic meaning: ordinary visible sessions only. It should not return hidden drafts or archived sessions.

## Git behavior

### New blank projects

Kairox-created blank projects use the default root `~/Kairox Projects`. The specific project directory is derived from the project name. If the name already exists, Kairox increments the directory name. Because Kairox creates this directory, it should run `git init` automatically.

### Existing folders

Adding an existing folder is allowed even when it is not a Git repository. Kairox should show `not_initialized` and provide an explicit initialization action. `git init` runs only after the user confirms.

### Git status

Expose structured status values:

- `not_initialized`.
- `clean`.
- `dirty`.
- `detached`.
- `missing_path`.
- `error`.

Branch display is read from the session's bound `worktree_path` at display time. It is not durable truth in Kairox metadata.

## Project instruction files

Project sessions should read project-level guidance from the project root.

Initial priority:

1. `AGENTS.md`.
2. Mainstream AI assistant files such as `CLAUDE.md`, `.cursorrules`, `GEMINI.md`, and `.windsurfrules`.
3. Existing project documentation such as `README.md` and `README.zh-CN.md`.

The first implementation reads root-level files only. Multiple hits are merged in priority order and record their source paths. The UI can display a compact summary such as `Loaded AGENTS.md, README.md`.

Instruction read failures should be visible but non-blocking. A missing instruction file is not an error.

## Frontend design

### Sidebar layout

`SessionsSidebar.vue` remains one sidebar but becomes section-based:

- Project section.
- Normal sessions section.

The project section appears above normal sessions by default. Users can reorder sections through a simple menu or button interaction. The order is persisted.

### Project section

Each project row shows:

- Expand/collapse control.
- Display name.
- Git status.
- Actions for rename, new project session, remove, and Git initialization when needed.

Expanding a project shows its visible project sessions. Project sessions do not appear in the normal sessions list.

### Normal sessions section

The normal sessions section shows only visible non-project sessions. It excludes hidden drafts, archived sessions, and project-bound sessions.

### Project creation

The sidebar project section provides two creation paths:

- Create a blank project under `~/Kairox Projects` and automatically initialize Git.
- Add an existing folder, allowing non-Git folders and showing `not_initialized` until the user initializes Git.

### Empty states

A blank normal conversation shows a simple prompt to start chatting.

A blank project conversation shows:

- Project display name.
- Project path.
- Git status.
- Worktree path and branch when available.
- Git initialization action when needed.
- Loaded instruction summary when available.

### Chat message layout

`ChatPanel.vue` should remove visible sender-name labels such as `User` and `Assistant`. Ownership is conveyed through layout and color:

- User messages align right and use an emphasized bubble background.
- Assistant, system, and tool messages align left.
- Tool output can keep structured card styling but should not display as a sender badge.

The design must maintain readable contrast in light and dark themes.

### Context meter and model metadata

`ContextMeter.vue` moves from the top chat header to the composer area, next to the send button. It becomes a compact ring indicator.

Behavior:

- The ring shows context usage percentage.
- Hover or focus shows token details, context window, remaining budget, and compaction state.
- Warning and danger states use both color and text.

The composer area also shows current provider and model near the input. If only the profile name is available, display the profile name first and expand to provider/model later.

### Worktree and branch metadata

Project conversations show worktree and branch metadata near the composer:

- `Worktree: /path/to/project`.
- `Branch: main`.
- `Not initialized` for non-Git folders.
- `Missing path` for deleted or moved folders.
- `Detached` for detached HEAD.

Branch values are refreshed from backend Git status rather than trusted from frontend cache.

### Archive/history

Add an archive/history entry in the sidebar footer or sessions menu.

The archive view shows archived sessions and, for project sessions, original project name and path. Restoring a project session restores the project registration and places the session back under that project. Missing paths are restored as `missing_path`, not recreated.

### Frontend state

Keep responsibilities separated:

- `session.ts`: existing session state, current session, message send, normal drafts, first-send visibility transition, temporary title generation.
- `project.ts`: project list, project sessions, project ordering, expansion state, Git status, create/add/remove/restore flows.
- `workspaceUi.ts`: section order, archive view state, sidebar UI preferences.

Avoid putting all project behavior into `session.ts`.

## Testing strategy

### Store and runtime tests

Add tests before implementation for:

- Project metadata creation and listing.
- Project rename changing only `display_name`.
- Project sort order and expansion persistence.
- Project removal archiving visible project sessions.
- Restoring archived project sessions and project registration.
- Hidden drafts not appearing in visible session lists.
- Existing sessions migrating to visible non-project sessions.
- Non-Git existing folders returning `not_initialized`.
- Explicit Git initialization changing status from `not_initialized` to initialized.
- Missing paths returning `missing_path`.
- Project instruction priority and non-blocking read failures.

### Frontend tests

Add Pinia and component tests for:

- `project.ts` store normalization and command calls.
- `session.ts` first-send draft visibility transition.
- Temporary title truncation from first user message.
- Project sessions excluded from normal session lists.
- `SessionsSidebar.vue` rendering project and normal sections.
- Project expansion rendering project sessions.
- Archive sessions excluded from normal lists.
- `ChatPanel.vue` rendering without sender labels.
- User message right alignment and background styling.
- Ring-mode `ContextMeter.vue` tooltip/focus details.

### E2E tests

Update `apps/agent-gui/e2e/tauri-mock.js` for all new or changed commands.

Add or extend Playwright scenarios for:

- Blank project creation and first project session send.
- Adding a non-Git folder and initializing Git after confirmation.
- Project removal and archive recovery.
- Chat message alignment and no sender labels.
- Context ring position near the send button.
- Worktree and branch metadata display in project sessions.

## Migration and compatibility

Existing sessions remain ordinary visible sessions. They have no project binding and continue to appear in the normal sessions section.

Database migration defaults:

- Existing session visibility: `visible`.
- Existing project binding: `NULL`.
- Default section order: projects first, sessions second.
- Default archived state: not archived.

IPC compatibility:

- Keep existing `list_sessions` behavior for ordinary visible sessions.
- Add archive and project-specific list commands instead of overloading old calls.
- Represent new project-related response fields as nullable values and keep existing required response fields unchanged.

## Phased delivery

### Phase 1: metadata and backend foundations

- Add project domain types.
- Add project metadata storage and migration.
- Add facade methods for project metadata and bindings.
- Add store/runtime tests.

### Phase 2: Tauri IPC and frontend stores

- Add project commands.
- Register commands for invocation and Specta generation.
- Run `just gen-types`.
- Add `project.ts` store.
- Extend `session.ts` for hidden drafts and first-send visibility.
- Update Tauri mock and store tests.

### Phase 3: sidebar project navigation

- Refactor sidebar into sections.
- Add project list and expansion.
- Add project session list.
- Add creation, add-folder, remove, and ordering controls.
- Add component and E2E coverage.

### Phase 4: chat composer and message polish

- Remove sender labels.
- Align message bubbles by role.
- Add model metadata near composer.
- Add worktree and branch metadata for project sessions.
- Add project-aware empty states.

### Phase 5: context ring and instruction summaries

- Add ring mode to `ContextMeter.vue`.
- Move it next to the send button.
- Add hover/focus detail display.
- Read root instruction files for project sessions.
- Display loaded instruction summary.

### Phase 6: archive/history recovery

- Add archive/history UI.
- Show archived project sessions with original project details.
- Restore project registration and sessions.
- Handle missing paths without recreating directories.

## Risks and mitigations

- Scope is large. Mitigate through phased commits where each phase is independently testable.
- Existing session behavior could regress. Mitigate by preserving `list_sessions` semantics and migrating old sessions to visible non-project sessions.
- Git operations could surprise users. Mitigate by only auto-initializing Kairox-created blank projects, requiring confirmation for existing folders, and never deleting Git data.
- Frontend state can become tangled. Mitigate by introducing `project.ts` instead of expanding `session.ts` into a project store.
- Instruction loading can destabilize sends. Mitigate by making read failures non-blocking and limiting the first version to root-level files.

## Locked implementation choices

These choices keep the implementation plan focused and avoid ambiguous scope:

- Store session visibility in a dedicated `session_visibility` metadata table keyed by `session_id`, rather than overloading existing `SessionMeta` fields.
- Add the project metadata repository as a focused `project_meta` module in `agent-store`, with tests colocated in that module.
- Add `project.ts` and `workspaceUi.ts` as separate Pinia setup-stores instead of expanding `session.ts` into a combined project/session store.
- Implement archive/history first as a sidebar subview opened from the sidebar footer, not as a new top-level route.
- Use explicit CSS classes for message alignment and context ring styling in the modified Vue components, backed by existing CSS custom properties from `theme.css`.
