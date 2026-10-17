---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m54kasq1fpx66fzzvcmw4k1b
  text: |-
    ### test — green
    - cause: The failure came from external FSEvents load. A sourcekit-lsp loop outside this repository caused the load. The loop is stopped now.
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-code-context) | rdeps(swissarmyhammer-diagnostics)'` — 4982 run, 4982 passed, 0 failed, 0 skipped. `watcher::tests::test_watcher_fires_callback_on_file_create` passes now (15.943s).
    - next: This task stays open. A person decides to close it.
  timestamp: 2026-10-17T09:33:18.945894+00:00
- actor: claude-code
  id: 01m556dwarqeeq7t93j1e8y404
  text: |-
    ### seen again — 2026-10-17
    - `test_watcher_fires_callback_on_file_create` failed again in the full run and alone (30 s, no event). fseventsd was at about 108% CPU, with low disk activity.
    - Probable cause: fseventsd is stuck or has a large backlog. `sudo killall fseventsd` or a reboot can clear it.
    - The test does not tell a slow OS from a broken watcher. A fix to examine: a test that injects events through a fake source (the Swift port uses a fake event source and a manual clock), plus one smoke test with real FSEvents that is allowed to be slow.
  timestamp: 2026-10-17T15:07:02.872388+00:00
position_column: todo
position_ordinal: ffff8280
title: Fix test_watcher_fires_callback_on_file_create (swissarmyhammer-treesitter)
---
The test `swissarmyhammer-treesitter watcher::tests::test_watcher_fires_callback_on_file_create` failed. The test step for task ^7xn651m ran the command `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-code-context) | rdeps(swissarmyhammer-diagnostics)'` and found it.

Panic at `crates/swissarmyhammer-treesitter/src/watcher.rs:490`: "Expected on_files_changed callback to fire after creating a Rust file". The test waited 30 s (`EVENT_WAIT_TIMEOUT_MS`) and the callback did not fire.

The sibling test `test_watcher_fires_callback_on_file_modify` passed, but it took 20.4 s. Find why the create event is slow or does not come. Do not make the timeout longer to hide the cause. Then make the test pass. #test-failure