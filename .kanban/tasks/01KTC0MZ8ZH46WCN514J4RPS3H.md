---
assignees:
- claude-code
depends_on:
- 01KTBNHSR4EVTVJ35MGGD510R2
position_column: todo
position_ordinal: '9580'
project: local-review
title: Author language validators from the *_REVIEW.md references (rust, python, js-ts, dart)
---
## What
Preserve the language-specific review depth that lives in the current skill's reference files by converting it into focused LANGUAGE VALIDATORS (rules-as-data) — so it survives the skill becoming a thin driver. Without this, deleting the reference files would silently drop all Rust/Python/JS/Dart-specific review guidance.

Convert each `builtin/skills/review/references/*_REVIEW.md` into a validator under `builtin/validators/`, matching by glob:

| Validator | Source file | `match.files` |
|-----------|-------------|---------------|
| `rust` | `RUST_REVIEW.md` | `**/*.rs` |
| `python` | `PYTHON_REVIEW.md` | `**/*.py` |
| `js-ts` | `JS_TS_REVIEW.md` | `**/*.{js,jsx,ts,tsx}` |
| `dart` | `DART_FLUTTER_REVIEW.md` | `**/*.dart` |

- Each validator's `rules/*.md` carry the existing language guidance (error handling, ownership/idioms, async, etc.) **verbatim where possible** — do not water it down. Severity per rule (mostly warning/nit; blocker where the source file marks it so).
- **No probes** — these are in-file idiom judgments.
- Delete the `references/*_REVIEW.md` files once migrated; the skill no longer links them (handled in the skill-rewrite task).
- New format: `name`, `description`, `match.files`, `severity`, no `trigger`, no `probes`.

## Acceptance Criteria
- [ ] `rust`, `python`, `js-ts`, `dart` validators exist with rules derived from the corresponding `*_REVIEW.md`; each matches its language's files and not others (e.g. `rust` matches `foo.rs`, not `foo.py`).
- [ ] The `references/*_REVIEW.md` files are removed.
- [ ] No `trigger`, no `probes`; `check validators` reports OK for the language set.
- [ ] Substantive content preserved (spot-check that key sections from each source file appear as rules).

## Tests
- [ ] Loader/parse + match test: each language validator parses and matches only its language's glob; `cargo test -p swissarmyhammer-validators` green.
- [ ] `check validators` OK for the language validators.
- [ ] (Behavioral proof — a planted language-specific issue flagged — can be added to the end-to-end task.)

## Workflow
- Content migration grounded in the existing reference files. Read each `references/*_REVIEW.md` and carry its guidance into the validator's rules. Depends on the format task (`match.files` schema, no `trigger`).