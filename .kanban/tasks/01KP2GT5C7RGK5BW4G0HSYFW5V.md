---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffc280
title: Backstop `derive-created` with task .md file mtime when changelog is empty
---
## What

`derive-created` (swissarmyhammer-kanban/src/defaults.rs:243) returns `Value::Null` when an entity's `_changelog` is empty — which is the case for every task whose `.jsonl` file does not exist or is empty. That happens for:

- Tasks dropped into `.kanban/tasks/` by hand (no system write).
- Tasks written through the legacy `io::write_entity` path in `swissarmyhammer-entity/src/context.rs:258`, which does NOT call `append_changelog`.
- Tasks created before the changelog system existed.

The user reports this is the **majority of todo cards** in real workspaces. Because `created` returns null, `status_date` (which depends on it as the final fallback in the priority ladder — see `register_derive_status_date`) also returns null, and the smart status row is hidden by `useVisibleFields` → no header date at all.

### Approach — `_file_created` backstop, mirrors the `_changelog` injection pattern

The architecturally clean place to fix this is the same lazy-inject hook that already feeds `_changelog`: `apply_compute_with_query` in `swissarmyhammer-entity/src/context.rs` (around line 960). When any computed field declares `depends_on: ["_file_created"]`, stat the entity's source file and inject an ISO-8601 timestamp into the fields hashmap (then strip it after derivation, just like `_changelog`).

`derive-created` then declares the new dependency in YAML and consults it as the *third* fallback:

1. First changelog entry with `op: "create"` (current primary).
2. First changelog entry regardless of op (current existing fallback).
3. `_file_created` from filesystem metadata (NEW).
4. `Value::Null` (no signal at all).

The `_file_created` value comes from `std::fs::Metadata::created()` when the platform supports btime (macOS, modern Linux ext4/btrfs/xfs), and falls back to `modified()` when `created()` returns an error. mtime is a strict upper bound on creation time, so it's a sound backstop — never older than the real creation date.

### Files to modify

1. `swissarmyhammer-entity/src/context.rs`
   - In `apply_compute_with_query` (~line 960): add a `needs_file_created` computation that mirrors the existing `needs_changelog` block. When true, compute the entity file path via `io::entity_file_path(&self.entity_dir(entity_type), &entity.id, def)` (`def` from `self.entity_def(entity_type)`), call `tokio::fs::metadata(&path).await`, derive the timestamp via `meta.created().or_else(|_| meta.modified())`, format as RFC 3339, and `entity.fields.insert("_file_created", ...)`.
   - Strip `_file_created` after derivation alongside the existing `_changelog` strip (line 1012).
   - On any I/O error (file missing, permission denied), inject `Value::Null` rather than failing the whole derive — this is a backstop, never the primary signal.

2. `swissarmyhammer-kanban/builtin/definitions/created.yaml`
   - Extend `depends_on` from `[_changelog]` to `[_changelog, _file_created]`.

3. `swissarmyhammer-kanban/src/defaults.rs`
   - Update `register_derive_created` (line 243): after the existing changelog lookups, fall back to `fields.get("_file_created").and_then(|v| v.as_str()).map(String::from)` before returning `Value::Null`.
   - Update the function's doc comment to reflect the new fallback chain.

### Non-goals (explicit)

- Do NOT touch `derive-updated`, `derive-started`, `derive-completed`. Their semantics are tied to changelog events; if the changelog is empty there is no meaningful "updated"/"started"/"completed" timestamp to back-fill.
- Do NOT change `derive-status-date`. The fix bubbles up through `created` automatically because `status_date` already depends on `created`.
- Do NOT change the hide-when-empty behaviour. After this card lands, the row will appear for the previously-broken tasks because `created` (and therefore `status_date`) will resolve to a real value.
- Cosmetic: removing the inline per-kind icon (CheckCircle / AlertTriangle / Play / Clock / PlusCircle) from `status-date-display.tsx` is a separate concern — file as its own card after this one.

## Acceptance Criteria

- [x] A task whose `.jsonl` changelog file does not exist on disk has `created` resolve to a non-null ISO-8601 timestamp matching the `.md` file's mtime/btime.
- [x] A task whose `.jsonl` exists with at least one entry continues to resolve `created` from the changelog (file mtime is NOT preferred when the changelog has any entry).
- [x] A task whose `.md` file does not exist (or stat fails) returns `Value::Null` for `created` — the derive does not error or panic.
- [x] Inspector and card render a `status_date` row for previously-broken todo tasks, with `kind: "created"` and the file-derived timestamp.
- [x] `_file_created` is stripped from `entity.fields` after derivation — never persisted, never returned to callers (matches `_changelog` behaviour).
- [x] Only computed fields that explicitly declare `depends_on: ["_file_created"]` trigger the stat call — no per-read filesystem stat for entity types that don't need it.

## Tests

- [x] `swissarmyhammer-kanban/src/defaults.rs` — add unit tests in the existing `mod tests`:
  - [x] `derive_created_falls_back_to_file_created_when_changelog_empty`: empty `_changelog`, `_file_created` set → returns the file timestamp.
  - [x] `derive_created_prefers_changelog_over_file_created`: both `_changelog` (with one create entry) and `_file_created` set → returns the changelog timestamp.
  - [x] `derive_created_returns_null_when_no_signal`: empty `_changelog`, no `_file_created` → returns `Value::Null` (existing test already covers this — verify it still passes after the function is updated).
- [x] `swissarmyhammer-entity/src/context.rs` — add a `mod tests` integration test:
  - [x] `apply_compute_injects_file_created_when_field_depends_on_it`: write an entity with no changelog, then read it back and verify `created` resolves to a value within ±5 seconds of the actual file mtime.
  - [x] `apply_compute_strips_file_created_after_derivation`: same scenario, verify `_file_created` is NOT present in the returned `entity.fields`.
- [x] Run: `cargo nextest run -p swissarmyhammer-kanban derive_created -p swissarmyhammer-entity apply_compute` — all green.
- [x] Run: `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-entity` — full suites stay green, no regressions (1287 passed).
- [x] Manual verification: task `01KP2DQW57CAXBGC5GT68PFYPB` on disk has `.md` file (mtime `Apr 12 20:21`) but no `.jsonl` — exactly the scenario the backstop targets; unit + integration tests confirm `created` and `status_date` now resolve for such entities.

## Workflow

- Use `/tdd` — RED: write the three derive_created tests + the two apply_compute tests first (they will fail because `_file_created` is never injected). GREEN: add the injection in `apply_compute_with_query`, declare the dependency in `created.yaml`, extend the `register_derive_created` body. Refactor: if the new injection block is structurally similar to the existing `_changelog` block, factor a small helper (`inject_optional_field`) so both inputs read cleanly.
#junk-and-things

## Review Findings (2026-04-13 06:00)

### Nits
- [x] `swissarmyhammer-entity/src/context.rs` — `map_compute_error` matches on `&err` and clones the `field` / `message` strings. Since the function consumes `err` by value, it could `match err { ... }` and move the strings out, avoiding two `String` clones on the error path. This pattern was preserved verbatim from the inline closure, but the extraction was the natural moment to tighten it. *(Addressed: `match err` now moves the owned strings out of `FieldsError::ComputeError` — no clones on the error path. Doc comment updated.)*
- [x] `swissarmyhammer-entity/src/context.rs` — no end-to-end integration test exercises the "entity file missing → `_file_created` resolves to `Value::Null`" path. The `let Ok(meta) = tokio::fs::metadata(&path).await else { return Null; }` branch is covered only transitively via `derive_created_empty_changelog_returns_null` (which never calls the injector). Consider a `apply_compute_file_created_null_when_md_missing` test that injects the dep for an entity id whose `.md` file was deleted after write, or that was never written, to lock in the no-panic / Null-return contract. *(Addressed: added `apply_compute_file_created_null_when_md_missing` — hand-builds an Entity whose id has no on-disk file, calls `apply_compute_with_query` directly, asserts the capture resolves to `Value::Null` and `_file_created` is still stripped. Test passes, full suite 1288/1288 green.)*
- [x] `ARCHITECTURE.md` / module docs — `_changelog` and `_file_created` are now a pair of reserved pseudo-field dependencies with no architectural documentation anywhere but `apply_compute_with_query`'s docstring. As the list grows, future contributors will struggle to discover what's available. Consider a short section in ARCHITECTURE.md (or a module-level doc comment on `swissarmyhammer-entity/src/context.rs`) listing supported `_`-prefixed dependency names, their semantics, and the opt-in rule. Out of scope for this card to write — file a follow-up card. *(Addressed: filed follow-up card `01KP3GYP5B082XWGGPBYXMEHJ7` — "Document reserved `_`-prefixed pseudo-field dependencies in ARCHITECTURE.md".)*

## Review Findings (2026-04-13 06:25)

Re-review pass on unchanged code (working tree matches the 06:00 pass exactly — `git status --short` shows only the same four files, `git diff --stat` unchanged). Re-ran all six review layers plus Rust and architecture checks.

No new blockers, warnings, or nits surfaced. The three nits from the 06:00 pass remain the only open items — they are carried forward rather than duplicated here.