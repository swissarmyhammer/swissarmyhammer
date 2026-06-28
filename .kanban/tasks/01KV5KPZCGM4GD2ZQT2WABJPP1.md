---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv68y6a6bzd28tx2q2t0q7hc
  text: |-
    Picked up. Research complete. Findings:
    - entity-commands/index.ts: single OPERABLE_ENTITY_TYPES (incl board) used as applies_to for delete/archive/unarchive/cut/copy/paste; entity.paste name is "Paste {{entity.type}}".
    - clipboard_commands.rs COPYABLE_ENTITY_TYPES includes "board".
    - PasteMatrix target types (register_paste_handlers keys, target side): board, task, column, project, attachment -> PASTE_TARGET set.
    - Caption rendering (caption.rs) only supports {{entity.type}}; list-time CommandContext carries NO clipboard, so "Paste {{clipboard.type}}" is NOT readily resolvable -> required minimum plain "Paste" is the correct caption.
    Plan (TDD): split into SUBJECT_OPERABLE_ENTITY_TYPES (no board) for cut/copy/delete/archive/unarchive and PASTE_TARGET_ENTITY_TYPES for paste; rename paste to "Paste"; drop board from COPYABLE_ENTITY_TYPES; add paste-target drift guard pinning TS PASTE_TARGET to Rust PasteMatrix target set.
  timestamp: 2026-06-15T18:33:10.726594+00:00
- actor: claude-code
  id: 01kv69jvxt5x1expaekv5fjmgb
  text: |-
    Implemented (TDD red->green). Changes:
    - clipboard_commands.rs: COPYABLE_ENTITY_TYPES now drops "board" -> [task,tag,column,actor,project,attachment] (the SUBJECT set). Updated doc.
    - entity-commands/index.ts: split into SUBJECT_OPERABLE_ENTITY_TYPES (no board, used by cut/copy/delete/archive/unarchive) and PASTE_TARGET_ENTITY_TYPES [task,attachment,board,column,project] (used by entity.paste). Re-captioned entity.paste from "Paste {{entity.type}}" to plain "Paste" (clipboard-driven; list-time CommandContext has no clipboard so {{clipboard.type}} is not resolvable -> plain "Paste" is the required minimum).
    - app-shell-commands/ui.ts: app.inspect's OPERABLE_ENTITY_TYPES also drops "board" — forced by the shared drift guard assert_operable_applies_to which pins it to the now-board-less COPYABLE_ENTITY_TYPES. app.inspect is a subject-class op (inspect THE entity); "Inspect Board" on the root belongs to the same class. INSPECTABLE_ENTITY_PREFIXES still includes board: (caption/context resolution is separate from the applies_to gate).
    - support.rs: added assert_paste_target_applies_to drift guard pinning TS PASTE_TARGET to the Rust register_paste_handlers() target set (derived, not hand-maintained). entity.paste metadata assert now uses it instead of the subject guard.

    Tests (all RED before, GREEN after):
    - New production-path e2e builtin_entity_commands_e2e::subject_ops_suppressed_paste_offered_with_clean_caption_on_the_root_board: board focus -> cut/copy/delete/archive/unarchive ABSENT, paste PRESENT with name=="Paste"; column focus -> all six present.
    - New list_applies_to unit tests for root-board (subject absent, paste present) and column (subject present).
    - Paste-target drift guard + subject drift guard both enforce Rust<->TS lockstep.

    Results: cargo test -p swissarmyhammer-command-service (all suites incl integration 51/51, list_applies_to 11/11) PASS; cargo test -p swissarmyhammer-kanban PASS (1222 + others); cargo test -p swissarmyhammer-entity-mcp PASS. No warnings.

    Architecture note: dropping board from COPYABLE_ENTITY_TYPES is shared by app.inspect via the common guard, so app.inspect also drops board (one shared subject capability constant, no per-command branching). Flagging since the card text scoped only the 5 subject ops + paste, but the shared-guard design required app.inspect to follow.
  timestamp: 2026-06-15T18:44:28.218292+00:00
- actor: claude-code
  id: 01kv6bp07tfheqkgfp5xzvhq2v
  text: 'Re-picked up for review-findings pass. PRIMARY BLOCKER confirmed: prior pass coupled app.inspect.applies_to to the board-less subject set (OPERABLE_ENTITY_TYPES), suppressing "Inspect Board" on the root. Board IS inspectable (INSPECTABLE_ENTITY_PREFIXES includes `board:`). Plan: decouple app.inspect into its own INSPECTABLE_ENTITY_TYPES (derived from INSPECTABLE_ENTITY_PREFIXES, board PRESENT); separate inspect drift guard; fixtures assert board PRESENT for app.inspect while subject ops stay board-excluded. Plus: escape interpolation in support.rs probe-plugin generators (JSON-serialize injected strings), extract duplicate list_ids to common/mod.rs.'
  timestamp: 2026-06-15T19:21:08.090132+00:00
- actor: claude-code
  id: 01kv6c9r287t2khwkskt3pkb91
  text: |-
    Review findings addressed (all checkboxes flipped to [x]).

    PRIMARY FIX — board is inspectable again: ui.ts now declares app.inspect.applies_to = INSPECTABLE_ENTITY_TYPES, derived from INSPECTABLE_ENTITY_PREFIXES (prefix.slice(0,-1)) → [task, tag, column, board, attachment]. "Inspect Board" surfaces on the root board. Cut/copy/delete/archive/unarchive remain on the board-less SUBJECT set (SUBJECT_OPERABLE_ENTITY_TYPES / Rust COPYABLE_ENTITY_TYPES) — board EXCLUDED. Paste unchanged on PASTE_TARGET set (board INCLUDED, caption "Paste").

    Important discovery: the inspectable set is NOT the subject set + board — INSPECTABLE_ENTITY_PREFIXES omits actor/project (they are never bare scope-chain leaves). So the inspect drift guard (assert_inspect_applies_to) derives from the Rust INSPECTABLE_ENTITY_PREFIXES constant (caption), the same source inspectable_prefixes_mirror.rs pins to the plugin. First attempt deriving inspect set from COPYABLE+board failed RED (had actor/project); corrected to prefix-projection.

    Other findings:
    - support.rs probe-plugin generators: added json_string() helper; write_noop_probe_plugin and write_sentinel_probe_plugin now JSON-serialize every interpolated value (id/command_id/sentinel/log messages) → no V8 code-injection.
    - Duplicate list_ids extracted to tests/common/mod.rs; list_applies_to.rs + list_filter.rs import it (removed local copies; dropped now-unused BTreeSet import from list_applies_to.rs).
    - assert_operable_applies_to doc corrected (board-less subject set) + board-ABSENT anchor added; shared declared_applies_to/assert_no_non_entity_types extracted, assert_paste_target_applies_to de-duplicated onto it. Stale OPERABLE_ENTITY_TYPES doc ref in builtin_entity_commands_e2e.rs fixed.

    Tests/coverage:
    - New: list_applies_to::inspect_present_on_the_root_board_but_absent_on_a_field (board PRESENT, field ABSENT for app.inspect).
    - builtin_app_shell_commands_e2e::app_inspect_suppressed_on_a_field_offered_on_an_entity extended with a board-focus assertion (production path: real plugin loaded through V8, surfaces app.inspect on board:b1).

    Verification (fresh):
    - cargo test -p swissarmyhammer-command-service: all 26 test binaries, 0 failed (integration 51, list_applies_to 11).
    - cargo test -p swissarmyhammer-kanban: 0 failed (incl 1222-test suite).
    - cargo test -p swissarmyhammer-entity-mcp: 0 failed.
    - cargo fmt --check: clean (exit 0).
    - cargo clippy: no warnings in any file I touched (support.rs, ui.ts wiring, list_applies_to.rs, list_filter.rs, common/mod.rs, both e2e files). Remaining clippy warnings are pre-existing in unrelated crates (focus, window-service) and pre-existing test files (no_stale_imports.rs, builtin_nav_commands_e2e.rs, caption.rs) — out of scope for this card.
  timestamp: 2026-06-15T19:31:55.080043+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb480
project: builtin-commands
title: 'Root board context menu: split subject vs paste-target capability metadata; re-caption Paste by clipboard contents'
---
## Problem

The main/root context menu (right-click the board background) resolves to `focused_entity_type = "board"`. So the root board shows Cut/Copy/Paste/Delete/Archive/Unarchive Board. At the root these make no sense — except the cause is more subtle than "remove board":

- Cut / Copy / Delete / Archive / Unarchive act on the focused entity as the SUBJECT. The board-as-subject is meaningless at the root. → must be removed for `board`.
- Paste is the opposite direction: it drops the clipboard contents INTO the target. The board is a legitimate paste TARGET, so paste must STAY on `board`, captioned plain "Paste".
- Inspect is ALSO meaningful on the root board (Inspect Board), so app.inspect must continue to apply to `board`.

## Review Findings

### Blockers
- [x] PRIMARY: board is no longer inspectable. `app.inspect` in `builtin/plugins/app-shell-commands/commands/ui.ts` declared `applies_to: OPERABLE_ENTITY_TYPES` (the board-less subject set), so `list command` suppressed "Inspect Board" on the root board. FIXED: replaced `OPERABLE_ENTITY_TYPES` with `INSPECTABLE_ENTITY_TYPES`, derived from `INSPECTABLE_ENTITY_PREFIXES` (`prefix.slice(0,-1)`), board PRESENT, field ABSENT. Subject ops stay on the board-less `SUBJECT_OPERABLE_ENTITY_TYPES` / Rust `COPYABLE_ENTITY_TYPES`.
- [x] Inspect drift guard: added `assert_inspect_applies_to` in `support.rs`, pinning app.inspect's applies_to to the entity-type projection of the Rust `INSPECTABLE_ENTITY_PREFIXES` (caption) — board PRESENT. `assert_operable_applies_to` now anchors board ABSENT for the subject ops. `app.inspect` switched off the subject guard onto the inspect guard in `builtin_app_shell_commands_e2e.rs`.
- [x] Fixtures/tests updated: `list_applies_to.rs` adds `inspect_present_on_the_root_board_but_absent_on_a_field`; `builtin_app_shell_commands_e2e.rs` adds a board-focus assertion (Inspect Board surfaces) plus the new inspect guard in `assert_app_inspect`. Subject ops board-absent still asserted in `builtin_entity_commands_e2e::subject_ops_suppressed_paste_offered_with_clean_caption_on_the_root_board`. Production-path coverage proves Inspect surfaces for board focus.
- [x] `support.rs` `write_noop_probe_plugin`: `id` now JSON-serialized via new `json_string` helper before interpolation — no code-injection.
- [x] `support.rs` `write_sentinel_probe_plugin`: `command_id` / `id` / `sentinel` / log message all JSON-serialized via `json_string` before interpolation — no code-injection.
- [x] Duplicate `list_ids` extracted to `tests/common/mod.rs::list_ids`; `list_applies_to.rs` and `list_filter.rs` now import it (local copies removed, stray `BTreeSet` import dropped from list_applies_to.rs).

### Warnings/Nits
- [x] `assert_operable_applies_to` doc rewritten to reflect the board-less SUBJECT membership accurately; also added `board` ABSENT anchor assertion. Shared extraction `declared_applies_to` + `assert_no_non_entity_types` extracted; `assert_paste_target_applies_to` reuses it (de-duplicated). Stale `OPERABLE_ENTITY_TYPES` doc reference in `builtin_entity_commands_e2e.rs` corrected to `SUBJECT_OPERABLE_ENTITY_TYPES`.

## Workflow
- Use `/tdd`. #commands #entity-commands #frontend