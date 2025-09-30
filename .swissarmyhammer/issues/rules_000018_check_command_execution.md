# Add Glob Expansion and Fail-Fast Execution to Check Command

Refer to ideas/rules.md

## Goal

Complete the check command with glob expansion and fail-fast execution via RuleChecker.

## Context

This completes the check command by expanding globs, creating the checker, and executing with fail-fast behavior.

## Implementation

1. Add glob expansion:
```rust
let mut target_files = Vec::new();
for pattern in &cmd.patterns {
    for entry in glob::glob(pattern)? {
        let path = entry?;
        if path.is_file() {
            target_files.push(path);
        }
    }
}
```

2. Create agent and checker:
```rust
let agent = create_agent_from_config()?;
let checker = RuleChecker::new(agent)?;
```

3. Execute with fail-fast:
```rust
match checker.check_all(rules, target_files).await {
    Ok(()) => {
        println!("✅ All checks passed");
        Ok(())
    }
    Err(RuleError::Violation(violation)) => {
        eprintln!("❌ Rule violation in {}", violation.file_path.display());
        eprintln!("Rule: {}", violation.rule_name);
        eprintln!("Severity: {:?}", violation.severity);
        eprintln!("\n{}", violation.message);
        std::process::exit(1);
    }
    Err(e) => {
        eprintln!("Error during checking: {}", e);
        std::process::exit(1);
    }
}
```

## Testing

- Test with matching files
- Test with no matches
- Test fail-fast behavior
- Test exit codes

## Success Criteria

- [ ] Glob expansion working
- [ ] Checker integration complete
- [ ] Fail-fast behavior correct
- [ ] Error display clear
- [ ] Tests passing
