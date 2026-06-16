---
assignees:
- claude-code
position_column: todo
position_ordinal: a780
project: diagnostics
title: diagnostics MCP tool (check working/file/sha, list/get servers)
---
## What
The pull side: an operation tool mirroring the `review` tool's structure. New MCP tool `diagnostics` in `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/`.

- Define ops as `Operation` impls with `ParamMeta` static arrays, modeled on `review/mod.rs` (`ReviewFile`/`ReviewWorking`/`ReviewSha`, `REVIEW_OPERATIONS`, `generate_mcp_schema`, op→handler match in `execute`):
  - `check working` — files changed vs HEAD (reuse the git scoping review uses via `ToolContext::git_ops`).
  - `check file` — explicit path or glob.
  - `check sha` — files touched in/since a commit or range.
  - `list servers` / `get server` — read the LSP supervisor (`LspSupervisorManager::status` / `DaemonStatus`), no analysis.
- Shared modifiers as params: `severity?`, `settle_ms?`, `dependents?`.
- **`allowed_values` infra does NOT exist yet — this card owns adding it.** Verified: `review/mod.rs` has no enum param and no `allowed_values`; `ParamMeta`/`ParamType` in `crates/swissarmyhammer-operations/src/parameter.rs` have only String/Integer/Number/Boolean/Array — no enum/values. The design doc explicitly wants `severity` to be "the enum param that wants `allowed_values`." So add `allowed_values: Option<Vec<&'static str>>` (or equivalent) to `ParamMeta`, a corresponding `ParamType` enum variant or field, and emit it into the JSON Schema in `generate_mcp_schema`. **Blast radius:** `swissarmyhammer-operations` is shared by every op tool — keep the change additive/back-compat (default `None`); run `cargo build --workspace` as the regression guard. (If this proves larger than expected, fall back to documenting severity values in the param description and split the `allowed_values` work into its own task — but prefer owning it here.)
- Each `check` calls `swissarmyhammer_diagnostics::diagnose(paths, config)` and returns a `DiagnosticsReport { diagnostics, counts }` serialized like review's `ReviewResponse` (markdown + counts).
- Register via `register_diagnostics_tools(registry)` + a `_with_factories` variant if a live session handle must be injected, mirroring `register_review_tool_with_factories`.

## Depends on
- "diagnose(paths) core API with capped broken-dependents" (9fq036d)

## Acceptance Criteria
- [ ] `ParamMeta`/`ParamType`/`generate_mcp_schema` in `swissarmyhammer-operations` support `allowed_values`, additively (existing op tools unaffected; `cargo build --workspace` clean).
- [ ] `diagnostics` MCP tool registered with ops `check working`/`check file`/`check sha`/`list servers`/`get server` and modifiers `severity?` (with `allowed_values`)/`settle_ms?`/`dependents?`.
- [ ] `check working`/`check sha` scope via git like `review`; `check file` accepts a path or glob.
- [ ] Returns `DiagnosticsReport { diagnostics, counts }`; flows through the same op dispatch/schema/grammar as other op tools (no bespoke schema).

## Tests
- [ ] `cargo test -p swissarmyhammer-operations`: schema-generation test asserts a param with `allowed_values` emits a JSON Schema `enum`, and a param without it is unchanged (back-compat).
- [ ] `cargo test -p swissarmyhammer-tools`: op-dispatch tests for each op string (mirroring review's dispatch tests); `check file` on a fixture (gated on rust-analyzer) returns the expected report; `list servers` returns supervisor status with no analysis.

## Workflow
- Use `/tdd`. Copy the review tool's structure; add `allowed_values` to the shared param infra first (with its back-compat test), then build the tool. #diagnostics