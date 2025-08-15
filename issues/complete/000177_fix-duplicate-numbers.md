The issues created appear to have the six digit number in their name twice. I don't want that, I just want it once.

## Root Cause Analysis

The problem is a **race condition** in the `FileSystemIssueStorage::create_issue` method located in `swissarmyhammer/src/issues/filesystem.rs:488-502`.

The issue creation process involves two separate, non-atomic operations:
1. `get_next_issue_number()` - scans directories to find the highest existing number
2. `create_issue_file()` - creates the file with that number

**Race Condition Scenario:**
1. Thread A calls `get_next_issue_number()` and gets `161`
2. Thread B calls `get_next_issue_number()` and also gets `161` (because A hasn't created the file yet)
3. Both threads proceed to create files with the same number
4. Result: Multiple files with `000161` prefix but different suffixes

This is evidenced by existing duplicate files in `issues/complete/`:
- `000161_a.md`
- `000161_b.md`
- `000161_c.md`
- `000161_step.md`

## Proposed Solution

Make the issue creation process atomic by adding synchronization to prevent race conditions. The solution involves:

1. **Add a Mutex to FileSystemIssueStorage** - Wrap the entire create operation in a mutex to ensure only one thread can create issues at a time
2. **Atomic File Operations** - Use atomic file creation with unique temporary names and rename on success
3. **Update the create_issue method** - Ensure `get_next_issue_number()` and `create_issue_file()` are executed atomically

The fix will be implemented in `swissarmyhammer/src/issues/filesystem.rs` to make the `create_issue` method thread-safe.