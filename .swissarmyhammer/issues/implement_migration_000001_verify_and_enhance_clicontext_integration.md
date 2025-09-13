# Verify and Enhance Implement Command CliContext Integration

## Overview

The implement command already uses CliContext and delegates to FlowSubcommand::Run (correct pattern), but needs verification and potential enhancements to ensure it fully follows the established patterns from doctor and prompt commands.

## Current Implement State

**Good aspects** (already implemented):
- ✅ Uses `CliContext` instead of `TemplateContext`
- ✅ Delegates to `FlowSubcommand::Run` to avoid code duplication
- ✅ Clean, simple implementation following proper patterns
- ✅ Help text from markdown file (`description.md`)

**Potential improvements to verify**:
- Does it properly use global `--verbose` and `--format` arguments?
- Does it handle error output consistently?
- Does it follow the same error handling patterns as other commands?

## Current Implementation

```rust
pub async fn handle_command(context: &CliContext) -> i32 {
    // Execute the implement workflow - equivalent to 'flow run implement'
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: vec![],
        interactive: false,
        dry_run: false,
        timeout: None,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
```

## Verification and Enhancement Steps

### 1. Test Global Arguments Work

**Verify these commands work correctly**:
```bash
sah --verbose implement                    # Should show verbose workflow execution
sah --format=json implement               # Should work (if workflow produces output)
sah --debug implement                     # Should show debug information
sah --quiet implement                     # Should suppress non-error output
```

### 2. Verify Error Handling Consistency

**Check**:
- Are errors handled consistently with doctor/prompt commands?
- Does it return proper exit codes?
- Are error messages formatted consistently?

### 3. Enhance Output Integration (If Needed)

**If implement needs its own output**:
- Check if implement command should show status before delegating to flow
- Consider adding pre-execution validation messages
- Ensure any implement-specific output uses CliContext patterns

**Example enhanced implementation**:
```rust
pub async fn handle_command(context: &CliContext) -> i32 {
    if context.verbose {
        println!("Starting implement workflow...");
    }

    // Check for pending issues first (implement-specific logic)
    let pending_issues = check_pending_issues(context)?;
    
    if pending_issues.is_empty() {
        if context.verbose {
            println!("No pending issues found - nothing to implement");
        }
        return EXIT_SUCCESS;
    }

    if context.verbose {
        println!("Found {} pending issues to implement", pending_issues.len());
    }

    // Execute the implement workflow using flow command
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: vec![],
        interactive: false,
        dry_run: false,
        timeout: None,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
```

### 4. Create Display Objects (If Needed)

**Only if implement has its own output to display**:
```rust
#[derive(Tabled, Serialize)]
pub struct ImplementStatus {
    #[tabled(rename = "Status")]
    pub status: String,
    
    #[tabled(rename = "Action")]
    pub action: String,
    
    #[tabled(rename = "Result")]
    pub result: String,
}
```

## Success Criteria

1. ✅ Global arguments work correctly with implement command
2. ✅ Error handling consistent with other commands
3. ✅ Proper delegation to flow command preserved
4. ✅ Any implement-specific output uses CliContext patterns
5. ✅ No code duplication with flow command
6. ✅ Maintains current functionality exactly
7. ✅ Follows established architectural patterns

## Expected Outcome

Most likely the implement command is already correctly implemented and just needs verification that global arguments work properly. The main value is ensuring it follows all the patterns we've established and identifying any minor enhancements needed.

## Files Modified (If Needed)

- `swissarmyhammer-cli/src/commands/implement/mod.rs` - Minor enhancements if needed
- `swissarmyhammer-cli/src/main.rs` - Verify CliContext is passed correctly

---

**Priority**: Low - Verification and minor enhancements
**Estimated Effort**: Small (likely just verification)
**Dependencies**: cli_prompt_000001_add_global_format_argument
**Benefits**: Ensures consistency across all commands