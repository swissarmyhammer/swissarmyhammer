# Add shared file globbing function for rule checking

## Problem
Both CLI and MCP rule checking need to glob files (default: `**/*.*`) while respecting `.gitignore`. This logic should be shared to avoid duplication.

## Goal
Create a shared file globbing utility in `swissarmyhammer-rules` that both CLI and MCP can use.

## Requirements
- Create a public function that globs files with configurable pattern
- Default pattern: `**/*.*`
- Must respect `.gitignore` (use existing glob implementation from swissarmyhammer-core if available)
- Return list of file paths
- Should handle errors gracefully (invalid patterns, permission issues, etc.)

## Implementation Notes
- Check if there's already a glob function in swissarmyhammer-core that respects .gitignore
- If so, expose it through swissarmyhammer-rules or use it directly
- If not, implement it in the appropriate location
- Ensure it's consistent with other file operations in the codebase

## Dependencies
- Should be done after or alongside `consolidate_rule_check_logic`


## Proposed Solution

After analyzing the codebase, I discovered that:

1. **swissarmyhammer-common already has comprehensive glob functionality** in `glob_utils.rs` with:
   - `expand_glob_patterns()` function that respects .gitignore
   - `GlobExpansionConfig` for configuration
   - Default pattern support
   - Comprehensive tests

2. **swissarmyhammer-rules/checker.rs already uses it** via:
   - `swissarmyhammer_common::glob_utils::expand_glob_patterns`
   - Called in the `check_with_filters()` method (line ~514)

3. **CLI uses the high-level API** in `check.rs`:
   - Calls `checker.check_with_filters(request)` which internally uses glob_utils
   - No direct glob logic duplication

4. **MCP currently uses CLI wrapper** in `tools/rules/check/mod.rs`:
   - Spawns `sah rule check` as a subprocess
   - Defaults to `**/*.*` pattern (line 90)
   - But this goes through the CLI which uses the checker

### Root Cause
The issue description mentions duplication, but the actual code shows:
- CLI → RuleChecker.check_with_filters() → glob_utils.expand_glob_patterns()
- MCP → CLI subprocess → same path

The real problem is that **MCP uses a CLI wrapper instead of calling the rules crate directly**. This is inefficient but not a duplication of glob logic.

### Implementation Plan

Since glob functionality already exists and is shared via swissarmyhammer-common, the actual work needed is:

1. **Add a convenience function to swissarmyhammer-rules** that wraps the glob logic with rule-appropriate defaults:
   - Default pattern: `**/*.*`
   - Respect .gitignore: true
   - Make it easy for both CLI and MCP to use

2. **Test the new function** to ensure it works correctly

3. **Update documentation** to clarify the function is available for direct use

This approach ensures:
- No duplication (uses existing glob_utils)
- Consistent defaults for rule checking
- Easy to use from both CLI and MCP contexts
- Backward compatible with existing code



## Implementation Complete

### What Was Done

Created a new module `swissarmyhammer-rules/src/glob.rs` with:

1. **`expand_files_for_rules()` function** - A convenience wrapper that:
   - Uses the existing `swissarmyhammer_common::glob_utils::expand_glob_patterns()`
   - Provides rule-specific defaults:
     - Default pattern: `**/*.*` (all files with extensions)
     - Respects .gitignore: `true`
     - Case-insensitive matching
     - Excludes hidden files
     - Max 10,000 files
   - Returns `Result<Vec<PathBuf>>`

2. **`DEFAULT_PATTERN` constant** - Exported for consistency

3. **Public exports** in `lib.rs`:
   - `pub use glob::{expand_files_for_rules, DEFAULT_PATTERN};`

4. **Comprehensive tests** covering:
   - Single patterns
   - Multiple patterns
   - Empty patterns (uses default)
   - Recursive patterns
   - Gitignore respect
   - Hidden file exclusion

### Key Design Decisions

1. **No duplication** - Uses existing `glob_utils` from `swissarmyhammer-common`
2. **Convenience layer** - Provides sensible defaults for rule checking use cases
3. **Backward compatible** - Doesn't change existing CLI or checker behavior
4. **Well-tested** - All 185 tests pass including 7 new tests for the glob module

### Files Modified

- `swissarmyhammer-rules/src/glob.rs` (new file, 196 lines)
- `swissarmyhammer-rules/src/lib.rs` (added module declaration and exports)

### Usage Example

```rust
use swissarmyhammer_rules::expand_files_for_rules;

// Use default pattern (**/*.*)
let files = expand_files_for_rules(&[])?;

// Use specific patterns
let files = expand_files_for_rules(&["**/*.rs".to_string()])?;

// Multiple patterns
let files = expand_files_for_rules(&[
    "src/**/*.rs".to_string(), 
    "tests/**/*.rs".to_string()
])?;
```

### Notes

- The MCP tool currently wraps the CLI, so it already benefits from this shared logic
- Future work could have MCP call `RuleChecker` directly instead of spawning subprocess
- The convenience function makes it trivial to add direct MCP integration later
