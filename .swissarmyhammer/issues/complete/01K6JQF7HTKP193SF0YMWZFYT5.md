# Remove dangerous pattern security checks from template validation

## Problem
The template validation code contains overly broad "security" checks that block legitimate Liquid template features:

```rust
// Check for dangerous patterns that could indicate code injection attempts
let dangerous_patterns = [
    "include",  // File inclusion
    "capture",  // Variable capture (potential data exfiltration)
    "tablerow", // Complex loops that could cause DoS
    "cycle",    // Another potential DoS vector
];

for pattern in &dangerous_patterns {
    if template_content.contains(&format!("{{% {pattern}")) {
        return Err(TemplatingError::Security(format!(
            "Template contains potentially dangerous pattern: {pattern}"
        )));
    }
}
```

## Why This Is Wrong

1. **`include`** - This is a standard Liquid feature for template composition. Blocking it prevents legitimate use of partials and reusable components.

2. **`capture`** - This is a basic Liquid feature for storing template output in variables. It's not "data exfiltration" - templates run in a sandboxed environment.

3. **`tablerow`** - This is just a Liquid loop construct for creating table rows. No more dangerous than `for` loops.

4. **`cycle`** - This is a utility tag for alternating values in loops (e.g., odd/even row styling). Not a DoS vector.

## Security Theater
These checks are security theater that:
- Block legitimate template features
- Don't actually improve security (templates are sandboxed by liquid-rust)
- Create a false sense of security
- Make the system harder to use

## Real Security
The `liquid-rust` library already provides security through:
- Sandboxed execution
- No filesystem access by default
- No arbitrary code execution
- Limited set of filters and tags

## Action Items

1. **Remove the dangerous patterns check** entirely from template validation
2. **Remove associated tests** that verify these patterns are blocked
3. **Keep actual security measures**:
   - Template size limits
   - Parse validation
   - Sandboxed execution environment

## Files Likely Affected
- Search for `dangerous_patterns` in the codebase
- Look for tests that verify blocking of `include`, `capture`, `tablerow`, `cycle`
- Check template validation code in swissarmyhammer-core or similar

## Impact
- Enables legitimate use of Liquid template features
- Removes false security checks
- Simplifies codebase
- No actual security downgrade (templates were already sandboxed)



## Proposed Solution

After analyzing the codebase, I found dangerous_patterns checks in 3 locations:

1. **swissarmyhammer/src/security.rs** (lines 213-227)
   - Main security validation function
   - Blocks `include`, `capture`, `tablerow`, `cycle` 
   - Has test `test_validate_template_security_dangerous_patterns`

2. **swissarmyhammer-templating/src/security.rs** (lines 66-80)
   - Duplicate implementation of the same check
   - Has test `test_validate_template_security_dangerous_patterns`

3. **swissarmyhammer-workflow/src/actions_tests/shell_action_tests.rs** (lines 1818-1833)
   - Test code checking that dangerous patterns exist in the validator

### Implementation Steps

1. **Remove dangerous_patterns array and check loop** from both security.rs files
   - Keep size limits, variable count limits, and nesting depth checks
   - These are legitimate DoS protections

2. **Remove the test_validate_template_security_dangerous_patterns tests**
   - From swissarmyhammer/src/security.rs
   - From swissarmyhammer-templating/src/security.rs

3. **Update shell_action_tests.rs**
   - Remove the test section that validates dangerous patterns are blocked
   - Keep other shell security tests intact

4. **Verify all tests pass**
   - Ensure no other code depends on these patterns being blocked
   - Run full test suite

### Why This Is Safe

The liquid-rust template engine provides sandboxing:
- No filesystem access by default
- No arbitrary code execution  
- Limited tag and filter set
- Templates run in isolated environment

The remaining security checks are sufficient:
- Template size limits (prevent resource exhaustion)
- Variable count limits (prevent resource exhaustion)
- Nesting depth limits (prevent stack overflow)
- Parse validation (malformed template rejection)

### Files to Modify

- `/Users/wballard/github/sah/swissarmyhammer/src/security.rs`
- `/Users/wballard/github/sah/swissarmyhammer-templating/src/security.rs`
- `/Users/wballard/github/sah/swissarmyhammer-workflow/src/actions_tests/shell_action_tests.rs`



## Implementation Notes

Successfully removed dangerous patterns security checks from template validation.

### Changes Made

1. **swissarmyhammer/src/security.rs**
   - Removed `dangerous_patterns` array and validation loop (lines 213-227)
   - Removed `test_validate_template_security_dangerous_patterns` test
   - Kept legitimate security checks: size limits, variable count, nesting depth

2. **swissarmyhammer-templating/src/security.rs**
   - Removed `dangerous_patterns` array and validation loop (lines 66-80)
   - Removed `test_validate_template_security_dangerous_patterns` test
   - Kept legitimate security checks: size limits, variable count, nesting depth

3. **swissarmyhammer-templating/src/lib.rs**
   - Updated `test_security_validation` to verify `{% include %}` is now allowed
   - Changed assertion from "should fail" to "should pass" for include tag

### Verification

- ✅ `cargo build` - Compiles successfully
- ✅ `cargo nextest run` - All 3241 tests pass (22 slow, 1 skipped)

### Impact

Templates can now use legitimate Liquid features:
- `{% include %}` - For template composition and partials
- `{% capture %}` - For storing template output in variables
- `{% tablerow %}` - For creating table rows with loops
- `{% cycle %}` - For alternating values in loops

These features are already used throughout the codebase:
- builtin/rules templates use `{% include "_partials/..." %}`
- Documentation uses `{% capture %}` and `{% include %}`
- Integration tests rely on these features

Security remains intact through:
- Template size limits (100KB untrusted, 1MB trusted)
- Variable count limits (1000 max)
- Nesting depth limits (10 levels max)
- Liquid-rust sandboxed execution environment
