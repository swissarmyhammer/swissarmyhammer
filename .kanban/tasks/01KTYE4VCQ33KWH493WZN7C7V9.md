---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvzdbc8mq4xx12tta3wne4am
  text: |-
    Picked up. Research complete (verified current code 2026-06-25).

    ROOT CAUSE confirmed: EntityWatcher (swissarmyhammer-entity/src/watcher.rs) routes .kanban/perspectives/<id>.yaml through parse_entity_path -> handle_file_event -> EntityCache::refresh_from_disk_with("perspective", ...), which fails with UnknownEntityType (no perspective entity def). Crucially, perspectives do NOT live in EntityCache at all — they live in a SEPARATE Arc<RwLock<PerspectiveContext>> (KanbanContext::perspectives). So even teaching the fields registry about "perspective" would invalidate the wrong store. Option 1 rejected: distorts fields registry AND touches the wrong cache.

    KEY DISCOVERY: PerspectiveContext already has reload_from_disk_with(id, prov) (perspectives/src/context.rs) that re-reads disk for one id, upserts/evicts the in-memory cache, AND broadcasts PerspectiveEvent::PerspectiveChanged/Deleted on its existing event bus. That bus is already wired to the frontend: state.rs subscribes perspective_rx -> notify_fanin maps PerspectiveEvent -> notifications/store/changed (store:"perspective") -> frontend PerspectiveProvider refetches perspective.list -> tab bar re-renders. So if the watcher drives reload_from_disk_with, the store-event-loop is satisfied with ZERO new frontend wiring.

    CONFIRMED via explore agent: external/watcher-driven perspective changes currently produce NO store/changed (only command-dispatch + undo/redo do). This is the gap.

    APPROACH CHOSEN: Option 2 (dedicated watcher route to the PerspectiveContext). Dependency note: entity is Tier 3, perspectives Tier 2, so entity MAY depend on perspectives per ARCHITECTURE.md tiers, but the generic EntityWatcher shouldn't be hard-coupled to perspectives. Plan: EntityWatcher::start gains an optional perspective-reload sink (callback) that the kanban crate's start_watcher supplies (it owns both the watcher start and the Arc<RwLock<PerspectiveContext>>). Watcher detects .kanban/perspectives/*.yaml and invokes the sink -> reload_from_disk_with -> existing broadcast. Reuses all existing machinery; least invasive correct change.

    Next: write failing cross-context staleness test FIRST (TDD RED).
  timestamp: 2026-06-25T12:51:17.908978+00:00
- actor: claude-code
  id: 01kvze13xmd30t5xt9gy434sna
  text: |-
    Implemented (TDD RED->GREEN).

    RED: tests/perspective_watcher_cross_context.rs failed — ctx_b perspective.list stayed stale ["Before","Default"] / ["Doomed","Default"] for 10s. Right reason: watcher dropped perspective events.

    GREEN approach (Option 2 — dedicated watcher route to PerspectiveContext):
    - swissarmyhammer-entity/src/watcher.rs: added pub trait PerspectiveReloader { async fn reload(&self, id) } and EntityWatcher::start_with(root, cache, Option<Arc<dyn PerspectiveReloader>>). start() now delegates with None (all 4 existing entity-test callers untouched). Event loop intercepts entity_type=="perspective" and calls reloader.reload(id) instead of routing to EntityCache (which has no perspective type -> would error). Exported via lib.rs.
    - swissarmyhammer-kanban/src/context.rs: added PerspectiveFileReloader (impl PerspectiveReloader) wrapping Arc<RwLock<PerspectiveContext>>; reload() calls reload_from_disk_with(id, EventProvenance::watcher()). start_watcher() builds the reloader from the already-initialized perspectives cell and passes it to start_with. Production ordering verified: wire_store_substrate() (state.rs:445) initializes the perspective cell before ctx.start_watcher() (state.rs:467), so the reloader is wired live.
    - ALSO canonicalize the watch root in start_watcher (std::fs::canonicalize, fallback to raw). Was the actual blocker: macOS FSEvents reports /private/var/... while TempDir/symlinked roots are /var/..., so strip_prefix dropped EVERY event (perspective AND entity). This fix benefits all entity types too. With it the watcher fires in <1s.
    - ensure_default.rs module doc updated: removed the stale "entity watcher rejects perspective files" claim, documented the new reload route.

    STORE-EVENT-LOOP: no new frontend wiring needed. reload_from_disk_with already broadcasts PerspectiveEvent on the existing bus; notify_fanin (spawn_notification_fanin) forwards EVERY PerspectiveEvent unconditionally (no origin filter) -> notifications/store/changed (store:"perspective") -> frontend PerspectiveProvider refetches perspective.list -> tab bar re-renders. Added test external_change_broadcasts_on_perspective_bus asserting the watcher reload emits PerspectiveChanged with origin=="watcher" on B's bus — proves the loop end to end. No frontend files touched, so no tsc/vitest needed.

    VERIFY: cargo nextest -p swissarmyhammer-entity (310 passed) and -p swissarmyhammer-kanban (1418 passed) = 1728/1728 green. cargo fmt clean. cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-entity --tests: zero warnings on touched files (only pre-existing swissarmyhammer-focus warnings remain, untouched).

    Next: really-done adversarial double-check, then move to review.
  timestamp: 2026-06-25T13:03:10.260008+00:00
- actor: wballard
  id: 01kvzezd6cge7rn5zwj3t8369r
  text: |-
    Review resolution — reviewer certified CORRECT, 0 blockers. All 5 load-bearing concerns independently verified GREEN: (1) the canonicalization fix is correct with NO sleeper regression — `start_watcher` canonicalizes a LOCAL `watch_root` used for both `watcher.watch()` AND the `strip_prefix` base in `parse_entity_path`/`parse_attachment_path`, so OS `/private/var` paths and the prefix base are both canonical and match; `self.root` stays un-canonicalized everywhere else; `unwrap_or_else` fallback = no panic; only caller is `KanbanContext::start_watcher`. This fixes the FSEvents drop for ALL entity types, not just perspectives. (2) `PerspectiveReloader` trait keeps entity crate generic (Cargo.toml unchanged, abstract trait). (3) Real-pipeline cross-context test drives two real contexts through the REAL opt-in FSEvents watcher (rename+delete on disk → `ListPerspectives` converges without re-open); RED→GREEN genuine, not fixture-only. (4) Store-event-loop verified end-to-end via `external_change_broadcasts_on_perspective_bus` (PerspectiveChanged origin=watcher → store/changed → tab-bar refetch). (5) Conventions clean; stale `ensure_default.rs` "unknown entity type" claim corrected.

    7 warnings + 7 nits WAIVED/deferred as non-blocking quality polish (engine over-produces these; none affect correctness): doc-sharpening on the trait/`start_with`; deep-nesting/length extraction in `watcher.rs::start_with`; `start_watcher` `Ok(false)` overloading two cases (minor API-clarity); repeated `"board"` literal; magic numbers (channel cap 256, poll 2s, debounce 50ms, test timeouts) → named constants. None are defects.

    Verified state holds: entity 310 pass, kanban 1418 pass, fmt clean, clippy clean on touched files. Moving to done.
  timestamp: 2026-06-25T13:19:42.796243+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe980
title: 'Perspective cache never refreshes from disk — entity watcher rejects perspective files ("unknown entity type: perspective"), so cross-window rename/delete see stale perspective.list'
---
## Symptom

`perspective.list` (and therefore the perspective tab bar) goes stale when another window or sibling process renames or deletes a perspective. Each process loads its `PerspectiveContext` once and never re-reads `.kanban/perspectives/` — an external rename/delete is invisible until the board is re-opened. Only the default-CREATE path is convergence-safe today (deterministic `default-<scope>` ids from card 01KTY6T1GPY94VYWANE9X41SKJ make duplicate creates upsert the same file); UPDATE/RENAME/DELETE still operate on stale in-memory state.

## Log evidence

The entity watcher (`crates/swissarmyhammer-entity/src/watcher.rs`) parses `.kanban/perspectives/<id>.yaml` into entity type `perspective` and calls `EntityCache::refresh_from_disk_with("perspective", ...)`, which fails with the `EntityError::UnknownEntityType` message:

```
unknown entity type: perspective
```

(`crates/swissarmyhammer-entity/src/error.rs` — `#[error("unknown entity type: {entity_type}")]`). The fields/entity registry has no `perspective` entity definition, so `EntityContext::entity_def("perspective")` errors and the watcher event is dropped — no cache invalidation ever fires for perspective files. Observable in the unified log: `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' | grep 'unknown entity type: perspective'`.

## Routed around, not fixed

Card 01KTY6T1GPY94VYWANE9X41SKJ deliberately routed AROUND this gap instead of fixing it: deterministic ensure ids + board-open reconciliation make stale-cache duplicate CREATEs harmless, and the module doc of `crates/swissarmyhammer-kanban/src/perspective/ensure_default.rs` explicitly records "the entity watcher rejects perspective files with \"unknown entity type\"". The underlying staleness for list/rename/delete remains.

## What a fix needs

- The entity watcher pipeline must learn the `perspective` entity type so cache invalidation fires for `.kanban/perspectives/*.yaml`: either register a `perspective` entity definition (so `EntityCache::refresh_from_disk_with` resolves it) or add a dedicated watcher route that reloads/evicts the `PerspectiveContext` (`KanbanContext::perspectives`) on file events — the invalidation must reach the `Arc<RwLock<PerspectiveContext>>` that `perspective.list`/rename/delete read.
- Alternative (acceptable fallback): reload-on-read — `perspective_context()` revalidates against disk (mtime/dir-generation check) before serving.
- A real-pipeline test: process A renames/deletes a perspective on disk; process B's (or a second context's) `perspective.list` reflects it without re-opening the board.
- Frontend store must receive the resulting change notification so the tab bar re-renders (store-event-loop rule).

## Constraints

- Crate-scoped builds/tests only (`-p swissarmyhammer-kanban`, `-p swissarmyhammer-entity`).
- Use tracing, never eprintln. #ui

## Review Findings (2026-06-25 07:08)

Verdict: no blockers. Load-bearing correctness concerns all verified GREEN (see below). Remaining items are clarity warnings and naming nits from the engine — none block the fix.

### Load-bearing correctness — verified (in-scope, all pass)
- [x] CANONICALIZATION FIX correct & not a sleeper regression — `start_watcher` canonicalizes a *local* `watch_root` (`std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone())`) and passes it straight into `EntityWatcher::start_with`. `self.root` is left un-canonicalized for all other uses; the canonical value is never stored or compared elsewhere. The watcher uses the same canonical `root_clone` for both `watcher.watch()` AND the `strip_prefix` base in `parse_entity_path`/`parse_attachment_path`, so OS-reported (`/private/var/…`) paths and the prefix base are both canonical and match — fixing the `/private/var` vs `/var` FSEvents drop for ALL entity types, not just perspectives. No panic: `unwrap_or_else` falls back to the raw root if the dir does not exist. Only caller of `EntityWatcher::start*` with a watch root is `KanbanContext::start_watcher`; the app-side `start_watcher`/`start_watchers` are unrelated plugin/AppState wrappers that do not compare an un-canonicalized root. Test evidence the watcher now delivers events generally: `watcher.rs::test_attachment_create_emits_event` / `test_attachment_remove_emits_event` (canonicalized root, real FSEvents).
- [x] `PerspectiveReloader` trait keeps the entity crate GENERIC — `crates/swissarmyhammer-entity/Cargo.toml` unchanged (`git diff --stat HEAD` empty); trait is abstract, exported via `lib.rs` (`pub use watcher::{EntityWatcher, PerspectiveReloader}`). Kanban adapter `PerspectiveFileReloader` (context.rs) wraps `Arc<RwLock<PerspectiveContext>>` and is wired from the already-initialized `self.perspectives` cell inside `start_watcher` — correct.
- [x] Real-pipeline test is genuine, not fixture-only — `tests/perspective_watcher_cross_context.rs`: real `InitBoard`, two real `KanbanContext::open`, a sibling context renames AND deletes a perspective on disk, and the watched context's real `ListPerspectives` command converges WITHOUT re-opening. Watcher is opt-in and started explicitly (`assert!(ctx_b.start_watcher().unwrap())`), driven through the real FSEvents pipeline (poll-with-retrigger), not a direct `reload_from_disk_with` poke. RED→GREEN documented in implementer comment (stale `["Before","Default"]` for 10s before fix).
- [x] STORE-EVENT-LOOP end-to-end — `external_change_broadcasts_on_perspective_bus` subscribes to B's perspective bus BEFORE starting the watcher and asserts the watcher reload broadcasts `PerspectiveChanged` with `origin == "watcher"` — the same bus `notify_fanin` fans into `notifications/store/changed` (store: "perspective") so the frontend tab bar refetches. No frontend files touched; no regression.
- [x] Conventions — tracing (not eprintln!) throughout; opt-in watcher started explicitly in tests; `ensure_default.rs` module doc corrected (stale "unknown entity type" claim removed, new reload route documented).

### Warnings (engine — clarity, non-blocking)
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — `PerspectiveReloader` trait lacks a top-level doc comment in the engine's view (NOTE: a trait doc IS present at lines 20-35; treat as low-confidence / likely refuted). Only the `reload` method is individually re-flagged.
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — `start_with` has deep nesting (nested for/while/if-let, ~6 levels) in the `tokio::spawn` event loop. Extract a `dedupe_events(...)` helper (collect+dedup) and a `route_event(...)` helper (perspective vs entity vs attachment routing) to flatten control flow.
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — the inline `tokio::spawn` event-handling closure is ~79 lines (over the 50-line threshold). Extract it into a named async `event_loop(...)` for testability/readability.
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — `start_with` is a key public entry point but its doc could more explicitly contrast it with `start` and explain the `perspective_reloader` parameter (a doc is present; engine wants it sharper).
- [ ] `crates/swissarmyhammer-kanban/src/context.rs` — `start_watcher` returns `Result<bool>` where `Ok(false)` conflates "cache not ready" with "watcher start failed". Consider `Result<()>` + `entity_watcher.get().is_some()` for idempotency, or an explicit `enum WatcherStatus { Started, AlreadyRunning }`.
- [ ] `crates/swissarmyhammer-kanban/src/perspective/ensure_default.rs` — the string literal `"board"` appears 5+ times (the `recover_zero_state` view kind plus test fixtures). Name it once: `const BOARD_VIEW_KIND: &str = "board";` and reuse.

### Nits (engine — magic numbers, non-blocking)
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — MPSC channel capacity `256` → named const `ENTITY_WATCHER_CHANNEL_CAPACITY`.
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — poll interval `Duration::from_secs(2)` → named const `ENTITY_WATCHER_POLL_INTERVAL_SECS`.
- [ ] `crates/swissarmyhammer-entity/src/watcher.rs` — debounce window `Duration::from_millis(50)` → named const `ENTITY_WATCHER_DEBOUNCE_MILLIS`.
- [ ] `crates/swissarmyhammer-kanban/tests/perspective_watcher_cross_context.rs` — test deadline `10s` (appears twice) → named const `TEST_DEADLINE_SECS`.
- [ ] `crates/swissarmyhammer-kanban/tests/perspective_watcher_cross_context.rs` — poll sleep `150ms` → named const `TEST_POLL_INTERVAL_MILLIS`.
- [ ] `crates/swissarmyhammer-kanban/tests/perspective_watcher_cross_context.rs` — recv timeout `200ms` → named const `TEST_RECV_TIMEOUT_MILLIS`.

### Out-of-scope / pre-existing (disregard for this task)
- `kanban-app::ai_panel_e2e` (GPU-gated) — pre-existing, not touched by this diff.
- `swissarmyhammer-plugin` `file_notes_e2e` / `example_layering_e2e` (CWD-isolation) — pre-existing.
- pre-existing clippy in `swissarmyhammer-focus` — untouched by this task.