# Project-local Config Path Design

## Summary

Move Kairox project-local configuration from the old repository-root `kairox.toml` path to `./.kairox/config.toml`, matching the filename and directory shape of the existing user-level `~/.kairox/config.toml` configuration. The old `./kairox.toml` path will no longer be discovered or treated as a fallback.

## Goals

- Use `./.kairox/config.toml` as the only project-local configuration path.
- Continue using `~/.kairox/config.toml` as the user-level fallback.
- Stop reading `./kairox.toml` completely.
- Migrate the current repository's local `kairox.toml` file, if present, to `./.kairox/config.toml` without changing its contents.
- Update documentation and examples so new users create `.kairox/config.toml` instead of `kairox.toml`.
- Keep the existing TOML schema, profile loading, MCP loading, and built-in defaults unchanged.

## Non-goals

- No compatibility shim or deprecation fallback for `./kairox.toml`.
- No change to profile, context, MCP, marketplace, or API key resolution schemas.
- No GUI or TUI IPC surface changes.
- No generated TypeScript type changes.

## Current behavior

`crates/agent-config/src/discovery.rs` currently discovers configuration in this order:

1. `./kairox.toml` as `ConfigSource::ProjectFile`
2. `~/.kairox/config.toml` as `ConfigSource::UserFile`
3. `None`, allowing the existing caller path to use built-in defaults

The root `kairox.toml.example` and `docs/dev/local-development.md` also tell users to copy the example file to `kairox.toml`.

## Proposed behavior

`find_config()` will discover configuration in this order:

1. `./.kairox/config.toml` as `ConfigSource::ProjectFile`
2. `~/.kairox/config.toml` as `ConfigSource::UserFile`
3. `None`, preserving the existing built-in defaults behavior

If `./kairox.toml` exists but `./.kairox/config.toml` does not, discovery returns `None` unless the user-level file exists. This makes the path migration explicit and avoids accidental use of stale local configuration.

## File changes

### `crates/agent-config/src/discovery.rs`

- Replace the old project filename constant with project-local directory and filename constants.
- Look up `current_dir().join(".kairox").join("config.toml")` before the user-level path.
- Keep `ConfigSource::ProjectFile` for the project-local path so downstream behavior remains unchanged.
- Add deterministic unit tests by isolating `current_dir` and `HOME`/user config behavior in temporary directories.

### `kairox.toml.example`

- Update the header instructions from copying to `kairox.toml` to copying to `.kairox/config.toml`.
- Update the documented discovery order.
- Keep the example TOML body unchanged.

### `docs/dev/local-development.md`

- Update local setup commands to create `.kairox/` and copy the example to `.kairox/config.toml`.
- Update nearby prose to tell users to edit `.kairox/config.toml`.

### `.gitignore`

- Ignore `/.kairox/config.toml` as a private local project config.
- Do not ignore the whole `.kairox/` directory, because workspace-level assets such as `.kairox/skills` may need to be versioned.
- Keep existing `kairox.toml` ignore behavior only if it is useful for preventing accidental commits of legacy local files; it does not imply runtime support.

### Local config migration

- If the current worktree contains `kairox.toml`, create `.kairox/` and move that file to `.kairox/config.toml`.
- Preserve file contents byte-for-byte.
- If `.kairox/config.toml` already exists, do not overwrite it blindly; inspect and resolve before moving.

## Testing strategy

Add or replace tests around `find_config()` so discovery behavior is deterministic:

- Project-local `.kairox/config.toml` wins over `~/.kairox/config.toml`.
- User-level `~/.kairox/config.toml` is used when project-local config is absent.
- Legacy `./kairox.toml` alone is ignored.
- Missing project-local and user-level configs return `None`.

Run the focused test first:

```bash
cargo test -p agent-config discovery
```

Then run at least the full `agent-config` crate test suite:

```bash
cargo test -p agent-config
```

## Risks and mitigations

- **Breaking existing local setups:** Users with only `./kairox.toml` must move it to `./.kairox/config.toml`. This is intentional based on the requested direct replacement.
- **Confusion from stale `kairox.toml`:** Documentation will no longer reference the old path, and tests will assert that the old path is ignored.
- **Accidentally ignoring versioned workspace assets:** `.gitignore` will target only `.kairox/config.toml`, not the whole `.kairox/` directory.

## Acceptance criteria

- `find_config()` returns `ConfigSource::ProjectFile` for `./.kairox/config.toml`.
- `find_config()` returns `ConfigSource::UserFile` for `~/.kairox/config.toml` when no project-local config exists.
- `find_config()` ignores `./kairox.toml`.
- `kairox.toml.example` and `docs/dev/local-development.md` document `.kairox/config.toml`.
- Private project-local config at `/.kairox/config.toml` is ignored by Git.
- The current local `kairox.toml`, if present, is migrated to `.kairox/config.toml` without content changes.
- Focused and crate-level `agent-config` tests pass.
