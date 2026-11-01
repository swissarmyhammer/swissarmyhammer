---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa380
title: route IsolatedEnvTest's bulk env fixture through EnvVarGuard
---
`crates/swissarmyhammer-config/tests/integration/environment_variables.rs` holds the last copy of the environment save-and-restore pattern that `swissarmyhammer_common::test_utils::EnvVarGuard` now owns. Card ^811xj0q routed the other twelve copies and left this one.

It was left because its `Drop` is not the plain single-variable restore the guard replaces. It runs an ordered three-step teardown:

1. restore `HOME`;
2. remove EVERY current `SAH_` / `SWISSARMYHAMMER_` variable, then put back only the ones that existed before the test;
3. restore the explicitly tracked variables, but ONLY those whose name carries neither prefix.

A naive `Vec<EnvVarGuard>` inverts step 3: the guards drop after `Drop::drop` returns, so a tracked `SAH_` variable the sweep just cleared would be written back, and the isolation the sweep exists for would break.

The work: keep the prefix sweep as the fixture's own step, and move only the single-variable restores (`HOME`, plus each non-prefixed tracked variable) onto `EnvVarGuard`, with the drop order stated in the field docs. Prove the ordering with a test that sets a `SAH_`-prefixed variable through `set_env_var` and asserts it is gone after the fixture drops.

Do not weaken the sweep to make the guards fit.