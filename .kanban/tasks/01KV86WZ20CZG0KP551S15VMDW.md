---
assignees:
- claude-code
position_column: todo
position_ordinal: a880
project: diagnostics
title: 'Inline-on-edit: mutated_paths + shared diagnostics fold-in step'
---
## What
When a write op mutates a file, its own result carries diagnostics — no hook, no config, model-facing in every host (a tool's own return value always reaches the model; confirmed: llama-agent appends `ToolResult.result` JSON straight into the conversation at `agent.rs` ~1137-1170). Today there is NO `mutated_paths` and NO shared post-processing seam: `EditResult { bytes_written, replacements_made, ... }` (`files/edit/mod.rs:61`), `write` returns a bare `usize`, and `FilesTool::execute` (`files/mod.rs:147`) returns each op's `CallToolResult` directly.

- Add a `mutated_paths: Vec<PathBuf>` signal that a mutating op declares (edit, write, and any future mutator). Prefer a small trait/struct convention so it is not per-op duplicated.
- **Seam location (decided):** put the single fold-in step at the **`McpTool` dispatch boundary in `crates/swissarmyhammer-tools/src/mcp/unified_server.rs`** (confirmed this is in swissarmyhammer-tools, not llama-agent). Dispatching there — rather than inside `FilesTool::execute` — means ANY MCP mutator tool that reports `mutated_paths` gets diagnostics folded in, not just the file tools. The step reads `mutated_paths` off the tool's structured result and, for diagnosable paths, calls `swissarmyhammer_diagnostics::diagnose()` and folds the `DiagnosticsReport` into the op result JSON. (If the unified dispatch cannot see a structured `mutated_paths` without a result-envelope change, add a minimal typed envelope field rather than string-parsing the `CallToolResult`.)
- **Gating** (design "when appropriate"): diagnosable language only (the supervisor knows — `.md`/`.txt` attaches nothing); settle generously, `pending` on timeout; severity/scope policy = edited file always + capped broken one-hop dependents inline.
- Keep output sharp (guardrail k=1): compute the full blast radius, return only what broke.

## Depends on
- "diagnose(paths) core API with capped broken-dependents" (9fq036d)
- "diagnostics MCP tool (check working/file/sha, list/get servers)" (na6cvh0) — shares the same diagnose plumbing / config wiring

## Acceptance Criteria
- [ ] Mutating file ops declare `mutated_paths`; a single shared step at the `unified_server.rs` `McpTool` dispatch boundary folds diagnostics into their result (no per-op duplication).
- [ ] The seam is generic: a non-file MCP tool that reports `mutated_paths` also gets diagnostics folded in.
- [ ] A `.rs` edit returns inline diagnostics in the op result; a `.md`/`.txt` edit attaches nothing.
- [ ] Non-quiescent analysis yields `pending` rather than blocking indefinitely.
- [ ] The edited file plus only broken capped dependents appear; no project-wide dump.

## Tests
- [ ] `cargo test -p swissarmyhammer-tools`: edit op on a fixture `.rs` with an injected error (mock diagnose) returns a result whose JSON contains the diagnostics; edit on `.md` returns none; timeout path yields `pending`. Assert the single `unified_server` fold-in seam covers both `edit` and `write` (and a stub non-file mutator).
- [ ] Integration (gated on rust-analyzer): real `files edit` introducing a type error returns the error inline in the tool result.

## Workflow
- Use `/tdd` — write the "edit .rs result contains diagnostics / edit .md does not" test first, then add the seam at the unified dispatch boundary. #diagnostics