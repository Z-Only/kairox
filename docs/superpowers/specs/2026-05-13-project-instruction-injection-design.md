# Project Instruction Injection into Agent Context

**Date**: 2026-05-13
**Status**: Approved
**Branch**: feat/instruction-context

## Summary

The agent loop currently discovers project instruction files (AGENTS.md, CLAUDE.md, README.md, etc.) but only records their paths. File contents are never read and never injected into the agent context. This feature reads the actual content of all existing instruction files and injects it into the context via a new `ContextSource::ProjectInstruction` section.

## Requirements

- Read all existing files from `INSTRUCTION_FILE_PRIORITY` list
- Merge content with `### Instructions from {filename}` headers
- Inject as a distinct context source with independent budget control
- Droppable under budget pressure as a last resort (after Memory, before Skill)

## Architecture

### Crates touched

| Crate           | Change                                                                                                      |
| --------------- | ----------------------------------------------------------------------------------------------------------- |
| `agent-core`    | Add `ContextSource::ProjectInstruction`; extend `ProjectInstructionSummary` with `contents: Option<String>` |
| `agent-memory`  | Add `project_instructions` to `ContextRequest`; new section in `assemble()`; update drop order              |
| `agent-runtime` | Read file contents in `read_project_instruction_summary()`; wire into agent loop; update facade             |
| `agent-gui`     | Regenerate types via `just gen-types`                                                                       |

### New priority & drop order

```
P0    System               (never dropped)
P0.25 ProjectInstruction   (dropped after Memory, before Skill)
P0.5  Skill                (before ToolDefinitions in drop)
P0.75 ToolDefinitions      (after Skill in drop)
P1    Request              (never dropped)
P2    Memory
P3    History
P4    ToolResult
P5    SelectedFile         (dropped first)
```

Drop order: SelectedFile → ToolResult → History → Memory → ProjectInstruction → ToolDefinitions → Skill

## Data Flow

```
Agent loop start
  → read_project_instruction_summary(root_path)
    → For each file in INSTRUCTION_FILE_PRIORITY:
        if exists → read content → append with "### Instructions from {filename}" header
    → return ProjectInstructionSummary { source_paths, contents: Some(merged), warning }
  → ContextRequest { project_instructions: merged_content, ... }
  → ContextAssembler::assemble()
    → [System] → [ProjectInstruction] → [Skill] → [ToolDefs] → [Request] → ...
  → ContextBundle { messages, sources, usage }
```

## File-by-File Changes

### 1. `agent-core/src/context_types.rs` — Add variant

```rust
pub enum ContextSource {
    System,
    ProjectInstruction,  // new
    ToolDefinitions,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
    CompactionSummary,
    Skill,
}
```

### 2. `agent-core/src/facade.rs` — Extend type

```rust
pub struct ProjectInstructionSummary {
    pub source_paths: Vec<String>,
    pub contents: Option<String>,        // NEW
    pub warning: Option<String>,
}
```

### 3. `agent-memory/src/context.rs` — Assembler integration

- `ContextRequest`: add `pub project_instructions: Option<String>`
- `assemble()`: emit `<project-instructions>` section after System, tagged with `ContextSource::ProjectInstruction`
- `ContextUsage`: add `project_instruction_tokens: u64`
- `find_lowest_priority_drop()`: add `ProjectInstruction` case between `Memory` and `ToolDefinitions`
- Support `source_caps` for `ProjectInstruction`

### 4. `agent-runtime/src/project.rs` — Read contents

- `read_project_instruction_summary()` now reads file contents via `tokio::fs::read_to_string()`
- Each file's content prefixed with `### Instructions from {filename}\n\n`
- Per-file limit: 64KB, truncated with `[...truncated]` marker if exceeded
- Read errors logged to `warning`, file skipped

### 5. `agent-runtime/src/agent_loop/runner.rs` — Wire in

- Call `read_project_instruction_summary(root_path)` before context assembly
- Pass `contents` as `project_instructions` in `ContextRequest`

### 6. `agent-runtime/src/facade_projects.rs` — Update impl

- `get_project_instruction_summary()` returns updated type with contents

## Error Handling

| Scenario                      | Behavior                                 |
| ----------------------------- | ---------------------------------------- |
| File exists but can't be read | Log warning, skip file, continue         |
| Empty file                    | Include header with empty body           |
| Non-UTF8 file                 | `read_to_string` error → warning, skip   |
| File > 64KB                   | Read first 64KB, append `[...truncated]` |
| No files found                | `contents` = `None`, no section emitted  |

## Edge Cases

| Case                            | Result                                   |
| ------------------------------- | ---------------------------------------- |
| Only README.md exists           | Single file content under its header     |
| All 7 priority files exist      | All merged in priority order             |
| Instruction section exceeds cap | Dropped per priority order (last resort) |

## Test Plan

| Scope                          | Test                                                                               |
| ------------------------------ | ---------------------------------------------------------------------------------- |
| `agent-runtime/src/project.rs` | Merged content from multiple files; empty when no files; header format; truncation |
| `agent-memory/src/context.rs`  | ProjectInstruction appears in output; correct drop priority; budget enforcement    |
| Integration                    | Full loop with instruction files present in workspace                              |

## Files NOT Changed

- `agent-config/` — existing discovery sufficient
- `agent-tools/` — no tool changes
- `agent-store/` — no schema changes
- `apps/agent-gui/src/generated/` — regenerated, not hand-edited
