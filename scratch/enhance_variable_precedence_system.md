# Implement comprehensive variable precedence system

## Location
`swissarmyhammer-templating/src/variables.rs:89`

## Current State
```rust
// For now, use the current hardcoded behavior for backward compatibility
```

## Description
The variable resolution system currently uses hardcoded behavior for backward compatibility. A more flexible precedence system should be implemented to handle complex variable resolution scenarios.

## Requirements
- Design comprehensive variable precedence rules
- Support multiple variable sources (config, environment, arguments, etc.)
- Implement precedence ordering (e.g., args > env > config > defaults)
- Add tests for all precedence combinations
- Document precedence rules clearly
- Maintain backward compatibility or provide migration path

## Related
Affects templating across workflows and prompts.