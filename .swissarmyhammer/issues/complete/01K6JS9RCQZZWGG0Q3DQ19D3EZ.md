the rule command needs a description.md like the other subcommands


## Proposed Solution

The rule command is missing its description.md file. I will:

1. Create `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/description.md`
2. Add `pub const DESCRIPTION: &str = include_str!("description.md");` to `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/mod.rs`
3. Write comprehensive documentation covering:
   - Rule management and testing capabilities
   - Available subcommands: list, validate, check
   - Usage examples for each subcommand
   - Common workflows
   - Format consistent with other command descriptions (agent, plan, validate)

The description will explain that rules are code quality and style enforcement patterns that can be listed, validated for correctness, and checked against source files with various filtering options.



## Implementation Notes

Successfully implemented the description.md for the rule command:

1. **Created** `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/description.md`
   - Comprehensive documentation covering all rule command functionality
   - Documented rule discovery and precedence (builtin → user → project)
   - Explained rule structure with YAML frontmatter
   - Covered all three subcommands: list, validate, check
   - Included usage examples and common workflows
   - Documented agent configuration options

2. **Added** `pub const DESCRIPTION: &str = include_str!("description.md");` to mod.rs
   - Located at line 17 in `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/mod.rs`

3. **Updated** dynamic_cli.rs to use the DESCRIPTION constant
   - Changed `build_rule_command()` to use `crate::commands::rule::DESCRIPTION`
   - Replaced hardcoded long_about string with the constant
   - Now consistent with other commands (agent, plan, validate, etc.)

4. **Verified** implementation:
   - `cargo build` succeeds with no warnings
   - `sah rule --help` displays the new comprehensive help text
   - All 1151 tests pass in swissarmyhammer-cli package
   - Help text now matches the pattern used by other subcommands



## Code Review Fixes

Addressed findings from code review:

1. **Fixed outdated comment** in `swissarmyhammer-cli/src/commands/rule/mod.rs:19`
   - Removed misleading comment that claimed DESCRIPTION was "no longer used in CLI definitions"
   - DESCRIPTION is actually actively used in dynamic_cli.rs:1141

2. **Replaced `eprintln!` with `tracing::error!`** in `swissarmyhammer-cli/src/commands/rule/mod.rs:27`
   - Consistent with project's use of the tracing crate for logging
   - Error handling now uses `tracing::error!("Rule command failed: {}", e)`

3. **Fixed type mismatch** in `swissarmyhammer-rules/tests/checker_partials_integration_test.rs`
   - Added import: `use swissarmyhammer_common::error::SwissArmyHammerError`
   - Updated `skip_if_agent_unavailable` function signature from `&anyhow::Error` to `&SwissArmyHammerError`
   - This matches the actual error type returned by `RuleChecker::check_file`

**Verification:**
- ✅ All 3229 tests pass with `cargo nextest run`
- ✅ No clippy warnings with `cargo clippy --all-targets --all-features -- -D warnings`