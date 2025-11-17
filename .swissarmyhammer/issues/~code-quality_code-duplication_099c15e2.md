# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-cli/src/commands/rule/check.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/commands/rule/check.rs
Line: 424-460
Severity: warning
Message: Duplicated violation processing logic with similar structure but different actions. Both branches (with and without create_todos flag) iterate over the stream, count violations, and handle errors in nearly identical ways. The only difference is whether create_issue_for_violation is called.
Suggestion: Extract the common stream processing logic into a helper function that accepts a closure for handling each violation. This would eliminate the duplication while maintaining clarity:

```rust
async fn process_violation_stream<F, Fut>(
    mut stream: impl Stream<Item = Result<RuleViolation, RuleError>> + Unpin,
    mut handler: F,
) -> CliResult<usize>
where
    F: FnMut(RuleViolation) -> Fut,
    Fut: Future<Output = CliResult<()>>,
{
    let mut violation_count = 0;
    while let Some(result) = stream.next().await {
        match result {
            Ok(violation) => {
                violation_count += 1;
                handler(violation).await?;
            }
            Err(e) => {
                return Err(CliError::new(
                    format!("Check failed: {}", e),
                    EXIT_CODE_ERROR,
                ));
            }
        }
    }
    Ok(violation_count)
}
```

Then use it in both branches:
- For create_todos: pass a closure that creates issues
- For reporting: pass a no-op closure

---

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/commands/rule/check.rs
Line: 473-481 and 491-499
Severity: info
Message: Duplicated result printing logic appears in both branches. Both check violation_count and print either success or error message, then return the same error condition.
Suggestion: Extract the final result handling into a single location after the if-else block:

```rust
// After the if-else block that processes violations
if !context.quiet {
    if violation_count == 0 {
        println!("All checks passed - no ERROR violations found");
    } else {
        println!("Found {} ERROR violation(s)", violation_count);
    }
}

if violation_count > 0 {
    return Err(CliError::new(
        format!("Found {} ERROR violation(s)", violation_count),
        EXIT_CODE_ERROR,
    ));
}

Ok(())
```

---
*This issue was automatically created by `sah rule check --create-todos`*
