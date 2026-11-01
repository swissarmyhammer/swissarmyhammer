---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe780
project: ai-panel
title: All tools must resolve CWD from the session working directory (the board dir), never std::env::current_dir(); grep must honor .gitignore
---
## P0 — root cause of "a simple grep hung forever" in the AI panel

(Original description and prior review findings retained below.)

## Resolution of 2026-06-04 18:35 review findings (claude-code)

All four findings addressed.

- [x] **Warning — glob filesystem-root guard.** Factored a shared `reject_filesystem_root(&Path)` helper into `crates/swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs` and applied it in both grep (`grep/mod.rs`) and glob (`glob/mod.rs`) right after the search dir is resolved.
- [x] **Warning — `session_root()` `.` fallback defeats the guard.** The shared helper also rejects any non-absolute root (bare relative `.`/empty), so the last-resort fallback can never silently root a walk at the process CWD (`Path::new(".").parent()` is `Some("")`). Updated the `session_root()` docstring in `tool_registry.rs` to state the guard catches this.
- [x] **Nit — read double-validation.** Removed the first `FilePathValidator::validate_path` in `read/mod.rs`; the request path now flows straight into `SecureFileAccess::read`, which validates once. Removed the now-unused import.
- [x] **Nit — guard message wording.** The shared helper emits a distinct, accurate message for the filesystem-root case vs. the unresolved-relative case.

### Tests
- 4 new unit tests for `reject_filesystem_root` (root / `.` / empty rejected, normal dir accepted).
- 2 new integration tests in `session_root_resolution.rs`: `grep_unscoped_refuses_filesystem_root`, `glob_unscoped_refuses_filesystem_root`.
- file unit suite 178 passed, shell 173 passed, integration session_root_resolution 5 passed, `cargo clippy -p swissarmyhammer-tools --lib --tests` clean (zero warnings).

---

### Original task

### Binding requirement (per user, previously specified)
**The working directory of a board IS the working directory of its agent session.** Every tool MUST operate rooted at that **session working directory**. The app process CWD is irrelevant (and is `/` for the bundled GUI app).

### Required regardless of approach
- Ban `std::env::current_dir()` in tool handlers — lint/test guard; resolve root from the session working dir.
- grep must honor `.gitignore` — `ignore::WalkBuilder` (respects `.gitignore`/`.ignore`, skips hidden + `target`/`.git`).
- Apply the same root-resolution fix to all file tools: `files`, `glob_files`, `read_file`, shell.
- Defensive guard: refuse / hard-anchor any unscoped search that would resolve to `/`.

## Review Findings (2026-06-04 18:35)

Approach A (thread `work_dir` through `ToolContext`) implemented and wired: `server.rs:656` sets `tool_context.working_dir`; all six file/shell handlers resolve root via `ToolContext::session_root()`. grep uses `ignore::WalkBuilder`. No blockers. Warnings + nits above — now all resolved.