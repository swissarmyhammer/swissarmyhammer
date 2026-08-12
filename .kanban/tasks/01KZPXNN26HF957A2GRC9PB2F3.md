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
position_column: todo
position_ordinal: ffcc80
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