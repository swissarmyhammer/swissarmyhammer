---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa580
title: Perspectives gone missing — default perspective creation raced into duplicates, then none; ensure-default must be idempotent with recovery
---
## What

LIVE BUG (user-observed): perspectives have gone missing entirely. Expected invariant: **there is always a default perspective**. Observed history: duplicate defaults were being created earlier, and now there are NONE.

This smells like a non-idempotent ensure-default racing (multiple windows / board opens each creating "Default" → duplicates) followed by some dedup/cleanup/delete path that removed all of them — or a validation/load change that now rejects the perspective files entirely (cf. the views-crate degenerate-def skip added in card 01KTCRY5W2BP7TYTHV4JB9CH8K — if a similar skip/validation got applied to perspectives, corrupted/duplicate perspective files may now be silently skipped on load, presenting as "none").

## Forensics FIRST (do not guess)

1. **On-disk state**: inspect `.kanban/perspectives/` in this repo's board (NOTE: the working tree currently has 5 UNTRACKED perspective file pairs from today — `01KTXMMDH20DQYNFG2PRCSR9WQ`, `01KTXMSXHXHK34S2X96BK8R1BH`, `01KTXMSZVCRZ22XYDSFENJK215`, `01KTXMVDCDGXC388GRFJ76JDHH`, `01KTXMW3X17JKMHY1KH4HTZH14` — these may BE the duplicate defaults). Read them: are they duplicates of a default? Valid or degenerate? Compare with the committed perspective files (git ls-files .kanban/perspectives/).
2. **The unified log**: `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 2h | grep -iE 'perspectiv|skip|degenerate|duplicate|default'` — find creation/skip/delete events.
3. **Code paths**: where is the default perspective ensured (board open? perspectives context build? frontend PerspectivesContainer?) — find the creation trigger and why it can run more than once (per-window? per-webview-mount? StrictMode double-effect like the layer push/pop race fixed in 01KTQCHWP5T4GS8SPGYVXD2CT9?). Where are perspectives loaded/listed (swissarmyhammer-kanban perspectives module; `list perspectives` op) and is there any validation/skip that could now reject all of them? Any dedup/cleanup that deletes?

## Required outcome

- **Idempotent ensure-default**: exactly one default perspective exists after any number of board opens / window opens / concurrent mounts. Creation must be guarded at the STORAGE layer (check-then-create under the board lock or keyed by a stable id — e.g. the default perspective gets a deterministic well-known id so a second create is a no-op upsert), not by frontend politeness.
- **Recovery**: a board with zero perspectives gets its default (re)created on open/load — the user must never see an empty perspective bar.
- **Dedup with data preservation**: if duplicates exist (as on this board now), converge to one default without losing any non-default user perspectives; document what happens to the extra defaults (merge/remove).
- Fix the current board's state as part of verification (the app should self-heal it via the recovery path — not by hand-editing files).

## Acceptance Criteria
- [x] Fresh board open → exactly one default perspective
- [x] Two windows opening the same board concurrently → still exactly one default (no duplicates)
- [x] Board with zero perspectives (current state) → default recreated automatically on open; perspective bar renders
- [x] Board with N duplicate defaults → converges to one; user-created perspectives untouched
- [x] Root cause documented with log/file evidence: what created the duplicates, what removed all of them (review-verified: 231 deleted YAMLs at HEAD all vanilla `name: Default`; minting cause documented in the `ensure_default.rs` module doc)

## Tests
- [x] Rust test: ensure-default is idempotent (call twice → one perspective; concurrent calls → one)
- [x] Rust test: load with zero perspectives → default created; load with duplicate defaults → converges to one, others preserved/merged per the documented semantics
- [x] Real-pipeline test (per fixture-only-anti-pattern): drive through the actual board-open path, not raw inserts
- [x] `cargo nextest run -p swissarmyhammer-kanban` green (full crate: 1291/1291 after review fixes — was 1277 at card creation, 1287 at review)

## Constraints
- NO whole-workspace cargo build/clippy; crate-scoped only. Frontend changes (if any) scoped vitest + tsc.
- Use tracing (never eprintln); read the unified log yourself.
- Do NOT touch .kanban/actors/wballard.jsonl. The 5 untracked perspective files are EVIDENCE — read them before any cleanup; the fix should make the app converge them, not hand-delete them.

## Workflow
- Use `/tdd` — failing test first (idempotence + zero-recovery), then fix.

## Review Findings (2026-06-12 12:19)

Verified: 1287/1287 crate tests green (fresh run); red-green probe re-confirmed (reconcile call disabled in `KanbanContext::open` → 5/6 recovery integration tests fail, restored → 6/6 pass; working tree restored to the implementer's exact diff). Board audit: all 231 deleted perspective YAMLs at HEAD were vanilla `name: Default` with zero customization (no filter/group/sort/fields, no non-Default names) — the self-heal on this board lost nothing user-authored. Current `.kanban/perspectives/`: 11 files = 7 user perspectives intact + 4 scoped Defaults. Views are loaded before reconciliation in `open` (ordering safe), `prune` bails when the view registry is empty, and `is_customized` covers every user-editable knob on `Perspective`. Frontend `useAutoCreateDefaultPerspective` is unchanged but now routes `if_absent` through the storage-layer ensure with deterministic ids, making its re-fires harmless (no frontend diff → no vitest/tsc needed).

### Blockers
- [x] `crates/swissarmyhammer-kanban/src/perspective/ensure_default.rs:129` — `dedup_defaults_per_scope` deletes ALL non-keeper duplicates including CUSTOMIZED ones: when two or more customized Defaults share a scope, the losers' filter/group/sort/fields are silently destroyed at board open. This directly contradicts the module doc ("customized or user-named perspectives are never deleted") and the stated semantics — and the multi-customized state is exactly what the production duplicate-minting bug could breed (user sets a filter on the Default tab in window A while window B's stale cache mints and the user customizes another). Fix: in the deletion loop, skip customized duplicates — only vanilla duplicates are deletable; if multiple customized Defaults remain, keep them all for the user to resolve. Update the module doc and add a unit test for the two-customized-defaults case. (This board's heal was audited clean — all 231 deletions vanilla — so no recovery action needed here, only the code/doc fix.) DONE: deletion loop now filters to vanilla duplicates only; module doc + pass doc updated; red-green unit test `dedup_preserves_all_customized_defaults_sharing_one_scope` added (red: customized 01BBB deleted, left:1 right:2 → green after fix).

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/perspective/add.rs:154` — the ensure path embeds the caller-supplied `view_id` verbatim into the on-disk filename (`default-<view_id>.yaml` via `PerspectiveContext::write` → `root.join(format!("{id}.yaml"))`) with no validation that the view exists and no character sanitization. An ensure against a dead view id re-mints a default pinned to a nonexistent view (the exact forensic shape: 19 Defaults pinned to dead view id "default"), which the next open then prunes — a create/prune churn loop; and a `view_id` containing path separators escapes the perspectives dir. Validate the `view_id` against the views registry in ensure mode (fall back to kind scope when not found). DONE: ensure mode now validates `view_id` against the views registry (fall back to kind scope when not found); with no registry wired, the new `is_safe_scope_component` filename guard (modeled on `is_safe_entity_type`: rejects path separators, `..`, leading `.`, empty, bounds length at 128) is the backstop. Red-green via three real-pipeline tests in `tests/perspective_default_recovery.rs`: `ensure_save_with_dead_view_id_falls_back_to_kind_scope` (red: default pinned to "no-such-view"), `ensure_save_with_path_separator_view_id_cannot_escape_perspectives_dir` (red: IO error os 2 — raw view_id reached the filename), `ensure_save_with_overlong_view_id_falls_back_to_kind_scope` (red: IO error os 63 "File name too long").
- [x] Stale perspective cache is routed around, not fixed: the entity watcher still rejects perspective files (`unknown entity type: perspective`), so `perspective.list`/rename/delete from another window/process still operate on stale in-memory state — only the default-CREATE path was made convergence-safe. Acceptable scope for this card, but no follow-up card exists on the board. Create one for perspective cache refresh (watcher support or reload-on-read). DONE: follow-up card filed — `01KTYE4VCQ33KWH493WZN7C7V9` (symptom, log evidence, routed-around note, and fix requirements: entity watcher must learn the perspective entity type so cache invalidation reaches the `PerspectiveContext`).

### Nits
- [x] `crates/swissarmyhammer-kanban/src/perspective/ensure_default.rs:174` — the "fully shadowed" legacy prune is all-or-nothing: a partially shadowed legacy kind-shared Default survives and renders as a duplicate "Default" tab in every view that also has a pinned Default. This board exhibits it now: legacy grid Default `01KNF7T1EF6Z8HQGT3YZ908DF7` coexists with pinned grid Defaults for PGRID0/TGRID0. Consider per-view shadowing or document the residual duplicate tab. DONE: documented the residual duplicate tab on `prune_unreachable_defaults` — shadowing is deliberately all-or-nothing because a legacy kind-shared default is ONE file serving every view of its kind (cannot be deleted "per view"; deleting it while partially shadowed would strip the Default tab from the still-unpinned views); the residual resolves when every view of the kind gains a pinned default or the user deletes the legacy perspective.
- [x] Task checkboxes (Acceptance Criteria / Tests) are all still unchecked despite the work being done and verified — flip them honestly when picking up these findings. DONE: all AC/Tests boxes flipped above (each reviewer-verified, full crate re-verified at 1291/1291 after these review fixes).