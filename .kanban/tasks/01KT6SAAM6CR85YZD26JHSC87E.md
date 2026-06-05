---
assignees:
- claude-code
depends_on:
- 01KT6R6HR3KJT6JVNDRAJV8V4T
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffeb80
project: short-ids
title: 'Short IDs: tool/CLI API accepts short id as input + emits it in output'
---
Make the short id usable from the kanban tool/MCP and CLI surfaces. Input is forgiving (short or full ULID); storage stays canonical (full ULID); output exposes the short id.

## Scope
- Input — id args: anywhere the kanban operations accept a task id (get/move/complete/assign/unassign/tag/untag/update/delete/archive), also accept the 7-char short id or a `^<short>` form, resolved via the core resolver to the full ULID. Case-insensitive. Full ULID continues to work unchanged.
  - Insertion point: the id-coercion in `crates/swissarmyhammer-kanban/src/dispatch.rs` (`TaskId::from_string(...)` call sites) — route task-id args through the resolver instead of a raw `from_string`.
- Input — structured ref fields: `depends_on` (and any future task-ref field) accepts short ids on write, but NORMALIZES them to the full ULID before storing. The stored value is always the full 26-char ULID (see Storage policy on the core card).
- Output: include a derived `short_id` field on the task JSON returned by the tool, and render `^<short>` in any human-formatted CLI output (kanban-cli). Stored JSONL is unchanged.

## Acceptance
- A task can be fetched/moved/completed by `^8rfp1r`, by `8rfp1r`, and by full ULID; case-insensitive.
- `add task`/`update task` with `depends_on` given as short ids persists full ULIDs in the JSONL.
- Tool task JSON includes `short_id`.
- An unknown/ambiguous short id returns a clean not-found error, not a panic.

Depends on core derivation/resolver.

## Review Findings (2026-06-05 10:42)

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/dispatch.rs` (`board_task_ids`) — Its docstring claims "Reads through the entity cache, so this is cheap on the hot path," but only the live `ectx.list("task")` half is cache-backed. The `ectx.list_archived("task")` half (`swissarmyhammer-entity/src/context.rs` `list_archived`) does a fresh `io::read_entity_dir` disk scan AND, when a compute engine is attached, runs `apply_compute_with_query` (per-archived-task changelog derivation) on every call — there is no cache. Because `req_task_id`/`resolve_task_ref` now run `board_task_ids` on every id-coercing op (get/move/complete/delete/assign/unassign/tag/untag/update/archive/unarchive, every attachment op, and every `depends_on` entry), this archived rescan happens unconditionally — even when the caller already passed a canonical full 26-char ULID that needs no board lookup. Impact is bounded (archive counts are usually small, ops aren't tight-looped) so this is a warning, not a blocker. Suggested fixes: (a) short-circuit `resolve_task_ref` to return a full 26-char ULID input directly when it matches the canonical form, skipping the board scan entirely; and/or (b) correct the docstring so it doesn't overstate the archived path as cache-cheap. If the existence check on `get`/`delete` is wanted, note the underlying command already enforces it, so the resolver scan for an exact full ULID is largely redundant.
  - RESOLVED: Added `canonical_full_ulid(raw)` helper that recognizes a canonical full 26-char ULID (optional `^` sigil, any case, validated via `ulid::Ulid::from_string`) and returns its canonical uppercase form. `resolve_task_ref` now short-circuits on it before touching the board, so canonical full ULIDs skip both the live and archived scans entirely (fix (a)). Also rewrote the `board_task_ids` docstring to state plainly that the archived half is an uncached disk scan and is only cheap when the archive is small, and to point callers holding a full ULID at the short-circuit (fix (b)). New unit tests: `resolve_task_ref_short_circuits_canonical_full_ulid` (an absent canonical ULID now resolves to itself, proving the scan is skipped) and `resolve_task_ref_short_circuit_normalizes_case_and_caret` (lowercase and `^`-prefixed forms normalize to canonical uppercase). `cargo test -p swissarmyhammer-kanban --lib` → 1206 passed, 0 failed; `cargo clippy --lib --all-targets -- -D warnings` clean.

### Nits
- [x] `crates/swissarmyhammer-kanban/src/dispatch.rs` (`resolve_depends_on`) — Non-string `depends_on` array entries are silently skipped (`if let Some(s) = v.as_str()`), unlike the date params which were deliberately made strict to surface a clear error on a type mismatch. This matches the pre-existing `main` behavior (the old `filter_map(|v| v.as_str())` also dropped them), so it is not a regression and not in scope to fix here — flagging only for consistency with the stricter date-param handling added alongside it.
  - ACKNOWLEDGED: No code change — the finding itself states this is not a regression and out of scope (it matches pre-existing `main` behavior). Left as-is intentionally; recorded here so the deliberate skip-on-non-string behavior is documented rather than silently re-litigated.