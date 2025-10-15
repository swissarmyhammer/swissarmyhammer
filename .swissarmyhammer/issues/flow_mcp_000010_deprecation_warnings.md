# Step 10: Add Deprecation Warnings to Wrapper Commands

Refer to ideas/flow_mcp.md

## Objective

Add deprecation warnings to the hardcoded `implement` and `plan` wrapper commands, guiding users to the new flow pattern.

## Context

The `implement` and `plan` commands are hardcoded wrappers around `flow run`. While they'll continue to work during the transition period, we should warn users that they're deprecated and suggest using the new shortcut pattern.

## Tasks

### 1. Add Deprecation Warning to Implement Command

Update `swissarmyhammer-cli/src/commands/implement/mod.rs`:

```rust
pub async fn handle_command(context: &CliContext) -> i32 {
    // Print deprecation warning
    if !context.quiet {
        eprintln!("Warning: 'sah implement' is deprecated.");
        eprintln!("  Use 'sah flow run implement' or 'sah implement' (via dynamic shortcut) instead.");
        eprintln!("  This command will be removed in a future version.");
        eprintln!();
    }
    
    // Execute the implement workflow
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        positional_args: vec![],
        params: vec![],
        vars: vec![],
        interactive: false,
        dry_run: false,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
```

### 2. Add Deprecation Warning to Plan Command

Update `swissarmyhammer-cli/src/commands/plan/mod.rs`:

```rust
pub async fn handle_command(plan_filename: String, context: &CliContext) -> i32 {
    // Print deprecation warning
    if !context.quiet {
        eprintln!("Warning: 'sah plan <file>' is deprecated.");
        eprintln!("  Use 'sah flow run plan <file>' or 'sah plan <file>' (via dynamic shortcut) instead.");
        eprintln!("  This command will be removed in a future version.");
        eprintln!();
    }
    
    // Execute the plan workflow
    let subcommand = FlowSubcommand::Run {
        workflow: "plan".to_string(),
        positional_args: vec![plan_filename],
        params: vec![],
        vars: vec![],
        interactive: false,
        dry_run: false,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
```

### 3. Update Command Descriptions

Update `swissarmyhammer-cli/src/commands/implement/description.md`:

```markdown
# Implement Command (DEPRECATED)

**This command is deprecated and will be removed in a future version.**

Use one of these alternatives instead:
- `sah flow run implement` (full form)
- `sah implement` (dynamic shortcut, preferred)

## Description

Executes the implement workflow for autonomous issue resolution.
```

Update `swissarmyhammer-cli/src/commands/plan/description.md`:

```markdown
# Plan Command (DEPRECATED)

**This command is deprecated and will be removed in a future version.**

Use one of these alternatives instead:
- `sah flow run plan <file>` (full form)
- `sah plan <file>` (dynamic shortcut, preferred)

## Description

Executes planning workflow for specific specification files.
```

### 4. Add --no-deprecation-warning Flag

Add optional flag to suppress warnings (useful for scripts):

```rust
// In CLI definition
.arg(
    Arg::new("no_deprecation_warning")
        .long("no-deprecation-warning")
        .env("SAH_NO_DEPRECATION_WARNING")
        .action(ArgAction::SetTrue)
        .help("Suppress deprecation warnings")
        .hide(true)  // Hidden flag for compatibility
)
```

Update handlers to check this flag:

```rust
pub async fn handle_command(context: &CliContext) -> i32 {
    if !context.quiet && !context.no_deprecation_warning {
        eprintln!("Warning: ...");
    }
    // ... rest of handler
}
```

### 5. Add Tests

Create `swissarmyhammer-cli/tests/deprecation_warnings_tests.rs`:

```rust
#[tokio::test]
async fn test_implement_shows_deprecation_warning() {
    // Test warning is printed to stderr
}

#[tokio::test]
async fn test_plan_shows_deprecation_warning() {
    // Test warning is printed to stderr
}

#[tokio::test]
async fn test_quiet_suppresses_warning() {
    // Test --quiet suppresses deprecation warning
}

#[tokio::test]
async fn test_no_deprecation_warning_flag() {
    // Test --no-deprecation-warning suppresses warning
}
```

## Files to Modify

- `swissarmyhammer-cli/src/commands/implement/mod.rs`
- `swissarmyhammer-cli/src/commands/plan/mod.rs`
- `swissarmyhammer-cli/src/commands/implement/description.md`
- `swissarmyhammer-cli/src/commands/plan/description.md`
- `swissarmyhammer-cli/src/context.rs` (add no_deprecation_warning field)
- `swissarmyhammer-cli/tests/deprecation_warnings_tests.rs` (create)

## Acceptance Criteria

- [ ] Implement command shows deprecation warning
- [ ] Plan command shows deprecation warning
- [ ] Warnings suggest correct alternatives
- [ ] Quiet mode suppresses warnings
- [ ] --no-deprecation-warning flag works
- [ ] Command descriptions updated
- [ ] All tests pass
- [ ] Commands still work correctly

## Estimated Changes

~120 lines of code
