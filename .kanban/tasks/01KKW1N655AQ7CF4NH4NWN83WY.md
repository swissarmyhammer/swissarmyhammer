---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffe980
title: 'warning: no test coverage for AvpHooks component in swissarmyhammer-cli'
---
swissarmyhammer-cli/src/commands/install/components/mod.rs:1231-1293\n\nThe `AvpHooks` struct is a new public `Initializable` component added to `swissarmyhammer-cli`, but there are no tests for it in this file or in any adjacent test module. The other components in this file (e.g. `LockfileCleanup`, `SkillDeployment`) also lack individual unit tests, so this follows existing convention — but since this is a new component and the behavior differs from the pre-existing `LockfileCleanup` (which it replaced in the `impl Initializable` block according to the semantic diff), a regression test is warranted.\n\nSpecifically: the `is_applicable` guard (Project/Local only) is the one place where `AvpHooks` diverges from a simple delegation, and it has no test.\n\nSuggestion: Add a unit test in this file's `#[cfg(test)]` block (or a new integration test) that constructs the `AvpHooks` component and asserts:\n- `is_applicable(&InitScope::Project)` returns `true`\n- `is_applicable(&InitScope::Local)` returns `true`\n- `is_applicable(&InitScope::User)` returns `false`\n\nVerification: `cargo nextest run -E 'package(swissarmyhammer-cli)'` with new tests passes." #review-finding