---
assignees:
- claude-code
depends_on:
- 01KTBNKCZ2JRRX514XWHPFB7V1
- 01KTBNM0YGVRJQJSCTQBDHR68H
- 01KTBNMJY54KG5K7BWG29C2J1J
- 01KTBNN3A9JNQ5VGD1JN16RCT8
position_column: todo
position_ordinal: 8d80
project: local-review
title: 'Operation-based `review` MCP tool: review file/working/sha + validator introspection'
---
## What
Expose the engine as an OPERATION-BASED MCP tool in `crates/swissarmyhammer-tools/src/mcp/tools/review/`, dispatched by an `op` field exactly like `git`, `kanban`, and `code_context` (single tool, verb-noun op-dispatch — NOT a tool-per-verb). Thin wrapper: parse op + args, build a scope, call the engine, return structured results.

**Engine entry point (implemented in this task, in `swissarmyhammer-validators::review`):** `run_review(scope, connection, opts) -> ReviewReport`. This is the pipeline driver — it owns the choreography: `scope_review` → create the `AgentPool` (sized from `opts` backend + `review.concurrency`) → submit fan-out tasks → run the inline probe guard on each returned finding → submit surviving verify tasks to the same pool → await the pool drain → `synthesize`. The MCP tool does NOT contain this logic; it only maps op→scope, resolves the connection + opts, calls `run_review`, and returns the report.

**`review` is the verb; the scope target is the noun.**

| Op | Args | Returns | Purpose |
|----|------|---------|---------|
| `review file` | `path` (file path **or** glob); `validators?[]` (subset); `backend?` (`session`\|`local`) | `ReviewReport { markdown, counts{blockers,warnings,nits,confirmed,refuted} }` | Review an explicit file/glob set (reviewed whole when there is no diff). |
| `review working` | `validators?[]`; `backend?` | `ReviewReport` | Review uncommitted changes vs HEAD. The everyday op. |
| `review sha` | `sha` (commit or range); `validators?[]`; `backend?` | `ReviewReport` | Review the changes in/since a commit or range. |
| `list validators` | `source?` (`builtin`\|`user`\|`project`\|`all`), `match?` (path/glob) | `[{ name, description, source_layer, match_globs, severity, probes, rule_count, path }]` | Introspect what's plugged in and from which precedence layer. |
| `get validator` | `name` | `{ name, frontmatter, source_layer, path, probes, rules:[{ name, severity, body }] }` | Read one validator's full rule bodies + probes. |
| `check validators` | — | `{ ok, errors:[{ path, problem }] }` | Lint all validators (frontmatter valid, globs compile, no stray `trigger`, declared probes exist in the catalog). Drives `sah doctor`. |

- `validators?[]` and `backend?` are shared modifiers on the three `review` ops.
- `list/get/check` are pure loader reads (no agent), fast.
- Register via the existing `register_*_tools()` → `ToolRegistry` pattern; mirror the git tool's op-dispatch module layout (`mod.rs` dispatch + per-op submodules) and request/response serde structs.
- Resolve the `ConnectionTo<Agent>` and CWD from the MCP session/work-dir, NOT `std::env::current_dir()` (session-cwd-for-tools convention).
- The tool does NOT write to kanban — the skill does.

## Acceptance Criteria
- [ ] Engine `run_review(scope, connection, opts)` exists as the pipeline driver (scope → pool → fan-out → guard → verify → drain → synthesize); the tool is a dispatch shim that calls it.
- [ ] The op-dispatched `review` tool is registered with `review file`/`review working`/`review sha` + `list/get/check validators` (MVP: `review working` + `list validators`).
- [ ] `backend` and `validators` modifiers honored; op-dispatch/registration/structs mirror the git tool; no `install`/`dimensions`.
- [ ] Connection + CWD resolved from the session/work-dir, never `current_dir()`.

## Tests
- [ ] Real-pipeline integration test (reference pattern `tests/integration/semantic_search_e2e.rs`): temp git repo with a planted duplicate + a planted dead function. Drive `review working`, `review sha <range>`, and `review file <glob>` with a scripted/playback agent; assert each returns a report flagging the issues tagged to the right validator/severity.
- [ ] `list validators` returns the builtin set with correct source layers (temp `XDG_DATA_HOME/validators` + temp project `./.validators` each adding one validator → all three layers appear) and includes `probes`; `check validators` errors on a malformed fixture validator.
- [ ] Tool registration test (the ops appear in the registry); `cargo test -p swissarmyhammer-tools review` green.

## Workflow
- Use `/tdd` — write the registered-tool integration tests first (real registry, real engine, scripted agent), then implement `run_review` + the thin dispatch. Reuse the git tool's module structure as the template. Real-path tests, not mock-boundary unit tests.