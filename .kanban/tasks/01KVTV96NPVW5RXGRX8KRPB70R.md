---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
project: null
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