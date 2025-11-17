# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-cli/src/commands/rule/cli.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/commands/rule/cli.rs
Line: 153
Severity: warning
Message: Duplicate Command builder setup pattern repeated across 8 test functions. Each test builds nearly identical Command structures with the same subcommand arguments (rule, patterns, severity, category, create-todos, no-fail-fast, force, max-errors, changed). This creates significant maintenance burden - any change to the check command structure requires updating all 8 tests.
Suggestion: Extract a helper function to build the base Command structure. Example:
```rust
fn build_check_command() -> Command {
    Command::new("rule").subcommand(
        Command::new("check")
            .arg(Arg::new("rule").short('r').long("rule").action(ArgAction::Append))
            .arg(Arg::new("patterns").action(ArgAction::Append))
            .arg(Arg::new("severity").short('s').long("severity"))
            .arg(Arg::new("category").short('c').long("category"))
            .arg(Arg::new("create-todos").long("create-todos").action(ArgAction::SetTrue))
            .arg(Arg::new("no-fail-fast").long("no-fail-fast").action(ArgAction::SetTrue))
            .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
            .arg(Arg::new("max-errors").long("max-errors").value_parser(clap::value_parser!(usize)))
            .arg(Arg::new("changed").long("changed").action(ArgAction::SetTrue))
    )
}
```
Then use: `build_check_command().try_get_matches_from([...])` in each test.

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/commands/rule/cli.rs
Line: 259
Severity: info
Message: Repeated assertion pattern across test functions lines 259-267, 289-297, 327-335. Multiple tests assert the same field values in nearly identical ways for CheckCommand validation (create_todos, no_fail_fast, force, changed flags).
Suggestion: Create a helper function for common assertions:
```rust
fn assert_check_flags(cmd: &CheckCommand, create_todos: bool, no_fail_fast: bool, force: bool, changed: bool) {
    assert_eq!(cmd.create_todos, create_todos);
    assert_eq!(cmd.no_fail_fast, no_fail_fast);
    assert_eq!(cmd.force, force);
    assert_eq!(cmd.changed, changed);
}
```
This reduces duplication and makes tests more maintainable.

---
*This issue was automatically created by `sah rule check --create-todos`*
