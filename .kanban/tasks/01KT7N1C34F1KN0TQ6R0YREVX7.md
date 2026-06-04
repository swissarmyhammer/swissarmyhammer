---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdf80
title: 'Flaky: entity watcher attachment event tests fail under full-suite parallel load'
---
crates/swissarmyhammer-entity/src/watcher.rs (test_attachment_create_emits_event ~line 880, test_attachment_remove_emits_event ~line 927)

Symptom: under `cargo nextest run --workspace` (14.9k tests, machine saturated) one of the two attachment-watcher tests intermittently fails by hitting its 10s deadline waiting for an EntityEvent::AttachmentChanged. Across two full runs a DIFFERENT one failed each time (create in run 1, remove in run 2); both PASS reliably when run in isolation or as a 2-test package run.

Root cause: macOS FSEvents backend delivery is starved/delayed when ~16 cores are busy compiling+running the whole suite. The tests already use a generous 10s retry-deadline loop; the event simply does not arrive within 10s under extreme load.

Not a branch regression: watcher.rs is unchanged vs the merge-base (no diff, no uncommitted changes). Tracking, not fixing, to avoid scope creep into the watcher's timing design and because globally serializing is disallowed by the test skill.

What was tried:
- Ran both in isolation (`cargo nextest run -p swissarmyhammer-entity watcher::tests::test_attachment_create_emits_event watcher::tests::test_attachment_remove_emits_event`): both PASS (~0.36s each).
- Confirmed each fails only inside the full --workspace run, and not the same one twice.

Possible fix: inject/poll the watcher's readiness instead of a fixed 200ms warm-up sleep, or raise the deadline / reduce FSEvents latency dependence; consider a serial group only for these two. #test-failure

## Review Findings (2026-06-03 22:10)

Scope: branch `skill` vs merge-base `18ece981a`. The fix for this flaky test lands in `.config/nextest.toml` (new `fsevents-watcher` test-group + override). `crates/swissarmyhammer-entity/src/watcher.rs` is unchanged (0-line diff), confirming the "not a branch regression" claim.

Verified: the override filter resolves to exactly the four intended tests via `cargo nextest list -E ...` (create/remove emit tests + both drop tests); those are precisely the four `EntityWatcher::start` call sites in the test module, so the comment's "only these four tests open a real EntityWatcher" claim is accurate. `cargo nextest list` exits 0 with no config errors/warnings, so the new test-group and filter DSL are valid. The choice of a nextest test-group over `#[serial]` is correctly justified (cross-process OS-subsystem contention). No duplication, no dead code, no correctness issues.

### Nits
- [x] `.config/nextest.toml:128-129` — Double blank line after the new `fsevents-watcher` override block, before the `embedding-models` profile comment. Other override blocks in the file are separated by a single blank line; drop one blank line for consistency.