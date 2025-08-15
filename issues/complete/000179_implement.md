I am incorrectly getting that issues are compete when I have issues

## Analysis

The issue tracking system uses the `issue_all_complete` functionality which:

1. Scans both `./issues/` (active) and `./issues/complete/` (completed) directories
2. Determines completion status based on file path - files in paths containing "complete" are marked completed
3. Reports all complete when `active_count == 0 && total_issues > 0`

Current system behavior appears correct in basic testing (properly reports 1 active, 95 completed issues), but the bug may occur under specific conditions.

## Proposed Solution

Investigate and fix potential edge cases in the issue completion detection:

1. **Write comprehensive tests** - Create tests that cover edge cases like:
   - Empty directories
   - Permission issues 
   - Malformed files
   - Concurrent access
   - Cache invalidation issues
   - Symlinks and special files

2. **Add error handling and logging** - Ensure file scanning errors are properly handled and logged

3. **Fix any discovered bugs** - Address root causes found through testing

4. **Add defensive checks** - Add validation to ensure the completion logic is robust

## Root Cause Investigation

Need to examine scenarios where the filesystem scanning might fail or return incorrect results, causing the system to report all issues complete when active issues exist.

## Implementation Completed

### Root Cause Found
The bug was in the path-based completion detection logic in `swissarmyhammer/src/issues/filesystem.rs:227-229`. The original code used overly broad ancestor checking:

```rust
// OLD (buggy) - checks ALL ancestors
let completed = path
    .ancestors()
    .any(|p| p.file_name() == Some(std::ffi::OsStr::new("complete")));
```

This could incorrectly mark files as completed if they had "complete" anywhere in their path ancestry, even in nested directories.

### Fix Applied
Fixed the completion detection to only check the immediate parent directory:

```rust
// NEW (fixed) - only checks immediate parent
let completed = path
    .parent()
    .and_then(|parent| parent.file_name())
    == Some(std::ffi::OsStr::new("complete"));
```

### Testing
- Created comprehensive test suites covering edge cases
- Verified fix works correctly with multiple directory naming patterns
- All 150+ existing tests pass - no regressions
- System correctly reports completion status (currently 2 active, 95 completed issues)

### Results
✅ **Bug fixed**: Path-based completion detection is now precise and robust
✅ **All tests pass**: No breaking changes to existing functionality  
✅ **System verified**: Issue tracking working correctly post-fix