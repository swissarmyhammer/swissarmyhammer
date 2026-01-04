---
severity: error
tags: [logging, formatting, code-quality]
---

# Tracing Pretty Format Rule

## Description

All tracing log statements (trace!, debug!, info!, warn!, error!) that log complex objects MUST use the `Pretty` wrapper with `{}` formatting instead of `{:?}` or `{:#?}` debug formatting.

## Rationale

Using the `Pretty` wrapper provides consistent, readable YAML-formatted output with a leading newline for all complex objects in logs. This makes logs easier to read and debug, especially for nested structures and configuration objects. The YAML format is more readable than Rust's Debug format and includes proper indentation.

## Correct Usage

```rust
use swissarmyhammer_common::Pretty;

// ✅ CORRECT: Use Pretty with {} formatting
tracing::info!("Config loaded: {}", Pretty(&config));
tracing::debug!("Request: {}", Pretty(&request));
tracing::trace!("State: {}", Pretty(&state));

// For references that are already borrowed
tracing::info!("Value: {}", Pretty(value_ref));

// For simple types that already have Display, no Pretty needed
tracing::info!("Count: {}", count);
tracing::info!("Name: {}", name);
```

## Incorrect Usage

```rust
// ❌ WRONG: Using {:?} debug formatting
tracing::info!("Config loaded: {:?}", config);
tracing::debug!("Request: {:?}", request);

// ❌ WRONG: Using {:#?} alternate debug formatting
tracing::info!("Config loaded: {:#?}", config);
tracing::debug!("Request: {:#?}", request);
```

## Requirements

- Types passed to `Pretty` MUST implement `serde::Serialize`
- The output will be YAML formatted with a newline before the content

## Exceptions

- Simple primitive types (numbers, strings, booleans) that already implement Display don't need Pretty
- Error types that already have good Display implementations don't need Pretty
- When logging within the `Pretty` implementation itself (to avoid infinite recursion)

## Detection

Check for patterns:
1. `tracing::(trace|debug|info|warn|error)!` calls
2. That contain `{:?}` or `{:#?}` format specifiers
3. Suggest replacing with `{}` and wrapping the value with `Pretty(&value)`

## Remediation

1. Add import: `use swissarmyhammer_common::Pretty;`
2. Replace `{:?}` with `{}`
3. Wrap the value with `Pretty(&value)`

Example transformation:
```rust
// Before
tracing::info!("Loaded config: {:?}", config);

// After
use swissarmyhammer_common::Pretty;
tracing::info!("Loaded config: {}", Pretty(&config));
```
