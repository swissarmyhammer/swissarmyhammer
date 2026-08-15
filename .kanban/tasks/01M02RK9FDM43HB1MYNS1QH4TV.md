---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe280
title: mirdan's fixture install roster is missing 9 files, so a deployed store cannot run two rules' fixtures
---
`CODE_HYGIENE_FIXTURES` in `crates/mirdan/src/builtin_validators.rs:227-274` lists **46** entries. **55** fixture files exist on disk. Nine are never installed into a deployed `~/.validators/` store.

## The nine

**Two are real rule fixtures**, and their absence breaks a shipped rule's fixture check on any deployed store:
- `magic-numbers-dart.fail.dart.tmpl`
- `magic-numbers-dart.pass.dart.tmpl`

**Seven are the shared package files** every probe package needs — a rule that stages a probe cannot build one without them:
- `Cargo.lock.tmpl`, `Cargo.toml.tmpl`, `go.mod.tmpl`, `lib.rs.tmpl`, `Package.swift.tmpl`, `pyproject.toml.tmpl`, `tsconfig.json.tmpl`

The doc comment at `builtin_validators.rs:225-226` **claims the package files are listed**. They are not. That false statement is why the gap survived.

## Why it was not caught

The roster is an install list, and nothing compares it against the directory it installs from. Every entry it does name exists on disk, so the list is internally consistent and only wrong by omission — the failure mode a roster test that walks the list can never see.

It also does not fail here: this repository runs the rules from `builtin/` directly, so the fixtures are always present. It fails only on a machine whose store was written by `sah init`, which is every user who is not developing this repo.

## Found by

The reviewer of `b88bab962` (`^z2r1psf`), which renamed the complexity fixtures in this same roster. Correctly NOT recorded as a finding there — `git log -1` puts the dart fixture at `d6a1d101c`, and `git show b88bab962` over that file touches only the complexity-to-function-length renames, so the gap pre-dates the commit and lands on no line it wrote.

## What to do

- Add the nine.
- Correct the doc comment, or delete the claim it makes.
- Add a guard that compares the roster against the fixtures directory in both directions, so an omission fails a test rather than waiting for a deployed store to notice. The `shipped/` guard family in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/` is the pattern — those already assert rosters against actual entries.
- Verify against a real deployed store: `sah init` into a throwaway HOME, then confirm `sah doctor` runs `magic-numbers-dart`'s fixtures rather than reporting the rule degraded.

## Done when

- The roster and the fixtures directory agree in both directions, held by a test.
- `sah doctor` on a freshly initialised store reports no degraded rule for a missing fixture.

#tool-validators