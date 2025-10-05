# Use Violation Severity for Logging Level

## Description
When logging violations in `checker.rs`, use the violation's severity to pick the appropriate tracing log level instead of always using `warn!`.

## Current State
In `checker.rs` around line 322:

```rust
tracing::warn!("{}", violation);
```

This always logs violations at WARN level regardless of their actual severity.

## Desired Behavior
The logging level should match the violation severity:
- `Severity::Error` → `tracing::error!`
- `Severity::Warning` → `tracing::warn!`
- `Severity::Info` → `tracing::info!`
- `Severity::Hint` → `tracing::debug!` or `tracing::info!`

## Implementation
```rust
match violation.severity {
    Severity::Error => tracing::error!("{}", violation),
    Severity::Warning => tracing::warn!("{}", violation),
    Severity::Info => tracing::info!("{}", violation),
    Severity::Hint => tracing::debug!("{}", violation),
}
```

## Location
File: `swissarmyhammer-rules/src/checker.rs`
Function: `RuleChecker::check_file()`
Line: ~322

## Acceptance Criteria
- [ ] Violations log at appropriate level based on severity
- [ ] Error severity uses `tracing::error!`
- [ ] Warning severity uses `tracing::warn!`
- [ ] Info severity uses `tracing::info!`
- [ ] Hint severity uses `tracing::debug!` or `tracing::info!`
- [ ] Existing tests continue to pass
