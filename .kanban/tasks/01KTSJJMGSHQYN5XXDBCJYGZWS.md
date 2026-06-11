---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9d80
project: local-review
title: 'fix(common): test_isolated_test_environment_drop_restores_home fails under parallel test_utils run'
---
## What

`cargo test -p swissarmyhammer-common --lib test_utils` consistently fails (reproduced 3/3 on main without unrelated changes):

```
---- test_utils::tests::test_isolated_test_environment_drop_restores_home stdout ----
assertion `left == right` failed
  left: Some("/Users/wballard")
 right: Some("/swissarmyhammer-test-home-restoration-sentinel")
```

The test passes when run alone, so it is a parallel-HOME interaction. Likely culprit: `test_isolated_test_home_drop_restores_home_none` (same module) mutates `HOME` via `std::env::remove_var`/`set_var` WITHOUT acquiring `HOME_ENV_LOCK`, so it can interleave with the sentinel phases of `test_isolated_test_environment_drop_restores_home` despite that test's careful locking.

## Acceptance Criteria

- [ ] `for i in 1 2 3; do cargo test -p swissarmyhammer-common --lib test_utils; done` — all green.
- [ ] Every test in `test_utils.rs` that mutates `HOME` does so under `acquire_home_env_lock()` (or `#[serial_test::serial(home_env)]`).