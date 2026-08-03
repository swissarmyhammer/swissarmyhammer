---
assignees:
- claude-code
position_column: todo
position_ordinal: e180
title: Merge nine single-rule builtin validators into code-security and code-hygiene
---
## What

The review fleet makes one agent task for each validator that matches the changed
files. Nine builtin validators hold one rule each, so one changed source file
costs nine agent tasks. Merge them into TWO sets, split by concern.

The source of truth is `builtin/validators/` (deployed to `~/.validators` by
`sah init`; see `builtin/validators/README.md` for the precedence rules).

Every one of the nine matches `@file_groups/source_code`, so the match globs
merge without loss. Probes are the only real constraint: `dead-code` needs
`callers`, and the rest need none.

## The two sets

**`code-security`** — no probes. Rules:

- `no-secrets.md`
- `injection.md`
- `command-safety.md`

**`code-hygiene`** — `probes: [callers]`, which `dead-code` needs. Rules:

- `no-commented-code.md`
- `function-length.md`
- `cognitive-complexity.md` (from `complexity`)
- `missing-docs.md`
- `data-driven.md`
- `dead-code.md`

Keep security separate from hygiene. A leaked credential or an injection hole is
not untidiness, and a set named "hygiene" understates it. Two names, two
concerns.

## Out of scope — do not touch these

- `test-integrity` — it matches `@file_groups/test_files` as well as source, so
  it does not merge with a source-only set. Leave it whole.
- `reuse` (`probes: [similar]`) and `duplication` (`probes: [duplicates]`) —
  each carries its own probe. Folding either in would force its probe on every
  rule in the set. Leave them alone.
- `naming` and `magic-numbers` — user-level sets in `~/.validators` only. They
  do not exist in `builtin/validators/`.
- The language sets (`rust`, `python`, `swift`, `dart`, `js-ts`, `numpy`) and
  `completeness`.

## Changes

- Create `builtin/validators/code-security/VALIDATOR.md`, `match.files: [@file_groups/source_code]`, no probes.
- Create `builtin/validators/code-hygiene/VALIDATOR.md`, `match.files: [@file_groups/source_code]`, `probes: [callers]`.
- Move the nine rule files unchanged into the two `rules/` directories.
- Delete the nine retired set directories from `builtin/validators/`.
- Make the builtin validator refresh remove a retired builtin set from the
  deployed store (`~/.validators`), but ONLY when the deployed files are
  identical to what was shipped. A user-modified set of the same name stays.

## Subtasks

- [ ] Create `builtin/validators/code-security/VALIDATOR.md`
- [ ] Create `builtin/validators/code-hygiene/VALIDATOR.md` with `probes: [callers]`
- [ ] Move the nine rule files into the two new `rules/` directories
- [ ] Delete the nine retired set directories
- [ ] Remove retired, unmodified builtin sets from the deployed store on refresh (`crates/mirdan/src/install.rs`)
- [ ] Update the embed and loader tests

## Acceptance Criteria

- [ ] The loader reports `code-security` with 3 rules and no probes
- [ ] The loader reports `code-hygiene` with 6 rules and `probes: ["callers"]`
- [ ] The loader no longer reports the nine retired set names from the builtin layer
- [ ] `test-integrity`, `reuse` and `duplication` still load unchanged
- [ ] A refresh deploy removes an unmodified retired set from the target store, and keeps a user-modified set of the same name
- [ ] Every one of the nine rule texts ships unchanged — no rule is reworded, weakened or dropped by this merge

## Tests

- [ ] Update `test_builtin_validators_embed_expected_sets` in `crates/mirdan/src/builtin_validators.rs`: assert `code-security` and `code-hygiene` are present and the nine retired names are gone
- [ ] Update the loader tests in `crates/swissarmyhammer-validators/src/builtin/mod.rs` (they read `../../builtin/validators` directly): assert both new sets, their rule counts, and their probes
- [ ] New test for the refresh prune in `crates/mirdan/src/install.rs`: deploy the old set, refresh, assert it is gone; deploy a modified copy, refresh, assert it stays
- [ ] `cargo test -p mirdan -p swissarmyhammer-validators` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #review