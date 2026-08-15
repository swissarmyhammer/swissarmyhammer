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
- actor: claude-code
  id: 01m03wh1x5xyyv101v8d8sqb38
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` over 4909a5eb2 — engine 0 findings, 0 confirmed, 0 refuted, 9 validators attempted, 0 failed. Directed verification added 1 finding: crates/mirdan/src/builtin_validators.rs:367. Probes run and reverted: roster-entry-without-file RED, file-without-roster-entry RED, nested-directory RED, and a new fixture-shipping set (`builtin/validators/dart/fixtures/`) left all 8 `mirdan builtin_validators` tests GREEN — the escape.
    - next: hold `FIXTURE_ROSTERS` to the sets that ship a `fixtures/` directory by reading them from disk, then re-review.
  timestamp: 2026-08-15T23:36:17.573006+00:00
- actor: claude-code
  id: 01m03wj1svmw991jj1406f0awx
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 1 file (crates/mirdan/src/builtin_validators.rs). RED watched first in both directions; the direction-1 failure named all nine missing fixtures unprompted.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14241 passed, 0 failed, 0 skipped. No stray artifacts: the real ~/.validators and HOME were not written, and the repo holds no untracked probe files.
    - commit: 4909a5eb2 test(validators): fill the missing fixtures in CODE_HYGIENE_FIXTURES (^s1qh4tv)
    - review: findings — 1 finding, crates/mirdan/src/builtin_validators.rs:367. The engine answered 0 findings over 1 file with 9 validators attempted; the finding comes from directed verification.
    - detail: `FIXTURE_ROSTERS` is a hand-written list of SETS, and nothing compares it against the sets that actually ship a `fixtures/` directory. A whole set escapes the new guard. The reviewer reproduced it: a new `builtin/validators/dart/fixtures/` holding two `.tmpl` files left all 8 tests passing, while `test_every_builtin_validator_file_is_embedded` passed beside them — which proves the build script embedded and shipped the new fixtures with no roster holding them. This is the card's own omission defect one level up. The fix is to read the set list from disk rather than write it by hand.

    ### The card's headline was wrong, and this is verified

    `CODE_HYGIENE_FIXTURES` is a `const` inside `#[cfg(test)] mod tests`. It is NOT the install roster. `crates/mirdan/build.rs` calls `BuiltinGenerator::new("validators").source_dir("../../builtin/validators").all_extensions().skip_dirs(&["target"])`, which embeds the WHOLE directory with no list gating it.

    Both the implementer and the reviewer measured this independently. A `sah init user` into a throwaway HOME wrote all 55 fixtures, and `sah doctor` reported `code-hygiene/magic-numbers-dart` as "fixtures pass" BEFORE the change, with no degraded row.

    So the defect was a COVERAGE gap, not a shipping gap. No deployed store ever lacked the nine files, and the card's second "Done when" was satisfied before this work started. All four "What to do" items still applied, because the roster was unheld.
    - next: iteration 2 works the open finding
  timestamp: 2026-08-15T23:36:50.235436+00:00
- actor: claude-code
  id: 01m03x463g0qr6kr795a00gk3h
  text: |-
    ## Iteration 2 — the open finding is answered

    **File changed**: `crates/mirdan/src/builtin_validators.rs`. One file, all edits inside `#[cfg(test)] mod tests`.

    ### The escape, reproduced exactly as the reviewer states it

    With `builtin/validators/dart/fixtures/` holding two `.tmpl` files and NO new guard:

    ```
    Starting 8 tests across 2 binaries (515 tests skipped)
        PASS mirdan builtin_validators::tests::test_every_builtin_validator_file_is_embedded
        ...
    Summary 8 tests run: 8 passed, 515 skipped
    ```

    A whole fixture-shipping set reached a deployed store with no roster holding it, and every test stayed green.

    ### RED, watched first, both directions

    Direction 1, the reviewer's scenario, with the new test in place and the `dart` probe still on disk:

    ```
    thread 'builtin_validators::tests::test_every_fixture_shipping_set_stands_in_the_rosters' panicked at crates/mirdan/src/builtin_validators.rs:245:9:
    these sets ship a `fixtures/` directory and `FIXTURE_ROSTERS` names none of them, so every fixture they ship reaches a deployed store with no roster holding it: ["dart"]
    ```

    It names the set, unprompted.

    Direction 2, with the probe removed and `("reuse", MANIFESTS_FIXTURES)` added to the roster of rosters:

    ```
    thread 'builtin_validators::tests::test_every_fixture_shipping_set_stands_in_the_rosters' panicked at crates/mirdan/src/builtin_validators.rs:245:9:
    `FIXTURE_ROSTERS` names these sets and no set directory on disk ships a `fixtures/` directory for them: ["reuse"]
    ```

    Both probes are removed. `git status --porcelain --untracked-files=all -- builtin/ crates/` answers one line only: `M crates/mirdan/src/builtin_validators.rs`. Nothing under `builtin/` is modified and nothing there is untracked.

    ### What "a set that ships fixtures" means, read from the code

    `crates/mirdan/build.rs` calls `BuiltinGenerator::new("validators").source_dir("../../builtin/validators").all_extensions().skip_dirs(&["target"]).preserve_extensions()`. It embeds the WHOLE tree and no list gates it. So a set ships fixtures when its directory holds a `fixtures/` subdirectory, and every file under that subdirectory reaches a deployed store. `fixture_shipping_sets_on_disk` reads exactly that, and it passes over a top-level `target` for the one reason the build script does.

    ### The hand-written per-set rosters are KEPT, and here is why

    The two lists answer different questions, and neither answers the other one:

    - The derived set list answers WHICH SETS must carry a roster. It cannot say which files a roster must name, because it reads no fixture filename.
    - `CODE_HYGIENE_FIXTURES` and `MANIFESTS_FIXTURES` answer WHICH FILES each roster names. Deriving those from disk as well would compare the directory against itself, and the comparison would hold nothing: a renamed or deleted fixture would move both sides together and pass. They are the only thing that reports `magic-numbers-dart.fail.dart.tmpl` gone.

    So the set list is derived and the file rosters stay written by hand. Every set that ships fixtures now ends up held: `test_every_fixture_shipping_set_stands_in_the_rosters` holds the membership, and `test_fixture_rosters_and_the_fixtures_directory_agree` holds each roster's files, both of them in both directions.

    ### The rest of the file, read for the same cause

    The cause is a list that CLAIMS to be complete while nothing measures it against reality. Each list in the file was read against that:

    - `FIXTURE_ROSTERS` — its doc states "Every set that ships a tool rule stands here". A completeness claim, and it was unheld. FIXED.
    - `CODE_HYGIENE_FIXTURES`, `MANIFESTS_FIXTURES` — completeness claims, held in both directions since iteration 1.
    - `expected` in `test_builtin_validators_embed_expected_sets` — states "embedded builtins must include the `{set}` set". A presence claim, not a completeness claim, and it is measured against the embed in the direction it claims. Seven set directories stand outside it today (`completeness`, `dart`, `js-ts`, `numpy`, `python`, `rust`, `swift`), by design: the list records which sets survived the nine-set merge. Making it exhaustive would state a NEW requirement the file never carried. A new set is still held — by `test_each_set_has_a_manifest`, by `test_every_builtin_validator_file_is_embedded`, and now by the new guard when it ships fixtures.
    - `retired` in the same test — a negative claim, complete by its own shape: it names sets that must NOT stand, and a set that is gone leaves nothing to measure.
    - The two expected-rule lists (`code-security`, `code-hygiene`) — presence claims of the same merge, same answer as `expected`.
    - `BUILD_ARTIFACT_DIR` — one value, and it mirrors `build.rs`. A subdirectory of any other name fails an assertion LOUDLY rather than escaping, so it carries no silent omission.

    ### Preloaded validator rules applied while writing

    - `duplication` — `read_dir` plus its two panics stood in two places once the new helper needed them. Extracted `entry_paths_of`, called by both disk readers. The file-name extraction went the same way, into `stored_name_of`.
    - `rust/error-handling` — "A bare 'No such file or directory' without context is a blocker." Both `read_dir` failures name the directory through `entry_paths_of`.
    - `test-integrity/no-test-cheating` — "Every test should be able to fail." Two empty sets agree, so the new test asserts the tree ships at least one fixture set before it compares anything.
    - `code-hygiene/missing-docs` — every new item carries a doc comment stating what it holds and why.
    - `magic-numbers` — no literal added. The repeated `"fixtures"` path segment became the named `FIXTURES_DIR`.

    ### Gates

    - `cargo fmt` — clean, no file rewritten.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run -E 'rdeps(mirdan)'` — **3345 tests run: 3345 passed (9 slow), 0 skipped**. One more than the 3344 of iteration 1, which is the new guard.

    Not committed. Left in `doing`.

    ### One discovery, out of scope

    `crates/swissarmyhammer-mcp-proxy/.kanban` is still there, untracked and dated 5 July. It has its own card now: ^a1exfn5.
  timestamp: 2026-08-15T23:46:44.464927+00:00
- actor: claude-code
  id: 01m03x4d3y1ephqxdb1sm32j79
  text: |
    ### implement — changed
    - evidence: 1 file — crates/mirdan/src/builtin_validators.rs. RED watched first in both directions; direction 1 answered `these sets ship a `fixtures/` directory and `FIXTURE_ROSTERS` names none of them, so every fixture they ship reaches a deployed store with no roster holding it: ["dart"]` and direction 2 answered ``FIXTURE_ROSTERS` names these sets and no set directory on disk ships a `fixtures/` directory for them: ["reuse"]`. Both probes removed; `git status --untracked-files=all -- builtin/ crates/` shows one modified source file and nothing else. cargo fmt clean; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run -E 'rdeps(mirdan)' 3345 passed, 0 failed, 0 skipped.
    - next: /review the open finding again
  timestamp: 2026-08-15T23:46:51.646564+00:00
- actor: claude-code
  id: 01m03xdykbc4brpsk5dpfq1wa4
  text: |-
    ### test — green

    - evidence:
      - `cargo fmt --check` — exit 0, no diff.
      - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
      - `cargo nextest run --workspace` — 14242 tests run: 14242 passed (24 slow), 0 failed, 0 skipped.
      - `cargo nextest run -p mirdan` — 524 tests run: 524 passed (2 slow), 0 failed, 0 skipped. Confirms `mirdan builtin_validators::tests::test_every_fixture_shipping_set_stands_in_the_rosters` PASSED.
    - probe cleanup check: `git status --porcelain --untracked-files=all -- builtin/` returns empty. `builtin/validators/dart/` contains only git-tracked `rules/` and `VALIDATOR.md`, no `fixtures/` dir, no diff. No leftover probe files anywhere under `builtin/`.
    - task left in `doing`, no commit made.
  timestamp: 2026-08-15T23:52:04.459269+00:00
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

## Review Findings (2026-08-15 18:35)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 1 file reviewed, 2 not reviewed (`.kanban/`, excluded by `.reviewignore`).
>
> The validator fleet returned 0 findings over 9 attempted validators. The item below comes from the directed verification of this change's own guard, reproduced on the working tree and restored after.

- [x] `crates/mirdan/src/builtin_validators.rs:367` `review/guard-completeness` — `FIXTURE_ROSTERS` is a hand-written list of sets, and nothing compares it against the sets that actually ship a `fixtures/` directory, so a whole set escapes the new guard. Reproduced: create `builtin/validators/dart/fixtures/` holding two `.tmpl` files, and all 8 `mirdan builtin_validators` tests still pass — `test_every_builtin_validator_file_is_embedded` passes with them, which proves the build script embedded the new fixtures and shipped them to a store with no roster holding them. This is this card's own omission defect one level up: the roster is internally consistent and wrong only by omission, the failure mode a guard that walks the roster can never see. Derive the set list from disk rather than writing it by hand — read each directory under `builtin/validators/` that contains `fixtures/`, and assert that set stands in `FIXTURE_ROSTERS` — so a new fixture-shipping set fails a test instead of shipping unheld.

### Verified, and not a finding

These are the claims this pass checked and found correct. They need no work.

- The guard fails in BOTH directions. A roster entry with no file gives `the code-hygiene roster names these fixtures and no file on disk answers them: ["zz-guard-probe.absent.tmpl"]`. A file on disk with no roster entry gives `code-hygiene ships these fixtures on disk and no roster entry names them: ["zz-guard-probe.pass.rs.tmpl"]`. Both were watched RED on the working tree, and the tree was restored.
- `fixture_filenames_on_disk` reads the build artifact directory correctly. `crates/mirdan/build.rs` calls `.skip_dirs(&["target"])`, and the helper skips exactly the directory named `target`. A nested directory of any other name fails the assertion, watched RED with `code-hygiene/fixtures/zzprobedir is neither, and a flat roster cannot name what it holds`.
- The implementer's reading of `build.rs` is correct. `BuiltinGenerator::new("validators").source_dir("../../builtin/validators").all_extensions()` embeds the whole directory, so `CODE_HYGIENE_FIXTURES` is a test constant and never the install roster. All 55 fixtures reached a deployed store before this change. The card's headline overstated the defect: it is a coverage gap, not a shipping gap, and the second "Done when" was already satisfied.
- The two rosters agree with disk today. `code-hygiene/fixtures/` holds 55 files and the roster names 55; `manifests/fixtures/` holds 3.
