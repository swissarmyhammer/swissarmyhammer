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
position_column: doing
position_ordinal: '8280'
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