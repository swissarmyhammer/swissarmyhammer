---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvv0n4p07t4qvwhyc8ztan9j
  text: 'Picked up. Research done: read edit/mod.rs (current EditRequest/EditOperation parsing, legacy old_string/new_string + edits[] aliases), files/mod.rs (op=="" inference arm), schema.rs (collect_all_parameters keys on param.name), files/schema.rs (existing test_full_schema_has_all_parameters asserts old_string/new_string ARE props — must update since they become aliases), and ParamMeta (has const aliases(&''static [&''static str]) builder). Plan: TDD a pure normalize_edit_args(&Map)->Result<Vec<EditPair>> helper covering scalar/parallel-array/edits[] shapes + zip/broadcast/concat/mismatch-remainder + legacy alias regression; declare find/replace canonical in EDIT_FILE_PARAMS with old_string/new_string/oldText/newText demoted to .aliases(); extend op=="" inference so any find-ish/replace-ish key OR edits routes to edit BEFORE the content->write branch.'
  timestamp: 2026-06-23T19:52:28.608668+00:00
- actor: claude-code
  id: 01kvv1atzry49v82910z8qms68
  text: |-
    Implemented via strict TDD (RED watched, then GREEN).

    Changes:
    - edit/mod.rs: EDIT_FILE_PARAMS now declares canonical `find`/`replace` (+ `edits`) with FILE_PATH_ALIASES/FIND_ALIASES/REPLACE_ALIASES static slices; old_string/new_string/oldText/newText/old_text/new_text demoted to .aliases() of find/replace. Added pub EditPair{find,replace,replace_all}, pure pub normalize_edit_args(&Map)->Result<Vec<EditPair>> with helpers first_present/collect_strings/read_replace_all/pair_finds_replaces (zip / broadcast incl delete-many / concat top-level+edits[] / mismatch-remainder error that names the unpaired items, never drops). Added pub looks_like_edit(&Map). Rewrote execute_edit to source pairs from normalize_edit_args (kept legacy 'edits array cannot be empty', 'old_text cannot be empty', 'must be different' messages, file_path alias extraction, rate limit, apply loop, record_mutated_path). No new matching/IO/cascade logic — edit_file_atomic/validate_edit_operation untouched.
    - files/mod.rs: op=="" inference now calls edit::looks_like_edit(&arguments) BEFORE the content->write branch so {find,replace} and {edits:[...]} route to edit; legacy old_string path still routes (covered by FIND/REPLACE aliases).
    - schema.rs (files): updated test_full_schema_has_all_parameters — find/replace/edits ARE properties; old_string/new_string are NOT (collect_all_parameters keys on param.name).
    - integration/file_tools_integrations.rs: test_edit_tool_discovery_and_registration optional props -> [file_path, find, replace, edits, replace_all].

    Tests modified (behavior explicitly changed by task): test_edit_json_argument_parsing_error (find-without-replace now errors 'find provided without a matching replace'); test_full_schema_has_all_parameters and test_edit_tool_discovery_and_registration (old_string/new_string demoted to aliases, no longer schema properties).

    Verification (cargo nextest run, never plain cargo test):
    - cargo nextest run -p swissarmyhammer-tools edit:: files:: -> 206 passed, 0 failed (incl 14 normalize_* and test_infer_edit_from_canonical_find_replace / _from_edits_array).
    - cargo nextest run -p swissarmyhammer-operations -> 85 passed, 0 failed.
    - cargo nextest run -p swissarmyhammer-tools --test tools_tests file_tools_integrations -> 95 passed, 0 failed.
    - cargo fmt: applied. cargo clippy -p swissarmyhammer-tools -- -D warnings: clean (0 warnings).
    - double-check agent: PASS (verified normalization rules, dispatcher precedence, alias regression, schema absence, no silent behavior change).

    NOT committed. Moving to review.
  timestamp: 2026-06-23T20:04:19.576994+00:00
- actor: claude-code
  id: 01kvv1zftqrjj199ar6c3skeyx
  text: |-
    Review 2026-06-23: All substantive acceptance criteria verified and green (43 edit-module tests + 206 in `edit:: files::` scope pass). Confirmed: find/replace/edits are schema properties and old_string/new_string are NOT (resolve as aliases); scalar/parallel-array/edits[] all normalize to the same pair list; broadcast+concat+mismatch-remainder hold; legacy {old_string,new_string} and {oldText,newText} resolve identically; no-op {find,replace} and {edits:[...]} both dispatch to edit; dispatcher collision check clean (find/replace alias set is disjoint from content/pattern/path/etc., write/read/glob/grep not misrouted); normalize_edit_args is pure with no IO/matching/cascade logic (that is downstream ^tajpmp0).

    Engine returned 0 blockers, 8 warnings, 4 nits — all cosmetic clarity nits (execute_edit/execute >50 lines, missing doc comments on self-documenting public items, hardcoded test constant 1_000_000, "duplicated" inference if-else chains). Per review calibration, these are non-blocking style nits and the change already passed an adversarial double-check; passing the task to done rather than holding on cosmetics. Cosmetic nits recorded here for optional future cleanup, not gating.
  timestamp: 2026-06-23T20:15:36.279950+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd580
project: file-edit-tools
title: edit files — forgiving argument normalization to canonical (find, replace) pairs
---
## What
Rework the argument surface of `edit files` in `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` so any of three input shapes normalize to one canonical `Vec<(find, replace)>`. This replaces the current `EditRequest`/`EditOperation` parsing. No matching/IO logic here — just produce the pair list (and per-pair `replace_all`).

Declare the params in `EDIT_FILE_PARAMS` (`ParamMeta`) so schema + grammar see them, using `ParamMeta::aliases(...)`. **`find` and `replace` are the canonical `ParamMeta.name`s; `old_string`/`new_string` (and `oldText`/`newText`) are demoted to aliases.** The schema (`collect_all_parameters` in `swissarmyhammer-operations/src/schema.rs`) keys properties on `param.name`, so only the canonical names become top-level properties and the grammar emits `find`/`replace`; aliases resolve at parse time and are NOT separate schema properties:
- `find` ← aliases `{search, old, old_string, from, target, match}`
- `replace` ← aliases `{new, new_string, to, with, replacement}`
- `replace_all?` (Boolean), `edits?` (Array), `file_path` (keep `path`/`filePath`/`absolute_path` aliases).

Normalization rules (collect find-ish and replace-ish from top-level scalar, top-level arrays, the `edits[]` array, under any alias):
- N finds + N replaces → zip.
- N finds + 1 replace → broadcast the single replace to all (delete-many = many finds + one empty replace).
- top-level `find`/`replace` **and** `edits[]` → concatenate.
- mismatched array lengths (or 1-find-N-replaces) → pair what zips cleanly, surface the unpaired remainder in the error; never silently drop.
Extract a pure helper `normalize_edit_args(args) -> Result<Vec<EditPair>>` (unit-testable without IO).

**Dispatcher inference (required by the doc — the canonical happy path must route without an explicit `op`):** extend the `op == ""` inference arm in `files/mod.rs` (`FilesTool::execute`) so the presence of any find-ish/replace-ish key OR `edits` routes to `edit::execute_edit`. The `find`/`edits` check must precede the `content`→write branch so `{find, replace}` is not misrouted to write or to "Cannot determine operation". Keep the legacy `old_string`/`new_string` inference working.

## Acceptance Criteria
- [ ] In `schema_full`, `find` and `replace` appear as top-level properties; `old_string`/`new_string` do NOT appear as separate properties (they are aliases, resolved at parse time). (Do not assert property *ordering* — `collect_all_parameters` dedups into a `HashMap`, so order is non-deterministic.)
- [ ] `normalize_edit_args` resolves `{old_string, new_string}` and `{oldText, newText}` to the same `(find, replace)` pair as `{find, replace}` (alias regression).
- [ ] Scalar `{find, replace}`, parallel arrays, and `edits: [{search, with}, ...]` all normalize to the same pair list; broadcast + concat + mismatch-remainder rules hold.
- [ ] With no `op`, `{find, replace}` and `{edits:[...]}` both dispatch to edit (not write, not error).

## Tests
- [ ] Unit tests for `normalize_edit_args` covering every shape and the broadcast/concat/mismatch rules, plus the legacy-alias resolution.
- [ ] Schema test: `find`/`replace`/`edits` are properties in `schema_full`; `old_string`/`new_string` are not properties but resolve via the normalizer (extend `files/schema.rs` tests).
- [ ] Dispatcher tests in `files/mod.rs`: no-`op` `{find, replace}` and `{edits:[...]}` reach edit; legacy keys still reach edit.
- [ ] `cargo test -p swissarmyhammer-tools edit:: files::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.