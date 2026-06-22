---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvqstv5h4fm1r2ppx3tbjjfy
  text: |-
    Picked up. Implemented merge (read-modify-write) semantics for `set view`, per the contract decision.

    Contract: on `set view` against an EXISTING view, omitted optional fields preserve their on-disk value; only provided fields overwrite. Against a NON-EXISTENT view (or omitted id → fresh ULID), the view is created fresh (unchanged behavior). Optional fields are tri-state: omitted → preserve; explicit empty/null → clear; value → set.

    Files changed (all in crates/swissarmyhammer-views):
    - src/operations.rs: SetView field types changed to merge-friendly shape. name: Option<String>, kind: Option<String>, icon/entity_type: Option<Option<String>> (with a new `double_option` deserialize_with so an absent field is None=preserve while explicit null is Some(None)=clear — plain serde collapses both to None), card_fields: Option<Vec<String>> (absent=preserve, [] = clear). Tool description notes the partial-merge.
    - src/server.rs: handle_set_view now reads existing view (get_by_id) and routes through a new merge_set_view() free fn. Existing branch preserves omitted fields incl. `commands` (which has no wire surface). Create branch defaults omitted optionals to empty/none. handle_set_view takes `mut req` + req.id.take() to avoid a partial-move.
    - tests/integration/views_e2e.rs: 3 regression tests — preserves_omitted_fields, explicit_empty_clears, creates_fresh_when_absent.

    TDD: wrote the 3 tests first, watched them fail (icon stripped to null on omit), then implemented. A serde probe revealed plain Option<Option<String>> + #[serde(default)] collapses explicit null to None, which is why the double_option deserializer was needed for genuine tri-state clearing.

    Verification (run in an isolated CARGO_TARGET_DIR to avoid rust-analyzer flycheck races on the shared target/, which were serving stale test binaries):
    - cargo nextest run -p swissarmyhammer-views: 83 passed, 0 failed.
    - cargo clippy -p swissarmyhammer-views --all-targets -- -D warnings: clean.
    - cargo fmt -p swissarmyhammer-views --check: clean.

    Blast radius: no Rust code constructs SetView directly (deserialization-only change). rdeps (kanban, command-service, plugin, entity-mcp) all `cargo check --all-targets` clean. The only workspace check failure is a pre-existing, unrelated error in apps/kanban-app/src/command_services.rs (missing CommandMetadata.applies_to field) — that file is unmodified by this change and references swissarmyhammer-commands, not views.
  timestamp: 2026-06-22T13:55:32.145297+00:00
- actor: claude-code
  id: 01kvqt387462krgq23z9t857nk
  text: |-
    double-check returned REVISE with one actionable finding (medium): `commands` preservation was asserted in merge_set_view but no test exercised it — tautological, since every `set view`-created view starts with empty commands. The contract explicitly requires commands preservation, so this was a real coverage gap.

    Fixed: added regression test `set_view_partial_update_preserves_commands` in views_e2e.rs. It seeds a ViewDef carrying a command directly through the views kernel (h.views.write().await.write_view) — bypassing the wire, which has no commands field, mirroring builtin YAML views — then issues a partial `set view` (name only) and asserts the command survives. TDD-verified: temporarily reverting `commands: existing.commands` to `Vec::new()` makes this test fail ("commands must survive a partial update"), confirming it is non-tautological.

    double-check Finding 2 (low/optional) — the Option<Option<String>> JsonSchema field surface is unpinned by meta_snapshot (which only pins the op-enum). The wire contract (string/null/absent) is unchanged and this is not a correctness issue; declining to add a field-level schema snapshot as out of scope for this bug.

    Final verification (isolated CARGO_TARGET_DIR):
    - cargo nextest run -p swissarmyhammer-views: 84 passed, 0 failed (4 regression tests now: preserves_omitted_fields, explicit_empty_clears, preserves_commands, creates_fresh_when_absent).
    - cargo clippy -p swissarmyhammer-views --all-targets -- -D warnings: clean.
    - cargo fmt -p swissarmyhammer-views --check: clean.

    Moving to review.
  timestamp: 2026-06-22T14:00:07.652690+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd980
title: 'views: `set view` is full-replace — partial updates silently drop icon/card_fields/commands'
---
## What
Discovered while fixing 01KTCRY5W2BP7TYTHV4JB9CH8K (all views showed the LayoutGrid icon).

`SetView` (`crates/swissarmyhammer-views/src/operations.rs`) defaults every field (`#[serde(default)]`), and `handle_set_view` (`crates/swissarmyhammer-views/src/server.rs`) builds a complete `ViewDef` from the request with `commands: Vec::new()`. The write is full-replace: a caller who intends a partial update (e.g. `{op: \"set view\", id, name, kind}`) silently wipes `icon`, `card_fields`, and `commands` from the on-disk view file.

This is exactly how the degenerate `{id, name: '', kind: unknown}` files got written over the builtin grid views on this board (committed in 5d69e2eeb). The empty-name case is now rejected (`ViewDef::validate` + `ViewsContext::write_view`), and degenerate files no longer shadow builtins — but a *named* partial `set view` still destroys the unspecified fields.

## Acceptance Criteria
- [ ] Decide and implement the contract: either read-modify-write merge semantics for omitted optional fields, or explicit full-replace documented in the op schema (and the tool description warns about it).
- [ ] A `set view` that omits `icon`/`card_fields` on an existing view no longer silently strips them (or the replace contract is explicit and tested).
- [ ] Regression test at the views server layer. #bug