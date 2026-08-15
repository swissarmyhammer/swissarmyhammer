---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m03vsaryzprtfdrkwts379fz
  text: |-
    ## Research — the card's counts are right, its headline is not

    **Counts verified against the current tree, before any edit.**
    - `CODE_HYGIENE_FIXTURES`: **46** entries. Card says 46. Correct.
    - `builtin/validators/code-hygiene/fixtures/`: **55** files. Card says 55. Correct.
    - The delta is exactly the nine the card names: `magic-numbers-dart.fail.dart.tmpl`, `magic-numbers-dart.pass.dart.tmpl`, `Cargo.lock.tmpl`, `Cargo.toml.tmpl`, `go.mod.tmpl`, `lib.rs.tmpl`, `Package.swift.tmpl`, `pyproject.toml.tmpl`, `tsconfig.json.tmpl`.
    - `manifests` fixtures: 3 on disk, 3 in `MANIFESTS_FIXTURES`. That roster already agreed.

    **The headline is wrong, and the correction matters for the next reader.**

    The card says the nine "are never installed into a deployed `~/.validators/` store" and that "a deployed store cannot run two rules' fixtures". Measured, that is false.

    `CODE_HYGIENE_FIXTURES` is **not an install roster**. It is a `const` inside `#[cfg(test)] mod tests`, read only by `test_tool_rule_fixtures_are_embedded`. Nothing in the install path reads it.

    The install roster is the build script. `crates/mirdan/build.rs` calls `BuiltinGenerator::new("validators").source_dir("../../builtin/validators").all_extensions().skip_dirs(&["target"])` — it embeds the whole directory and no list gates it. Proven end to end: `sah init user` into a throwaway HOME wrote **55** files to `~/.validators/code-hygiene/fixtures/`, the nine among them, and `sah doctor` reported `magic-numbers-dart` as `fixtures pass` on that store **before** any change of mine.

    So the defect is real but it is a **coverage** gap, not a shipping gap: nine fixtures reached the store with no test asserting they must. A later change that dropped one from the embed would have broken a deployed store with nothing failing here. Every one of the card's four "What to do" items still applies, and all four are done.

    **Why it survived** is exactly as the card says: the roster is walked in one direction only, every entry it names exists, so the list is internally consistent and wrong only by omission.

    **Pattern followed**: `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/scope_roster.rs` — named `const` rosters plus a guard that names the deviating members in the failure message.

    **Unrelated discovery, not touched**: a stray `crates/swissarmyhammer-mcp-proxy/.kanban/` board exists in the tree, dated 5 Jul, untracked and not gitignored. It predates this work by weeks. Left alone as out of scope.
  timestamp: 2026-08-15T23:23:20.222684+00:00
- actor: claude-code
  id: 01m03vt7wkrxbwxw8sz1m004rw
  text: |-
    ### implement — changed

    **File changed**: `crates/mirdan/src/builtin_validators.rs` (one file; all edits inside `#[cfg(test)] mod tests` plus one doc comment).

    **All four "What to do" items:**

    1. **Added the nine** to `CODE_HYGIENE_FIXTURES`: the `magic-numbers-dart` fail/pass pair beside the other `magic-numbers` entries, then the seven shared package files (`Cargo.toml.tmpl`, `Cargo.lock.tmpl`, `lib.rs.tmpl`, `pyproject.toml.tmpl`, `tsconfig.json.tmpl`, `go.mod.tmpl`, `Package.swift.tmpl`). Roster 46 → 55, matching the directory.
    2. **Corrected the doc comment.** It claimed the package files were listed while they were not. It now states what the roster holds, why a `workspace`-scope tool needs a package file beside the pair, that the package files belong to no one rule, and it names the guard that holds the claim true.
    3. **Added the both-directions guard**: `test_fixture_rosters_and_the_fixtures_directory_agree`. It reads the set's `fixtures/` directory with `read_dir` and compares it against the roster both ways. New supporting items: `builtin_validators_dir()`, `fixture_filenames_on_disk()`, `assert_no_names_outside()`, and a `FIXTURE_ROSTERS` const that pairs each set with its roster. `test_tool_rule_fixtures_are_embedded` now reads `FIXTURE_ROSTERS` instead of its own inline copy, so one list feeds both guards. The guard covers `code-hygiene` AND `manifests`.
    4. **Verified against a real deployed store** (see below).

    **RED watched first, both directions.**

    Direction 1, against the roster as it stood (46 entries), before adding anything:

    ```
    thread 'builtin_validators::tests::test_fixture_rosters_and_the_fixtures_directory_agree' panicked at crates/mirdan/src/builtin_validators.rs:343:13:
    `code-hygiene` ships these fixtures on disk and no roster entry names them, so nothing holds them to reaching a deployed store: ["Cargo.lock.tmpl", "Cargo.toml.tmpl", "Package.swift.tmpl", "go.mod.tmpl", "lib.rs.tmpl", "magic-numbers-dart.fail.dart.tmpl", "magic-numbers-dart.pass.dart.tmpl", "pyproject.toml.tmpl", "tsconfig.json.tmpl"]
    ```

    It named all nine, unprompted.

    Direction 2, with a roster entry that no file answers:

    ```
    the `code-hygiene` roster names these fixtures and no file on disk answers them, so the rule they stand for has lost its fixtures: ["renamed-away.fail.rs.tmpl"]
    ```

    Both were re-proven AFTER the refactor described below, since the refactor rewrote the comparison:
    - removing `magic-numbers-dart.fail.dart.tmpl` from the roster → direction 1 fires naming it;
    - adding `renamed-away.fail.rs.tmpl` → direction 2 fires naming it.

    The probe entries were removed; the file carries neither.

    **Deployed-store verification** — throwaway HOME under the scratchpad, never the real `~/.validators` or the user's HOME:

    ```
    HOME=<scratch>/fakehome2 sah init user --cwd <scratch>/fakeproj
      → ~/.validators/code-hygiene/fixtures/ holds 55 files
    HOME=<scratch>/fakehome2 sah doctor --cwd <scratch>/fakeproj
      → exit 0
      → ✓ Validator Tool Rule · code-hygiene/magic-numbers-dart
          tool present (Dart SDK version: 3.11.0 (stable) ... on "macos_arm64"); fixtures pass
      → 4 Validator Tool Rule rows, no degraded row, no missing-fixture row
    ```

    The throwaway project had to be a real Dart project for the rule to be exercised — an empty directory makes doctor report "Validator Project Types: none detected" and it runs no tool rule at all. Worth knowing for the next agent verifying a rule this way.

    **Preloaded validator rules were applied, and four changed the code I first wrote:**
    - `duplication` — two near-identical directional loops are "one function with an argument". Replaced with one `assert_no_names_outside` helper called twice. Not parameterized on a `bool`, per `rust/type-safety` "No adjacent `bool` parameters".
    - `rust/error-handling` — "A bare 'No such file or directory' without context is a blocker." Both `read_dir` failures now name the directory via `unwrap_or_else` + `panic!` with `fixtures_dir.display()`.
    - `test-integrity/no-test-cheating` — "Every test should be able to fail." Two empty sets agree, so a directory that read as empty would have carried the test to a pass having compared nothing. The guard now asserts the directory is non-empty first.
    - `completeness/case-sensitivity-coverage` — names are compared character for character, and the helper doc now states why: the embedded name carries the on-disk spelling to the store, so a case-insensitive match would pass an entry the store never writes under that spelling.

    `fixture_filenames_on_disk` also rejects any subdirectory other than `target` (the build artifact directory `build.rs` skips), so a nested layout cannot silently slip past a flat roster.

    **Gates, all green:**
    - `cargo fmt` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, zero warnings.
    - `cargo nextest run -E 'rdeps(mirdan)'` — **3344 tests run: 3344 passed, 0 failed, 0 skipped**.

    Not committed. Left in `doing`.
  timestamp: 2026-08-15T23:23:50.035692+00:00
- actor: claude-code
  id: 01m03w3w7b1f5tz70jew4w8fp7
  text: |-
    ### test — green

    Gates:
    - `cargo fmt --check`: exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, zero warnings.
    - `cargo nextest run --workspace`: 14241 tests run, 14241 passed (26 slow), 0 skipped, 0 failed.
    - Targeted confirm: `cargo nextest run --package mirdan test_fixture_rosters_and_the_fixtures_directory_agree` — 1 passed. The new guard exists and passes.

    Stray-artifact check:
    - `git status --porcelain=v1`: only 5 tracked files modified — the two kanban task files for this task, one other kanban task's files, and `crates/mirdan/src/builtin_validators.rs`. No untracked (`??`) files in the repo.
    - Real `~/.validators`: newest entries timestamped 11:44, before this session's scratchpad activity began (~16:39). No writes during this task.
    - Scratchpad shows the implementer used throwaway `fakehome`/`fakehome2` directories, not the real HOME, for its `sah init user` verification.

    No fix needed. Left in `doing`. No commit made.
  timestamp: 2026-08-15T23:29:05.771009+00:00
position_column: doing
position_ordinal: '8280'
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