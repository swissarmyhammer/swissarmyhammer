---
assignees:
- claude-code
position_column: todo
position_ordinal: a780
project: diagnostics
title: diagnostics MCP tool (check working/file/sha, list/get servers)
---
## What
The pull side: an operation tool mirroring the `review` tool exactly. New MCP tool `diagnostics` in `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/`.

- Define ops as `Operation` impls with `ParamMeta` static arrays, modeled on `review/mod.rs` (`ReviewFile`/`ReviewWorking`/`ReviewSha`, `REVIEW_OPERATIONS`, `generate_mcp_schema`):
  - `check working` — files changed vs HEAD (everyday op; reuse the git scoping the review tool uses via `ToolContext::git_ops`).
  - `check file` — explicit path or glob.
  - `check sha` — files touched in/since a commit or range.
  - `list servers` / `get server` — read the LSP supervisor (`LspSupervisorManager::status` / `DaemonStatus`), no analysis.
- Shared modifiers as params: `severity?` (enum param with `allowed_values`, mirroring how review declares its enum), `settle_ms?`, `dependents?`.
- Each `check` calls `swissarmyhammer_diagnostics::diagnose(paths, config)` and returns a `DiagnosticsReport { diagnostics, counts }` serialized like review's `ReviewResponse` (markdown + counts).
- Register via `register_diagnostics_tools(registry)` + a `_with_factories` variant if a live session handle must be injected, mirroring `register_review_tool_with_factories`. Add to the tool registry wiring.

## Depends on
- "diagnose(paths) core API with capped broken-dependents"

## Acceptance Criteria
- [ ] `diagnostics` MCP tool registered with ops `check working`/`check file`/`check sha`/`list servers`/`get server` and modifiers `severity?`/`settle_ms?`/`dependents?` (severity exposes `allowed_values`).
- [ ] `check working`/`check sha` scope via git like `review`; `check file` accepts a path or glob.
- [ ] Returns `DiagnosticsReport { diagnostics, counts }`; flows through the same op dispatch/schema/grammar as other op tools (no bespoke schema).

## Tests
- [ ] `cargo test -p swissarmyhammer-tools`: op-dispatch tests for each op string (mirroring review's dispatch tests); schema-generation test asserts the severity enum `allowed_values`; `check file` on a fixture (gated on rust-analyzer) returns the expected report; `list servers` returns supervisor status with no analysis.

## Workflow
- Use `/tdd`. Copy the review tool's structure; do not invent a new op pattern. #diagnostics