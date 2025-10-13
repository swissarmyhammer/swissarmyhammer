# Implement auto-fix capability for rules

## Location
`swissarmyhammer-rules/src/rules.rs:63`

## Current State
```rust
/// Whether rule can auto-fix violations (future feature)
```

## Description
Rules have a field for auto-fix capability, but this feature is not yet implemented. This would allow rules to automatically fix violations they detect.

## Requirements
- Design auto-fix API for rules
- Implement fix application mechanism
- Add safety checks (backup, dry-run mode)
- Support multiple fix strategies per rule
- Add tests for fix application
- Document fix capabilities per rule type
- Consider integration with language servers/formatters

## Use Cases
- Automatic code formatting fixes
- Simple code pattern corrections
- Style guide enforcement
- Reducing manual fix burden

## Safety Considerations
- Always allow user confirmation before applying fixes
- Provide diff preview of changes
- Support rollback mechanism