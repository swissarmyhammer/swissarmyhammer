---
position_column: done
position_ordinal: df80
title: Transplant `detect projects` op to code_context MCP tool
---
## What

The `detect projects` operation was deleted in commit `4521f0ca` when the treesitter MCP tool was removed. It was never transplanted to code_context. Agents calling `{"op": "detect projects"}` get an error, so they never discover project-type-specific guidelines (like using `cargo nextest run` for Rust).

## Source

Recover the old implementation from git: `git show 4521f0ca^:swissarmyhammer-tools/src/mcp/tools/treesitter/detect/mod.rs`

## Implementation

Port the old `detect/mod.rs` into the code_context tool:

1. Add `detect` submodule under `swissarmyhammer-tools/src/mcp/tools/code_context/` (or inline)
2. Define `DetectProjects` struct implementing `Operation` (verb: `detect`, noun: `projects`)
3. Parameters: `path` (optional), `max_depth` (optional, default 3), `include_guidelines` (optional, default true)
4. Add to `CODE_CONTEXT_OPERATIONS` vec and `Lazy` static
5. Add `\"detect projects\"` match arm in `CodeContextTool::execute()`
6. Port `execute_detect()`: resolve workspace path via `open_workspace()`, call `detect_projects()`, format as markdown with guidelines
7. Port `guidelines_for_type()` — update the match to include `ProjectType::Php` (added since old code). Create `builtin/_partials/project-types/php.md` or use a fallback for types without guidelines
8. Fix `include_str!` paths for the new file location
9. Update `description.md` to document the new op
10. Update schema test assertions (op count is currently hardcoded)
11. Update error message strings listing valid operations

## Dependencies (all still exist)

- `swissarmyhammer_project_detection::detect_projects` in `swissarmyhammer-project-detection/src/detect.rs`
- `ProjectType`, `DetectedProject`, `WorkspaceInfo` in `swissarmyhammer-project-detection/src/types.rs`
- Guideline partials in `builtin/_partials/project-types/*.md` (10 files, need `include_str!`)

## Acceptance Criteria

- `{"op": "detect projects"}` on code_context returns detected project types with guidelines
- Rust projects include nextest instructions in the output
- All existing code_context tests still pass
- New tests for detect projects op

## Tests

- Test detect projects returns Rust for this workspace
- Test include_guidelines=false omits guideline text
- Test max_depth parameter is respected
- `cargo nextest run --workspace` #detect-projects