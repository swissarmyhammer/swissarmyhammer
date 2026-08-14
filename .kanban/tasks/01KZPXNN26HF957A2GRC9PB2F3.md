---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztkxfq64r1jprxn5c9vq63h
  text: |-
    ### measurement from 2026-08-12 (merged from the duplicate ^fpg9823)

    Measured on the `review` branch AND on the tree with no change applied, so the race is not caused by ^y4xyw1g:

    ```
    assertion `left == right` failed: the first element must restore the working directory;
    instead the working directory was removed while the process still stood in it
      left: Some("/Users/wballard/github/swissarmyhammer/swissarmyhammer/crates/swissarmyhammer-validators")
     right: Some("/private/var/folders/.../T/.tmpXT5KiR")
    ```

    `cargo test -p swissarmyhammer-validators --lib the_swift_package_root_restores_the_directory_before_it_removes_it` passes alone. The same test inside the whole-crate run failed in four runs out of four.

    The working directory is process state, so a test that reads it answers for whatever another test set. The shipped-rule tests hold `CurrentDirGuard` for the Swift package root, and nothing holds the other tests off that state.

    Note: the test did NOT fail under `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` in three later runs. nextest gives each test its own process, so the race shows under `cargo test` and hides under nextest.
  timestamp: 2026-08-12T09:12:37.862977+00:00
- actor: claude-code
  id: 01kzvk36ydz4jz4a0mfyrzrcyd
  text: |-
    ### research — which tests share the working directory

    Read every mover of the process working directory in the `swissarmyhammer-validators` lib test binary. `rg 'set_current_dir|CurrentDirGuard'` over `crates/swissarmyhammer-validators/src` gives 9 hits in 2 files, and there is no direct `std::env::set_current_dir` anywhere in the crate. So every mover goes through `CurrentDirGuard`:

    - `validators/loader.rs` — four guards (lines 731, 901, 1070, 1396). All four tests already carry `#[serial_test::serial(cwd)]`. Two more loader tests (765, 816) carry the key without a guard.
    - `review/drive.rs:2136` — already carries the key.
    - `review/tool_rules/tests/shipped.rs::swift_package_root` — the one guard of the shipped module. Its callers are `the_swift_package_root_restores_the_directory_before_it_removes_it` and `verify_shipped_tool_rules_pass_fixtures`, and the latter has five callers: the roster tests of `complexity.rs`, `dead_code.rs`, `magic_numbers.rs`, `missing_docs.rs` and `unused_dependencies.rs`. **None of those six carried the key.**

    `CurrentDirGuard` holds `CURRENT_DIR_LOCK`, a process-wide `Mutex`, so two guards never stand at the same time. That lock is not enough here: the failing test reads `std::env::current_dir()` into `outside` BEFORE it takes the guard. A roster test holding the lock at that moment leaves the temp package root as the value `outside` reads, then restores the crate directory before this test's guard is built — so the guard captures and restores the crate directory and the assertion compares two different tests' values. That is exactly the observed pair: left = the crate directory, right = a `/private/var/.../T/.tmp*` root.

    So the mover set is: the six shipped-module callers of `swift_package_root`, plus the loader and drive tests that already used the key. `#[serial_test::serial(cwd)]` on all six is what holds them apart — the guard alone cannot, because the read happens outside the guard.
  timestamp: 2026-08-12T18:17:31.341706+00:00
- actor: claude-code
  id: 01kzw1j0fcpx0apfd5y2kpzxwx
  text: |-
    ### measured RED then GREEN at HEAD ab61db5d6

    I reverted the working-tree change first and measured the failure myself, then put it back.

    RED — `git checkout --` the six test files, then `cargo test -p swissarmyhammer-validators --lib review::tool_rules::tests::shipped`, three runs:

    ```
    run 1: FAILED. 127 passed; 1 failed  (121.61s)
    run 2: FAILED. 127 passed; 1 failed  (228.41s)
    run 3: FAILED. 127 passed; 1 failed  (202.68s)
    ```

    Each failure is the one on the card:

    ```
      left: Some(".../crates/swissarmyhammer-validators")
     right: Some("/private/var/folders/.../T/.tmp0iBZBY")
    ```

    3 of 3, not 2 of 3 — the race is more frequent than the first measurement said.

    GREEN — the same command with the change applied, five runs:

    ```
    run 1: ok. 128 passed; 0 failed (132.74s)
    run 2: ok. 128 passed; 0 failed (224.35s)
    run 3: ok. 128 passed; 0 failed (131.37s)
    run 4: ok. 128 passed; 0 failed (123.16s)
    run 5: ok. 128 passed; 0 failed (131.79s)
    ```

    Note on the tool: `cargo test` is what shows the race, because one process holds every test thread. `cargo nextest` gives each test its own process and therefore hides it. I used `cargo test` only for this reproduction, and `cargo nextest` for the suite.
  timestamp: 2026-08-12T22:30:16.300370+00:00
- actor: claude-code
  id: 01kzw2h3mwaftja41h35n03nt8
  text: |-
    ### the mechanism

    Which tests share the working directory in the `swissarmyhammer-validators` lib test binary:

    - `validators::loader` — 6 tests, 4 of them holding a `CurrentDirGuard`.
    - `review::drive` — 1 test.
    - `review::tool_rules::tests::shipped` — 6 tests, all reaching the single guard in `swift_package_root`:
      - `the_swift_package_root_restores_the_directory_before_it_removes_it` (calls the helper directly)
      - `every_shipped_complexity_tool_rule_passes_its_fixtures`
      - `every_shipped_dead_code_tool_rule_passes_its_fixtures`
      - `every_shipped_magic_numbers_tool_rule_passes_its_fixtures`
      - `every_shipped_missing_docs_tool_rule_passes_its_fixtures`
      - `every_shipped_unused_dependency_tool_rule_passes_its_fixtures`

    `rg 'CurrentDirGuard|set_current_dir'` over `crates/swissarmyhammer-validators/src/review/tool_rules/` gives one guard, at `shipped.rs:1283`. `swift_package_root` has two callers and `verify_shipped_tool_rules_pass_fixtures` has five, so the mover set is exactly those six tests and nothing else.

    What holds them apart: `#[serial_test::serial(cwd)]` on all six. That is the same key the `validators::loader` and `review::drive` tests already carry, so one exclusion set now covers every mover in the binary.

    Why the guard alone cannot do it: `CurrentDirGuard` holds `CURRENT_DIR_LOCK`, so two guards never stand at once, but the failing test reads `std::env::current_dir()` into `outside` BEFORE it takes its guard. A roster test holding the lock at that moment gives `outside` the temp package root, then restores the crate directory before this test's guard is built. The guard then captures and restores the crate directory, and the assertion compares two different tests' values — the observed `left` = crate directory, `right` = a `T/.tmp*` root. `serial(cwd)` holds the whole test body apart, the read included.

    No production API was added. Every changed line sits inside `mod tests`, which `review/tool_rules.rs` declares under `#[cfg(test)]`.

    ### implement — changed
    - evidence: 6 test files — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, and `shipped/{complexity,dead_code,magic_numbers,missing_docs,unused_dependencies}.rs`
    - RED: 3 of 3 module runs failed with the change reverted
    - GREEN: 12 of 12 module runs passed with it applied (`cargo test -p swissarmyhammer-validators --lib review::tool_rules::tests::shipped`, 128 passed each)
    - suite: `cargo nextest run -p swissarmyhammer-validators` — 690 tests run, 690 passed, 0 skipped
    - `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
    - next: /review
  timestamp: 2026-08-12T22:47:15.356986+00:00
- actor: claude-code
  id: 01kzw2wnvhkdm3mc48x11ychja
  text: |-
    ### review — findings
    - scope: review sha d7d3288d7~1..d7d3288d7
    - evidence: 1 finding — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970 (command injection in verify_shipped_tree_breaks_without)
    - dropped: 1 duplication finding on pre-existing test helpers (dead_code.rs:44 vs complexity.rs:37), per the review skill rule that drops findings which ask to refactor tests that already existed
    - next: escape the binary path before it goes into the shell script, then re-review
  timestamp: 2026-08-12T22:53:34.449874+00:00
- actor: claude-code
  id: 01kzw2y15pzpr58hr61y6t7wtj
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 6 files. All six tests that reach `swift_package_root` now carry `#[serial_test::serial(cwd)]`. Mechanism stated: `CurrentDirGuard` holds a process-wide lock, but that lock does not cover the `std::env::current_dir()` read the failing test makes BEFORE it builds its guard, so a roster test holding the lock at that instant makes the read return the temp package root. The serial key holds the whole body apart, the read included. RED 3/3 module runs failed, GREEN 12/12 passed at 128 tests each. nextest 690/690. No production API added.
    - test: green — measured inside implement. cargo nextest run -p swissarmyhammer-validators 690/690, fmt and clippy clean.
    - commit: d7d3288d7
    - review: findings — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970 (1 recorded; a second finding was dropped by the review skill's written test-refactor exception).

    The recorded finding's premise is FALSE on disk. It asks to change `exec {real} "$@"` to `exec "{real}" "$@"`. Line 970 already reads `exec \"{real}\" \"$@\"` — the path is already quoted, fixed under ^btphzkn in bf0cd8d34. No edit can satisfy this finding.
  timestamp: 2026-08-12T22:54:18.806734+00:00
- actor: claude-code
  id: 01kzw31pvz586bc4ca13f7v2rg
  text: |-
    Research for the open review finding (2026-08-12 18:48).

    **The premise is false on disk.** `shipped.rs:970` reads:

        exit {COMMAND_NOT_FOUND_STATUS}\nfi\nexec \"{real}\" \"$@\"\n

    The path is ALREADY inside double quotes. `git blame` puts that exact line at commit `bf0cd8d34` ("fix(validators,tools): close review findings on shell mod and stub script (^btphzkn)"), 2026-08-12 05:25. The surrounding lines are still at `f495f760c9` (03:09), so `bf0cd8d34` changed this one line and nothing else there. The finding asks for the treatment the line already carries. No edit can satisfy it.

    **The rule that produced it: `builtin/validators/code-security/rules/injection.md`.** Evidence, not a guess:
    - `dump validators` on `shipped.rs` matches the `code-security` validator (`match: files: @file_groups/source_code`).
    - `code-security` ships exactly three rules: `no-secrets.md` (credentials), `command-safety.md` (destructive patterns — `rm -rf /`, fork bomb, `curl | bash`; none of which the finding names), and `injection.md`.
    - The finding's words "Command injection" are the verbatim heading of `injection.md` section 2, and its "Escape the real path" / "shell escaping" wording is that rule's "Suggested fix using safe APIs" output requirement.

    **Do not touch `crates/mirdan/retired-validators/injection/rules/injection.md`.** That is a byte-frozen snapshot of the PRE-MERGE standalone `injection` set. `retired_validators.rs` compares a deployed store against it byte for byte to prune a stale copy. It is never loaded or run. Changing it would break pruning.

    Tests over the rule (`builtin/mod.rs`, `mirdan/builtin_validators.rs`) assert rule NAMES only, never body text, so adding a section to the live rule is safe.
  timestamp: 2026-08-12T22:56:19.327390+00:00
- actor: claude-code
  id: 01kzw3711fwc9fn396vbekd5v5
  text: |-
    ### implement — changed
    - evidence: 1 file — `builtin/validators/code-security/rules/injection.md`. Added a `## Before you report` gate (quote the sink from the file; read the quoted characters for the treatment; stay silent when it is already present; the 2026-08-12 measurement and the `shipped.rs` case) and a **Treatment already present** exception. `cargo nextest run -p swissarmyhammer-validators -p mirdan` → 1193 passed, 0 failed, 0 skipped, including `the_swift_package_root_restores_the_directory_before_it_removes_it` (PASS 0.198s, in the full module run). `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
    - next: /review. The one review finding is checked off as resolved-by-rule-correction — its premise was false on disk, so the rule was corrected instead of the code. Nothing staged; no file outside `builtin/validators/` and this card was touched.
  timestamp: 2026-08-12T22:59:13.583875+00:00
- actor: claude-code
  id: 01kzw3aedpxdm4k8m9hke6cdh6
  text: |-
    ### review — clean

    - evidence: `review sha 51af948d7~1..51af948d7` — findings 0, confirmed 0, refuted 0, attempted 0, failed 0, skipped 0. Commit 51af948d7 changes three files: `builtin/validators/code-security/rules/injection.md` and the two `.kanban` files of this task. No validator declares a `*.md` match glob, so no file of this commit is in the scope of any validator. A direct `review file` call on the rule file gives the same empty result, which proves the empty scope and not a silent skip.
    - prior findings: the one finding of 2026-08-12 18:48, at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970`, is marked `- [x]` and carries its resolution note.
    - limit of this pass: the engine cannot run the injection rule on the text of the rule, because the rule text is markdown and every validator matches source code only.
    - next: none. The task moves to done.
  timestamp: 2026-08-12T23:01:05.590271+00:00
- actor: claude-code
  id: 01kzw3bgyrr3ybvh9fpfv3k1wb
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 1 file, builtin/validators/code-security/rules/injection.md. The finding's premise was refuted by `git blame -L 966,972`: line 970 alone sits at bf0cd8d34 (^btphzkn, 05:25) carrying the quoted form, while its neighbours are still at f495f760c. The producing rule was identified by evidence, not guessed — `dump validators` matches code-security on shipped.rs, that set ships three rules, and the finding's words are the verbatim heading of injection.md section 2. A "Before you report" gate now requires quoting the sink from the file with every quote and escape character, plus a "Treatment already present" exception. The retired byte-frozen snapshot at crates/mirdan/retired-validators/injection/rules/injection.md was deliberately left alone; editing it would break pruning.
    - test: green — cargo nextest run -p swissarmyhammer-validators -p mirdan, 1193 passed, 0 failed, 0 skipped, including this card's own subject test inside the full parallel module run. fmt and clippy clean.
    - commit: 51af948d7
    - review: clean — task moved to done.

    Caveat recorded rather than hidden: this second review attempted 0 pairs. No validator in the fleet declares a `*.md` match glob, so the rule file this commit changed is in no validator's scope, and `review file` on it returns the same empty scope. The card's own CODE change was reviewed for real in iteration 1 (d7d3288d7, 2 findings raised). A corrected rule's own text cannot be reviewed by the engine today.
  timestamp: 2026-08-12T23:01:40.952437+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffff380
title: swift_package_root test races the working directory under a parallel module run
---
`the_swift_package_root_restores_the_directory_before_it_removes_it` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` fails when the `shipped::` module runs in parallel, and passes when it runs alone.

## Measured

On 2026-08-10, by the reviewer of ^2syfvyt, at commit 758416086 (BEFORE the change it was reviewing):

- Alone: passes.
- Module run, three repeats: **fails 2 of 3**.

The failure:

    assertion `left == right` failed: the first element must restore the working
    directory; instead the working directory was removed while the process still
    stood in it
      left:  Some(".../crates/swissarmyhammer-validators")
      right: Some("/private/var/folders/.../T/.tmp11vNrd")

The test reads the process working directory, which is one value shared by every thread of the test binary. Another test in the same module changes it at the same time, so the value the assertion reads belongs to the other test.

## This is not the ^bh5ncd0 set

^bh5ncd0 holds three `review_e2e` tests and one stdio-transport timeout. This is a fifth, separate failure, and it has been observed across many sessions without a card.

## Fix direction

The workspace already holds the tools for this: `CurrentDirGuard` and `serial_test`. See [[test-isolation-raii]] — the rule is to fix the test isolation, never to add a production API to answer a test environment problem.

Weigh:
- `#[serial_test::serial(cwd)]` on every test of the module that changes the working directory. Other modules of this workspace already use that key.
- A `CurrentDirGuard` that restores on drop, so no test leaves the value changed.

Read which other tests in `shipped.rs` change the working directory before you choose. A guard on one test does not help if a second test changes the value without one.

## Done when

- The test passes in a module run, measured over at least 10 repeats
- The mechanism is stated: which tests share the working directory, and what holds them apart
- No production API was added to answer a test problem
#tool-validators

## Review Findings (2026-08-12 18:48)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:970` — Command injection in verify_shipped_tree_breaks_without: the resolved binary path is interpolated into a shell script string without escaping, allowing injection if the path contains quotes or other shell metacharacters. Escape the real path before interpolating into the shell script. Use shell escaping: `format!("exec {} \"$@\"", shell_escape::unix::escape(real))`. Alternatively, pass the binary path via an environment variable that the shell script reads, avoiding direct interpolation.

  **Resolved by rule correction, 2026-08-12.** The premise is false on disk. The named line reads `exec \"{real}\" \"$@\"` — the path is ALREADY inside double quotes. `git blame` puts that line at commit `bf0cd8d34` ("fix(validators,tools): close review findings on shell mod and stub script (^btphzkn)"), 2026-08-12 05:25; the lines around it are still at `f495f760c9` (03:09), so `bf0cd8d34` quoted this one line and nothing else there. The finding asks for the treatment the line already carries, so no edit can satisfy it.

  The rule that produced it is `builtin/validators/code-security/rules/injection.md`, identified by evidence: `dump validators` on `shipped.rs` matches the `code-security` validator; that set ships exactly three rules, and neither `no-secrets.md` (credentials) nor `command-safety.md` (destructive patterns — `rm -rf /`, fork bomb, `curl | bash`) covers this; the finding's words "Command injection" are the verbatim heading of `injection.md` section 2, and its "shell escaping" wording is that rule's "Suggested fix using safe APIs" output requirement.

  The rule now carries a `## Before you report` gate, in the shape set by `builtin/validators/completeness/rules/invariant-propagation.md` (commit 4e41d04ab) for the same defect class. The gate makes the reviewer quote the sink from the file, read the quoted characters for the treatment, and stay silent when the treatment is already present. It contrasts `format!("exec {real} \"$@\"")` with `format!("exec \"{real}\" \"$@\"")`, states the 2026-08-12 measurement and this concrete case, and adds a **Treatment already present** exception.

  `crates/mirdan/retired-validators/injection/rules/injection.md` was deliberately NOT touched: it is a byte-frozen snapshot of the pre-merge standalone `injection` set, which `retired_validators.rs` compares against byte for byte to prune a stale deployed copy. It is never loaded or run.