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

## Proposed Solution

After analyzing the codebase, I found that the `toml_core` module is **completely unused**. The exports in `swissarmyhammer/src/lib.rs` have zero usage throughout the entire codebase - this makes the elimination straightforward.

### Key Findings:
1. **Zero External Usage**: No code outside `toml_core/` imports or uses any of the exported types (`TomlCoreParser`, `TomlCoreValue`, `TomlCoreConfiguration`, etc.)
2. **Complete Duplication**: `swissarmyhammer-config` already provides all needed functionality using `figment` (the industry standard)
3. **Self-Contained**: The `toml_core` module is completely self-contained with no external dependencies on it

### Implementation Plan:
1. **Remove toml_core exports** from `swissarmyhammer/src/lib.rs` (lines 134-139, 181-186)
2. **Remove toml_core module** entirely: `swissarmyhammer/src/toml_core/` directory
3. **Update lib.rs** to remove the `pub mod toml_core;` declaration
4. **Run tests** to verify nothing breaks (should be clean since no usage exists)

### Benefits:
- ✅ **Eliminates ~1500+ lines of unnecessary code**
- ✅ **Removes maintenance overhead** of custom TOML parser
- ✅ **No migration needed** - nothing uses it
- ✅ **Zero risk** - no external dependencies to break
- ✅ **Immediate simplification** - cleaner codebase

This is a perfect case for elimination - the duplication exists but is completely unused, making removal risk-free.

## Implementation Complete ✅

Successfully eliminated the `toml_core` wrapper entirely! The implementation was straightforward since there were no external dependencies.

### Changes Made:
1. **Removed toml_core exports** from `swissarmyhammer/src/lib.rs`
2. **Removed module declaration** `pub mod toml_core;` from `swissarmyhammer/src/lib.rs` 
3. **Deleted entire directory** `swissarmyhammer/src/toml_core/` (5 files, ~1,500+ lines of code)

### Verification Results:
- ✅ **Build Success**: `cargo build --workspace` passes cleanly
- ✅ **All Tests Pass**: 1,198 tests in swissarmyhammer crate all pass
- ✅ **No Dependencies**: Confirmed zero external usage before removal
- ✅ **Clean Removal**: No compilation errors or warnings related to toml_core

### Code Reduction:
- **Removed 1,500+ lines** of unnecessary custom TOML parsing code
- **Eliminated 5 source files**: mod.rs, configuration.rs, parser.rs, value.rs, error.rs
- **Simplified lib.rs exports** - cleaner public API

### Next Steps:
The elimination is complete! All configuration functionality is now properly consolidated in the `swissarmyhammer-config` crate using `figment` (the Rust ecosystem standard).

**Result**: Zero duplication, reduced maintenance overhead, cleaner codebase, and ecosystem-standard configuration management.