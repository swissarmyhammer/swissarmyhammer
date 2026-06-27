---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvtwmttp4twwj4r8f4pm105h
  text: Picked up by /finish (single-task). No prior comments. Starting implement → test → review loop.
  timestamp: 2026-06-23T18:42:24.214132+00:00
- actor: claude-code
  id: 01kvtwt7ygqp7k7mgcahwvt6ss
  text: |-
    Implemented per task spec (TDD). RED first: added two failing tests in context.rs (test_ensure_directories_self_heals_gitignore_entries, test_ensure_directories_reconciles_gitignore_when_dirs_present) — confirmed both failed with "missing required gitignore entry undo_stack.yaml" while 3 existing ensure_directories tests passed. GREEN: made init::ensure_gitignore_entries and REQUIRED_GITIGNORE_ENTRIES pub(crate), made `mod init` pub(crate) in board/mod.rs, and added an unconditional crate::board::init::ensure_gitignore_entries(self.root()).map_err(KanbanError::Io)? call to KanbanContext::ensure_directories (outside the directories_exist guard). Left InitBoard::execute call in place (redundant/harmless).

    BLOCKER (transient): a parallel shell is mid-scaffolding a new workspace member crates/swissarmyhammer-hashline (added to root Cargo.toml members + crc32fast workspace dep, but src/lib.rs not yet present), so `cargo metadata` / nextest fails to load the workspace manifest ("no targets specified in the manifest"). Not related to this change. Retrying once the parallel crate stabilizes.
  timestamp: 2026-06-23T18:45:21.488074+00:00
- actor: claude-code
  id: 01kvtx1t71c72vb12w3cm3zk8c
  text: |-
    VERIFIED (after parallel-shell workspace stabilized). Forced rebuild resolved a stale-binary false-fail. Results:
    - cargo nextest run -p swissarmyhammer-kanban ensure_directories: 4 passed (self_heals, reconciles_when_dirs_present, is_idempotent, recreates_missing).
    - cargo nextest run -p swissarmyhammer-kanban ensure_gitignore: 1 passed (ensure_gitignore_entries_is_idempotent).
    - Full crate suite: 1537/1537 passed, 0 failures.
    - cargo clippy -p swissarmyhammer-kanban --all-targets: clean, no warnings.
    - Adversarial double-check: PASS, no findings (verified dir-existence edge case is safe — create_directories runs create_dir_all(root) before the gitignore write; error maps to KanbanError::Io; idempotent trim-compare append; no blanket .kanban/ ignore; pub(crate) visibility correct, external pub re-exports of InitBoard/register_merge_drivers unaffected). Moving to review.
  timestamp: 2026-06-23T18:49:29.569915+00:00
- actor: claude-code
  id: 01kvtycd7jbr3kqxjqnsw3x9t9
  text: 'Review verdict: findings appended. Only 1 of 3 is in-scope for this task (init.rs:17 — REQUIRED_GITIGNORE_ENTRIES doc comment). The other two (HashlineError Error/Display in crates/swissarmyhammer-hashline) are from an unrelated parallel-shell crate scaffold that leaked into the `review working` scope — NOT this task''s edits, leaving them untouched. Implement loop addressing the doc-comment nit only.'
  timestamp: 2026-06-23T19:12:45.298181+00:00
- actor: claude-code
  id: 01kvtyfgtjq7p1pgv70126n680
  text: |-
    Addressed the in-scope review nit: added a doc comment to REQUIRED_GITIGNORE_ENTRIES in crates/swissarmyhammer-kanban/src/board/init.rs ("Ephemeral gitignore entries (search cache, undo state) that must not be version controlled..."). Verified: `cargo nextest run -p swissarmyhammer-kanban ensure_gitignore` = 1 passed; `cargo clippy -p swissarmyhammer-kanban` = exit 0, no warnings. Checked off that finding box.

    The two HashlineError findings (crates/swissarmyhammer-hashline/src/lib.rs:67 — missing std::error::Error and Display) are OUT OF SCOPE: that crate was scaffolded by a parallel shell and leaked into this review run. Left those two boxes unchecked for separate tracking; did NOT touch the hashline crate.
  timestamp: 2026-06-23T19:14:27.282302+00:00
- actor: claude-code
  id: 01kvtz27tfxwwakqkzsfsmf1g7
  text: 'Addressed the 2026-06-23 13:15 review nits in init.rs (no hashline changes). The intended doc comment for ensure_gitignore_entries had drifted up above REQUIRED_GITIGNORE_ENTRIES, leaving the function undocumented and the const double-documented. Moved that doc block back to sit directly above `pub(crate) fn ensure_gitignore_entries`, rewording it to describe reconciling .kanban/.gitignore so all REQUIRED_GITIGNORE_ENTRIES are present, appending missing ones idempotently without clobbering existing lines, rewriting only on change, and returning the io::Error on failure. REQUIRED_GITIGNORE_ENTRIES already had its own doc comment — checked that box off as already-satisfied (stale re-flag). Verified: `cargo nextest run -p swissarmyhammer-kanban ensure_gitignore` → 1 passed; `cargo clippy -p swissarmyhammer-kanban` → clean (exit 0). Task stays in review.'
  timestamp: 2026-06-23T19:24:40.655033+00:00
- actor: claude-code
  id: 01kvtz9wxyszz1zmjn9x1cc5d8
  text: 'Heads-up: an implementation of this task landed unexpectedly during the /finish $file-edit-tools batch (an implementer subagent did out-of-scope work). It''s in local commit 8c48cc703: `ensure_gitignore_entries` is now called from `KanbanContext::ensure_directories` (context.rs), made `pub(crate)` in board/init.rs, plus a `test_ensure_directories_self_heals_gitignore_entries` test. This matches this card''s spec. NOT driven through /review yet and this task is outside the file-edit-tools scope — needs a proper /test + /review pass before being marked done. Verify the change is complete/correct rather than assuming.'
  timestamp: 2026-06-23T19:28:51.646453+00:00
- actor: claude-code
  id: 01kvtzzf1bp3x0g1ht2zkpht2m
  text: 'DONE via /finish. Final state: review clean (0 blockers/warnings/nits against in-scope swissarmyhammer-kanban files), tests green (1537/1537, clippy clean). 3 review iterations — each surfaced a distinct doc-comment nit (REQUIRED_GITIGNORE_ENTRIES, then ensure_gitignore_entries), now resolved. Source landed in HEAD 8c48cc703; local rollback-point commit 1d3c0d971 captures the self-healed .kanban/.gitignore + task state. NOT pushed (user''s step). Out-of-scope hashline Error/Display findings left untouched.'
  timestamp: 2026-06-23T19:40:38.315896+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd380
title: Kanban .gitignore self-heals search-cache entries on board open, not just first init
---
## What
The kanban search cache (`.kanban/search-cache.sqlite3` + `-wal`/`-shm` sidecars) is supposed to be gitignored, but on existing boards it is not — it shows up as untracked. Example: this repo's `.kanban/.gitignore` contains only `mcp.log`, so `git status` lists `.kanban/search-cache.sqlite3` as `??`.

**Root cause:** `ensure_gitignore_entries` (in `crates/swissarmyhammer-kanban/src/board/init.rs:311`) writes the required entries (`REQUIRED_GITIGNORE_ENTRIES` at init.rs:304 — `undo_stack.yaml`, `search-cache.sqlite3`, `search-cache.sqlite3-wal`, `search-cache.sqlite3-shm`), but it is only called from `InitBoard::execute` (init.rs:379). That `execute` **early-returns `AlreadyExists`** when `ctx.is_initialized()` (init.rs:367-371), so the gitignore step never runs on an already-initialized board. The function's doc comment claims \"existing boards gain any newly-required entries on the next init/open\" (init.rs:377-378) — but nothing runs it on open. Boards created before the search-cache entries existed (or whose `.gitignore` was written with only `mcp.log`) never self-heal.

**Fix:** run the gitignore reconciliation on the board-open path so existing boards self-heal, idempotently.
- In `crates/swissarmyhammer-kanban/src/board/init.rs`, change `fn ensure_gitignore_entries` to `pub(crate) fn ensure_gitignore_entries` (and `REQUIRED_GITIGNORE_ENTRIES` to `pub(crate)` if needed for the test) so it can be called from `context.rs`.
- In `crates/swissarmyhammer-kanban/src/context.rs`, call `ensure_gitignore_entries(self.root())` from `KanbanContext::ensure_directories` (context.rs:368) **unconditionally — outside the `if !self.directories_exist()` guard** (context.rs:369), so it runs even when directories already exist. It is invoked by the operation processor on every op (`processor.rs:40`), so any kanban operation against an existing board will reconcile the `.gitignore`. The function already only rewrites when something changed, so this stays cheap/idempotent. Map the `io::Error` to `KanbanError::Io`.
- Keep the existing `InitBoard::execute` call (harmless/redundant) or remove it since `ensure_directories` now covers it — implementer's choice; do not change behavior on fresh init.

## Acceptance Criteria
- [ ] `KanbanContext::ensure_directories` reconciles `.kanban/.gitignore` on every call, including when the board is already initialized and directories already exist.
- [ ] After running any kanban operation against a pre-existing board whose `.gitignore` lacks them, the `.gitignore` contains `search-cache.sqlite3`, `search-cache.sqlite3-wal`, `search-cache.sqlite3-shm`, and `undo_stack.yaml` (and pre-existing lines like `mcp.log` are preserved, not clobbered).
- [ ] The reconciliation does not blanket-ignore `.kanban/`: committed task `.md`/`.jsonl` files stay tracked.
- [ ] Reconciliation is idempotent: a second `ensure_directories` does not duplicate entries or rewrite an already-complete `.gitignore`.

## Tests
- [ ] Add a unit test in `crates/swissarmyhammer-kanban/src/context.rs` (tests module, alongside `test_ensure_directories_is_idempotent` at ~context.rs:862): create a `KanbanContext` on a temp dir, pre-write `.kanban/.gitignore` containing only `mcp.log`, call `ensure_directories()`, and assert the file now also contains the four `REQUIRED_GITIGNORE_ENTRIES` while still containing `mcp.log`. Then call `ensure_directories()` again and assert no entry is duplicated.
- [ ] Add a test asserting `ensure_directories()` on an already-initialized board (directories present) still writes the missing search-cache entries (guards against re-introducing the `directories_exist()` early-return regression).
- [ ] `cargo test -p swissarmyhammer-kanban ensure_directories` and `cargo test -p swissarmyhammer-kanban ensure_gitignore` are green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-06-23 12:51)

> ⚠️ 1/15 review tasks failed — results are INCOMPLETE.

### Warnings
- [ ] `crates/swissarmyhammer-hashline/src/lib.rs:67` — Public error type `HashlineError` does not implement `std::error::Error` trait. Library error enums must implement Error so callers can use them in error chains, convert to trait objects, and integrate with error handling frameworks. This violates the library error handling requirement. Implement `std::error::Error` for `HashlineError`. Use the `thiserror` crate (add as a dependency) and derive the Error trait, or manually implement it with a Display impl. The Mismatch variant should expose source() for the contained op.

### Nits
- [ ] `crates/swissarmyhammer-hashline/src/lib.rs:67` — Public error type `HashlineError` lacks `Display` implementation. Error types should provide user-friendly Display messages (lowercase, no trailing punctuation) distinct from Debug. Implement Display for HashlineError, e.g., Mismatch → 'anchor resolution failed — content may have drifted'. The Display message should guide users to re-anchor.
- [x] `crates/swissarmyhammer-kanban/src/board/init.rs:17` — `REQUIRED_GITIGNORE_ENTRIES` constant lacks a doc comment. Crate-public constants should document their purpose. Add a doc comment: `/// Ephemeral gitignore entries (search cache, undo state) that must not be version controlled.`.

## Review Findings (2026-06-23 13:15)

### Nits
- [x] `crates/swissarmyhammer-kanban/src/board/init.rs:21` — Public function `ensure_gitignore_entries` lacks a documentation comment explaining its purpose, parameters, and return value. FIXED: the intended doc block had drifted up to sit above `REQUIRED_GITIGNORE_ENTRIES`; moved it back to sit directly above `pub(crate) fn ensure_gitignore_entries`, describing that it reconciles `.kanban/.gitignore` so all `REQUIRED_GITIGNORE_ENTRIES` are present, appending missing ones idempotently without clobbering existing lines, rewriting only on change, and returning the `std::io::Error` on failure.
- [x] `crates/swissarmyhammer-kanban/src/board/init.rs:36` — Public constant `REQUIRED_GITIGNORE_ENTRIES` lacks a documentation comment. ALREADY SATISFIED: the constant already carries a doc comment in current source (\"Ephemeral gitignore entries (search cache, undo state) that must not be version controlled. Listed explicitly … so the guarantee is exact and the directory's tracked task files stay un-ignored. Declared at module scope … so the test suite can assert against the single source of truth\"). Stale re-flag — no change needed.