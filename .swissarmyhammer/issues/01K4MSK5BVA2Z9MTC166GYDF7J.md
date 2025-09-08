# Eliminate toml_core Wrapper - Use swissarmyhammer-config and figment

## Problem
We have a large custom TOML implementation in `swissarmyhammer/src/toml_core/` that reinvents configuration parsing when we already have a dedicated `swissarmyhammer-config` domain crate that uses `figment`. This creates duplication, maintenance overhead, and unnecessary complexity.

## Current State Analysis

### What We Have (Problematic Duplication)
- **`swissarmyhammer/src/toml_core/`** - Large custom TOML implementation
- **`swissarmyhammer-config`** - Domain crate using `figment` for configuration

### Evidence of Duplication
1. **Custom TOML Core**: `swissarmyhammer/src/toml_core/mod.rs` - Complete custom TOML parsing system
2. **Existing Config Crate**: `swissarmyhammer-config` - Uses `figment` (industry standard)
3. **Parallel Systems**: Two different approaches to configuration in the same codebase

## Why This Is Wrong
- **Duplication**: Two configuration systems doing the same job
- **Maintenance Overhead**: Custom TOML parser needs ongoing maintenance
- **Ecosystem Standards**: `figment` is the Rust ecosystem standard for configuration
- **Domain Separation**: Configuration should be handled by the config domain crate
- **Unnecessary Complexity**: Reinventing well-solved problems

## Proposed Solution
**Eliminate `toml_core` entirely** and consolidate all configuration handling through `swissarmyhammer-config` which already uses `figment`.

## Benefits of Using figment via swissarmyhammer-config
- ✅ **Mature Ecosystem Solution**: `figment` is battle-tested and widely used
- ✅ **Multiple Sources**: Environment variables, files, CLI args, etc.
- ✅ **Type Safety**: Strong integration with `serde` 
- ✅ **Validation**: Built-in validation and error handling
- ✅ **Documentation**: Well-documented with community support
- ✅ **Domain Separation**: Configuration logic belongs in config crate
- ✅ **Reduced Maintenance**: No custom parser to maintain

## Implementation Plan

### Phase 1: Identify toml_core Usage
- [ ] Find all usages of `swissarmyhammer/src/toml_core/` in codebase
- [ ] Catalog what functionality is currently provided by toml_core
- [ ] Map toml_core features to figment/swissarmyhammer-config equivalents
- [ ] Identify any unique features that need migration

### Phase 2: Enhance swissarmyhammer-config if Needed
- [ ] Review `swissarmyhammer-config` capabilities vs toml_core requirements
- [ ] Add any missing functionality to swissarmyhammer-config using figment
- [ ] Ensure swissarmyhammer-config can handle all current toml_core use cases
- [ ] Add proper error handling and validation using figment patterns

### Phase 3: Migrate Users to swissarmyhammer-config
- [ ] Update imports from `swissarmyhammer::toml_core::*`
- [ ] To `swissarmyhammer_config::*` (using figment internally)
- [ ] Update all configuration loading to use the domain crate
- [ ] Replace custom TOML parsing with figment-based solutions
- [ ] Update tests to use swissarmyhammer-config patterns

### Phase 4: Remove toml_core
- [ ] Remove `swissarmyhammer/src/toml_core/` directory entirely:
  - `mod.rs` - Custom TOML core implementation
  - `parser.rs` - Custom TOML parser
  - `value.rs` - Custom value types  
  - `error.rs` - Custom error handling
  - `configuration.rs` - Custom configuration handling
- [ ] Remove toml_core exports from `swissarmyhammer/src/lib.rs`
- [ ] Remove any toml_core-related dependencies if no longer needed

### Phase 5: Update Dependencies
- [ ] Ensure `swissarmyhammer-config` is properly available where needed
- [ ] Remove any unused TOML-related dependencies from main crate
- [ ] Verify figment provides all needed functionality through config crate
- [ ] Update Cargo.toml dependencies appropriately

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify configuration still works
- [ ] Test all configuration loading scenarios
- [ ] Verify environment variable handling still works
- [ ] Ensure configuration validation is preserved

## Files to Remove

### swissarmyhammer/src/toml_core/ (Entire Directory)
- `mod.rs` - Main module with custom TOML implementation
- `parser.rs` - Custom TOML parser implementation
- `value.rs` - Custom value type system
- `error.rs` - Custom TOML error types
- `configuration.rs` - Custom configuration handling

### swissarmyhammer Updates
- `src/lib.rs` - Remove toml_core module exports
- Remove any toml_core re-exports

## Expected Migration Patterns

### Before (Custom toml_core)
```rust
use swissarmyhammer::toml_core::{ConfigValue, Configuration};
```

### After (Domain crate with figment)
```rust
use swissarmyhammer_config::{load_configuration, TemplateContext};
```

## Success Criteria
- [ ] `swissarmyhammer/src/toml_core/` no longer exists
- [ ] All configuration handled through `swissarmyhammer-config` + `figment`
- [ ] No custom TOML parsing code in main crate
- [ ] Configuration functionality preserved and working
- [ ] Tests pass with domain crate configuration
- [ ] Reduced maintenance overhead and complexity

## Risk Mitigation
- Thoroughly map all toml_core functionality before removal
- Ensure swissarmyhammer-config can handle all use cases
- Test configuration loading extensively
- Keep migration changes granular for easy rollback
- Verify environment variable and file loading works

## Benefits
- **Eliminate Duplication**: Single configuration approach
- **Use Ecosystem Standards**: `figment` is the Rust standard
- **Domain Separation**: Configuration handled by config crate
- **Reduced Maintenance**: No custom TOML parser to maintain
- **Better Testing**: figment is well-tested and documented

## Notes
This eliminates a major unnecessary wrapper/reimplementation. Configuration is a solved problem in Rust - we should use `figment` through our domain crate rather than maintaining a custom TOML implementation.

The `swissarmyhammer-config` crate already exists and uses proper ecosystem tools. We should consolidate all configuration logic there instead of having parallel systems.