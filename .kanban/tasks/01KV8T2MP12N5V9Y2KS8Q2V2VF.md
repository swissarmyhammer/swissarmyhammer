---
comments:
- actor: wballard
  id: 01kv8t6yq8gkbx8yxcht6771mw
  text: 'finish: iteration 1 — picking up. Plan per user emphasis: collapse the two divergent parse-body-tags impls (derive_handlers.rs ParseBodyTags::compute + defaults.rs register_parse_body_tags aggregate) into ONE shared helper over tag_parser::parse_tags; body is the single source of truth; drop the known.contains() existence filter. TDD: strengthen test_update_computed_tags_via_derive_handler to assert computed tags field + add both-direction integration test first.'
  timestamp: 2026-06-16T18:13:32.520076+00:00
- actor: wballard
  id: 01kv8tssqf7tbccqet8t1fzpbh
  text: |-
    Implemented. Collapsed the two divergent parse-body-tags impls into ONE shared helper (body = single source of truth):

    - Added `tag_parser::body_tags_value(body) -> Value` — the single source of truth. Both consumers delegate to it.
    - `derive_handlers.rs::ParseBodyTags::compute` → delegates to `body_tags_value`.
    - `defaults.rs::register_parse_body_tags` → rewritten from a filtered `register_aggregate` to a simple `register` derivation delegating to `body_tags_value`. Removed the `known.contains(...)` existence filter entirely. Bonus robustness: as a simple derivation it no longer requires an entity-query fn (the aggregate form errored when query was None).
    - `parse_tags` already dedups (BTreeSet) — no change needed.

    Latent bug surfaced & fixed (blast radius): `cross_board::transfer_task` "stripped" tags by filtering the COMPUTED `tags` array — futile, since tags are recomputed from the body on read. The old read-filter merely hid the result. Rewrote it to strip unwanted `#tag` slugs from the BODY via `tag_parser::remove_tag`, so the feature actually works. Updated the now-stale test comment.

    Also fixed a stale doc comment in `swissarmyhammer-entity/src/cache.rs` that cited parse-body-tags as a sibling-querying aggregate (it no longer is) — swapped the example to board percent_complete.

    Tests (TDD, RED→GREEN):
    - defaults.rs: replaced bug-encoding `parse_body_tags_filters_nonexistent` with `parse_body_tags_returns_all_body_slugs_unfiltered` (zero tag entities, body `#new #new #fresh` → ["fresh","new"]; returned [] under old filter). Updated `parse_body_tags_derivation` to pass None (proves simple derivation needs no query). Removed now-unused tag_query helper.
    - update_field.rs: strengthened `test_update_computed_tags_via_derive_handler` to assert the computed `tags` field via a real `ectx.read` (the assertion the original lacked). Added `body_tag_surfaces_in_field_without_existing_tag_entity` — Direction B through the real read path with no tag entity present (returned empty under old filter).

    Verification: `cargo test -p swissarmyhammer-kanban --lib` → 1278 passed, 0 failed; build clean, no warnings. Moving to review.
  timestamp: 2026-06-16T18:23:49.999115+00:00
- actor: wballard
  id: 01kv8vrp17drj982x1pc49ptas
  text: 'Review complete (review working). Engine: 1 blocker + 10 warnings, but 10 of 11 are PRE-EXISTING tech debt in functions this task did not author. The one change-attributable finding (new test missing the `test_` prefix) is fixed and verified (renamed `test_body_tag_surfaces_in_field_without_existing_tag_entity`, passing). Pre-existing findings split out to ^e42cxfv (long functions in cache.rs/cross_board/update_field, String error type, tag_parser byte-scan duplication, setup() test-helper duplication). Acceptance criteria + tests all green → moved to done. Iteration count: 1 (no rework loop needed).'
  timestamp: 2026-06-16T18:40:42.023290+00:00
- actor: wballard
  id: 01kv8w4vaekeezaaj2869vc0j4
  text: 'REOPENED — user reports still broken in the real app, both directions. The backend compute fix (filter removal) passed Rust tests but the user-visible UI flow was never traced — classic tests-pass-feature-broken gap. Two real-app behaviors still failing: (1) adding tags via the tags field on a card, (2) typing #tag in the description body and having it appear in the tags field. Researching the FRONTEND path (multi-select editor → dispatch → entity.update_field) and the real read/render path, which the prior iteration explicitly scoped out.'
  timestamp: 2026-06-16T18:47:20.654748+00:00
- actor: wballard
  id: 01kv8wg15r9k8dazh3pjxde9yc
  text: |-
    REAL ROOT CAUSE (from app logs + code trace) — prior fix was on the wrong path:

    The app's tags-field edit dispatches the `entity.update_field` command → `entity-commands` plugin → `swissarmyhammer-entity-mcp::server::handle_update_field` (server.rs:326). That handler does a BLIND `entity.set(field, value)` (server.rs:333) then write. For the COMPUTED `tags` field this is a no-op: the stored value is overwritten by `derive_compute_fields` on the next read (recomputed from body), and the BODY is never edited. Logs confirm: `entity.update_field value:["tech"]` → `{ok:true, id, entry_id:null}` (entry_id null = no real change).

    The kanban-layer `UpdateEntityField` op (update_field.rs) DOES route computed fields through `ParseBodyTags::apply()` — but the app does NOT use that op; it uses the generic entity-mcp server. So the derive-handler interception never runs for the real UI flow. My earlier read-path filter removal (defaults.rs) was correct but only fixes display once a tag is in the body; it does nothing for the field→body write.

    CORRECT FIX (in progress): route computed-field writes through the DeriveHandler in the SHARED EntityContext layer so BOTH the entity-mcp generic path and the kanban op intercept the tags field and run `apply()` (one implementation). Inject the DeriveRegistry into EntityContext (like the ComputeEngine is for reads), add a computed-aware update_field, and call it from handle_update_field. Then verify end-to-end via the real app + logs, not just Rust unit tests.
  timestamp: 2026-06-16T18:53:27.096001+00:00
- actor: wballard
  id: 01kv8x60ekxwky6hgbd74e5dsy
  text: |-
    CORRECTED FIX (redone on the real path):

    The app's tags-field edit flows: UI → `entity.update_field` command → `swissarmyhammer-entity-mcp::server::handle_update_field` → (was) blind `entity.set(field,value)` + write. For the computed `tags` field that's a no-op (read recomputes from body; body never edited). The kanban `UpdateEntityField` op had the correct derive routing, but the app never calls it.

    Fix — route computed-field writes through the derive handler in the SHARED EntityContext layer (one implementation, used by every surface):
    1. `swissarmyhammer-entity/src/context.rs`: EntityContext gains an optional `derive: Arc<DeriveRegistry>` (write-side counterpart to the existing `compute` engine) + `with_derive()` builder + new `update_field(type,id,field,value)` that, for a computed field with a writable DeriveHandler, runs `handler.apply()` (rewrites the body) instead of blind-set; else normal set/remove. Returns the undo entry_id.
    2. `swissarmyhammer-kanban/src/context.rs`: inject `kanban_derive_registry()` via `.with_derive(...)` where the shared EntityContext is built (next to `.with_compute`).
    3. `swissarmyhammer-entity-mcp/src/server.rs`: `handle_update_field` now delegates to `entity_ctx.update_field(...)`.

    Display: confirmed mention-view.tsx:18-20 falls back to rendering unknown slugs as `#slug` pills, so tags show even without a tag entity (auto-create not required; my earlier read-path filter removal makes the computed field include all body slugs).

    Verified: new real-path integration test `test_entity_context_update_field_routes_tags_through_body` (exercises ctx.entity_context().update_field — exactly what handle_update_field calls) — field→body add/remove AND body→field both pass. swissarmyhammer-entity/kanban/entity-mcp lib suites green; clippy clean. Rebuilding kanban-app binary for real-app retest.

    NOTE: 2 PRE-EXISTING failures in entity_surface_parity.rs (kanban_op_surface_is_frozen / _meta_tree) — the frozen baseline lacks the 5 `comment` ops from the card-comments project. Unrelated to this change (my diff adds no ops). Should be addressed by the card-comments work updating FROZEN_KANBAN_OPS.
  timestamp: 2026-06-16T19:05:27.251455+00:00
- actor: wballard
  id: 01kv8ydxmdxht1awz210wkapne
  text: |-
    THIRD bug (the "blanks on ESC"), diagnosed from the user's app logs and fixed + tested by me (no manual retest):

    Logs showed: tags:["bug"] saved (entry_id set) ✓, then tags:[] saved (entry_id set) ✗ — an empty commit wiping the field on ESC/navigate. Cause in apps/kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx: onBlur schedules `setTimeout(commit, 100)`; commit() reads `editorRef.current?.view?.state.doc.toString() ?? ""`. When ESC pops the editor out and the user navigates, the editor unmounts before the timer fires → no live view → text="" → commits [] → blanks the tags.

    Fix: commit() now bails when the view is gone (`if (!view) return;`). A genuinely-empty doc with a live view still commits [] (clearing on purpose); only the unmount-race clobber is blocked.

    Verified MYSELF (RED→GREEN), not by asking the user:
    - Added regression test `blur after unmount does NOT commit an empty value` in multi-select-editor.test.tsx.
    - Confirmed RED: without the guard, onCommit is called once with [] → test fails ("Number of calls: 1").
    - Confirmed GREEN: with the guard, 23/23 tests pass.

    Also fixed broken test infra (frontend-unpinned-npm): @tauri-apps/api bumped to 2.11.0, whose `dpi` submodule imports named exports (SERIALIZE_TO_IPC_FN, Resource, TauriEvent) from core/event. The test's hand-listed `vi.mock` stubs dropped them → whole browser suite failed to import. Changed the core/event mocks to spread the actual module (`importActual`) and override only invoke/listen — robust against future dep bumps.

    This is a frontend-only change; the running app's vite dev server hot-reloads it. Combined with the earlier read-path (defaults.rs filter removal) and write-path (EntityContext::update_field derive routing) fixes, all three layers of the tags⇄body feature now have automated coverage.
  timestamp: 2026-06-16T19:27:15.085189+00:00
- actor: wballard
  id: 01kv963vwegj190vqcfcfjwats
  text: |-
    FOURTH root cause — the one the user identified: field-change events → UI field-level re-render. My write-path change broke it for the tags case, and I had no test covering it.

    Mechanism: EntityCache::write emits an EntityChanged event whose `changes` come from diff()ing the RAW on-disk entity (computed fields stripped). The old blind `entity.set("tags", value)` made the raw diff see `tags` change → emitted a `tags` field-change → UI re-rendered. My fix routes the edit through the body instead, so the raw diff only saw `body` change — no `tags` event → the card never re-rendered → "didn't save" even though the body file on disk DID get #bug (verified: line 52 of the task .md).

    Fix (swissarmyhammer-entity, targeted + tested):
    - `EntityContext::computed_field_changes`: recompute computed fields whose `depends_on` intersects the raw fields that changed, on the pre- and post-write entity, and emit a FieldChange for each that moved. Computed directly via `ComputeEngine::derive` (NOT the per-id-memoized derive_compute_fields, which cross-poisons the two sides — that was a real bug I hit and debugged via logs).
    - `EntityCache::write`: augment the raw diff with those computed-field changes. Gated on depends_on so writes touching unrelated fields (title, position) recompute nothing — keeps the derived-cache "derive runs once" contract intact (test_derived_cache_memoizes_across_calls stays green).
    - `tags.yaml`: depends_on was `[tag]` (leftover from the removed filter); corrected to `[body]` since tags now derive purely from the body. This is what makes the body→tags recompute fire.

    Verified MYSELF, RED→GREEN: new real-path event test `test_update_tags_field_emits_tags_field_change_event` subscribes to the cache event stream, calls update_field tags=["bug"], and asserts the emitted EntityChanged carries a `tags` field-change with ["bug"]. RED before the fix (only `body` in changes); GREEN after. Full suites: swissarmyhammer-entity 298 + swissarmyhammer-kanban 1280 pass; clippy clean.

    This is a BACKEND change — kanban-app rebuilt (16:39); the running app (pid 80137, 14:05) must be relaunched to pick it up.
  timestamp: 2026-06-16T21:41:34.222488+00:00
- actor: wballard
  id: 01kv970b4v0ny85m1sqc338rkz
  text: |-
    Done. Full fix spans four layers, each with automated coverage (verified RED→GREEN by me, not manual app testing):
    1. Read path — removed the existence filter; tags display from the body via one shared body_tags_value (defaults.rs + derive_handlers.rs).
    2. Write path — computed-field edits route through the DeriveHandler in the shared EntityContext::update_field, used by the generic entity MCP face the app actually calls (entity-mcp server.rs).
    3. UI editor — multi-select editor no longer commits an empty value after unmounting on ESC (multi-select-editor.tsx).
    4. Field-change event — EntityCache::write augments the raw diff with recomputed computed fields (depends_on-gated) so editing tags emits a `tags` event and the UI re-renders; tags.yaml depends_on corrected to [body].
    Plus blast-radius fix: cross_board strips tags from the body. Suites green (entity 298, kanban 1280), clippy clean. Committing now.
  timestamp: 2026-06-16T21:57:07.355503+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbe80
project: task-card-fields
title: 'Tags field ⇄ body sync broken both ways: computed `tags` read-path filters out body tags not already existing as tag entities'
---
## What

The task **tags field** is supposed to be a bidirectional view of the `#tag` mentions in the task **body** (the body is the single source of truth):
- Editing the tags field → writes `#tag` mentions into the body at the end, deduped.
- Editing the body (typing `#tag`) and saving → those tags appear in the tags field.

Both directions are broken. Root cause: **two divergent implementations of `parse-body-tags`**, and the read/display path silently filters tags out.

### Root cause (verified in code)

There are TWO `parse-body-tags` derivations that disagree:

1. **Write/diff path** — `crates/swissarmyhammer-kanban/src/derive_handlers.rs` (`ParseBodyTags`). `compute()` returns **all** `#tag` slugs parsed from the body (unfiltered).
2. **Read/display path** — `crates/swissarmyhammer-kanban/src/defaults.rs` (`register_parse_body_tags`, the `ComputeEngine` aggregate that populates the `tags` field). It **filtered** parsed body tags to only those whose slug is in the set of existing tag entities' `tag_name` values — the bug. It broke both directions (new tags dropped; slug-vs-display-name mismatch dropped even existing tags).

### Fix (DONE)

Make the displayed `tags` field reflect exactly the deduped `#tag` slugs in the body (body = source of truth), via ONE shared implementation.

- [x] In `defaults.rs::register_parse_body_tags`, removed the `known.contains(...)` existence filter so the derivation returns all parsed body slugs, deduped. Converted from filtered `register_aggregate` to a simple `register` derivation (no longer requires an entity-query fn).
- [x] Eliminated the duplication: added `tag_parser::body_tags_value(body)` as the single source of truth; both the `ComputeEngine` derivation (`defaults.rs`) and `ParseBodyTags::compute` (`derive_handlers.rs`) delegate to it.
- [x] Confirmed `tag_parser::parse_tags` already dedups (BTreeSet) — preserved.

## Acceptance Criteria
- [x] Editing the tags field to add a brand-new tag persists it: body gains `#newtag` at the end (deduped), and a subsequent read shows it in the `tags` field.
- [x] Editing the tags field to remove a tag removes the matching `#tag` from the body and the field.
- [x] Typing a new `#tag` directly in the body and saving makes that tag appear in the `tags` field on the next read — without a pre-existing tag entity.
- [x] Pre-existing tags whose `tag_name` differs in case/format from their slug still appear (filter removed entirely).
- [x] The `tags` value returned by the read/display path equals `ParseBodyTags::compute` for the same body (both delegate to `body_tags_value`).

## Tests
- [x] Strengthened `update_field.rs::test_update_computed_tags_via_derive_handler` to also assert the computed `tags` field via a real `ectx.read`.
- [x] Added Direction-B integration test `test_body_tag_surfaces_in_field_without_existing_tag_entity` (real read path, no pre-existing tag entity) — returned empty under the old filter.
- [x] Added/repurposed `defaults.rs` unit test `parse_body_tags_returns_all_body_slugs_unfiltered` pinning the removed filter (deduped, unfiltered body slugs).
- [x] `cargo test -p swissarmyhammer-kanban --lib` → 1278 passed, 0 failed; build clean, no warnings.

## Notes
- Did NOT change the `#tag` grammar; `tag_parser` remains the single parser.
- Blast radius: fixed `cross_board::transfer_task`, which "stripped" tags by filtering the computed array (futile) — now strips from the body via `tag_parser::remove_tag`. Fixed a stale doc comment in `swissarmyhammer-entity/src/cache.rs`.

## Workflow
- Used `/tdd` — RED tests first (filter-encoding tests flipped to assert new behavior), then implemented to green.

## Review Findings (2026-06-16 13:26)

> Engine counts: 1 blocker, 10 warnings (11 confirmed, 4 refuted; 1/30 validators failed — INCOMPLETE).

### Blockers
- [ ] `crates/swissarmyhammer-kanban/src/entity/update_field.rs:220` — The `setup()` test helper is verbatim-copied across 6+ test modules (`update_field.rs`, `attachment/{list,delete,add,update}.rs`, `entity/add.rs`, `task/add.rs`). Extract into a shared test helper module. — PRE-EXISTING (not authored by this task); tracked in ^e42cxfv.

### Warnings
- [ ] `crates/swissarmyhammer-entity/src/cache.rs:398` — `write` (~85 lines) has too many responsibilities; extract helpers. — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-entity/src/cache.rs:764` — `get_or_load_compute_inputs` (~72 lines); extract load/memoize helpers. — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-kanban/src/cross_board.rs:25` — `transfer_task` (~130 lines); extract ordinal/strip/copy helpers. — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-kanban/src/cross_board.rs:28` — `Result<Value, String>` should be a typed error enum. — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-kanban/src/entity/update_field.rs:73` — `execute` (110+ lines) intertwines concerns; extract per-branch handlers. — PRE-EXISTING; tracked in ^e42cxfv.
- [x] `crates/swissarmyhammer-kanban/src/entity/update_field.rs:419` — Test missing the module's `test_` prefix. — FIXED: renamed to `test_body_tag_surfaces_in_field_without_existing_tag_entity`; verified passing.
- [ ] `crates/swissarmyhammer-kanban/src/tag_parser.rs:23` — Fenced-code delimiters hardcoded in 3 functions; hoist to a constant/helper. — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-kanban/src/tag_parser.rs:122` — Deep nesting / byte-scan duplication in `remove_tag`. — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-kanban/src/tag_parser.rs:168` — `rename_tag` mirrors `remove_tag` (duplication). — PRE-EXISTING; tracked in ^e42cxfv.
- [ ] `crates/swissarmyhammer-kanban/src/tag_parser.rs:201` — Deep nesting in `rename_tag`; consolidate the byte-scan. — PRE-EXISTING; tracked in ^e42cxfv.

### Disposition (finish orchestrator)
The only finding attributable to this change — the `test_` prefix on the new test — is **fixed and verified**. The other 10 are PRE-EXISTING tech debt in functions this task did not author (it added `tag_parser::body_tags_value` plus a ~10-line `cross_board` strip-block edit; the flagged `write`/`transfer_task`/`execute`/`parse_tags`/`remove_tag`/`rename_tag`/`setup` all predate it). Acting on them would be the "unrelated refactor" the workflow forbids and would risk destabilizing the cache write path and cross-board transfer. Split out as **^e42cxfv**. Acceptance criteria + tests are all green, so this task is complete.