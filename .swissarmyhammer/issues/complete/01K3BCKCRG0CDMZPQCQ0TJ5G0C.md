fix all clippy warnings

## Proposed Solution

Found several clippy warnings that need to be fixed:

1. **Invisible character detection** in `swissarmyhammer-config/tests/error_scenarios_tests.rs:347` - needs to replace invisible characters with visible Unicode escapes
2. **Collapsible match** in `swissarmyhammer-config/tests/end_to_end_tests.rs:481` - can collapse nested if-let statements  
3. **Bool assertion comparison** - multiple instances of `assert_eq!(expr, true/false)` that should use `assert!()` or `assert!(!)`
4. **Double ended iterator last** in `swissarmyhammer-config/tests/cross_platform_tests.rs:89` - should use `next_back()` instead of `last()`

Implementation steps:
1. Fix invisible character issue by replacing with explicit Unicode escapes
2. Collapse nested if-let statements into a single pattern match
3. Replace all `assert_eq!(expr, true)` with `assert!(expr)` and `assert_eq!(expr, false)` with `assert!(!expr)`
4. Replace `.last()` with `.next_back()` on double-ended iterator
5. Run clippy again to verify all warnings are resolved
## Implementation Notes

Successfully resolved all clippy warnings:

### Fixed Issues:
1. **Invisible character detection** in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-config/tests/error_scenarios_tests.rs:353` - replaced zero-width space characters with explicit Unicode escapes `\u{200B}`
2. **Collapsible match** in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-config/tests/end_to_end_tests.rs:479` - collapsed nested if-let statements into a single pattern match
3. **Bool assertion comparisons** - replaced all instances of:
   - `assert_eq!(expr, true)` with `assert!(expr)`
   - `assert_eq!(expr, false)` with `assert!(!expr)`
   - Fixed in 3 test files: integration_tests.rs, end_to_end_tests.rs, cross_platform_tests.rs
4. **Double ended iterator last** in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-config/tests/cross_platform_tests.rs:89` - replaced `.last()` with `.next_back()` on double-ended iterator

### Verification:
- ✅ `cargo clippy --all-targets --all-features -- -D warnings` passes with no warnings
- ✅ `cargo build` completes successfully  
- ✅ `cargo nextest run --fail-fast` - all 3019 tests pass

All clippy warnings have been successfully resolved without breaking any existing functionality.