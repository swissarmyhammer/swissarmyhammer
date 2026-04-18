---
assignees:
- claude-code
position_column: todo
position_ordinal: d980
title: 'Remove or stabilize 2 #[ignore]''d treesitter watcher timing tests'
---
What: swissarmyhammer-treesitter/src/watcher.rs ignores two tests at lines 586 and 685 with justification "filesystem watcher timing is inherently platform-dependent and can be flaky on CI". Per the test discipline, this is not acceptable — either make them deterministic or delete them.

Tests:
- watcher::tests::test_watcher_event_loop_full_lifecycle
- watcher::tests::test_watcher_callback_error_triggers_on_error

Acceptance Criteria:
- Refactor the tests to drive the watcher's event loop deterministically (mock the filesystem event source, use explicit signal channels, or a virtualized clock) and unignore them, OR delete them and replace the coverage with unit tests on the non-IO pieces of the watcher.
- `cargo nextest run -p swissarmyhammer-treesitter` shows zero ignored tests.

Tests: the reworked tests must pass on CI (macOS + Linux) across 10 consecutive runs without flakes. #test-failure