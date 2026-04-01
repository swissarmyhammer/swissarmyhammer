---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8180'
title: Enforce GitHub file size limits on attachment add
---
## What

Add file size validation to `AddAttachment::execute()` that enforces GitHub-compatible limits before copying the file. This must happen **before** the copy to avoid wasting I/O on files that will be rejected.

### Size limits
- **Hard limit**: 100 MB (GitHub's absolute max per file) → reject with error
- **Warning threshold**: 50 MB (GitHub recommends staying under this) → succeed but include a warning in the response JSON

### Approach
1. Read file metadata (`std::fs::metadata`) to get size before copying
2. If size > 100 MB → return `KanbanError` with a clear message mentioning the GitHub limit
3. If size > 50 MB → proceed but add `"warning": "File exceeds GitHub's recommended 50MB limit"` to the response
4. Define constants `ATTACHMENT_MAX_BYTES: u64 = 100 * 1024 * 1024` and `ATTACHMENT_WARN_BYTES: u64 = 50 * 1024 * 1024` in `add.rs` (or a shared constants location if one exists)

### Files to modify
- `swissarmyhammer-kanban/src/attachment/add.rs` — size check before copy, warning in response

## Acceptance Criteria
- [ ] Files > 100 MB are rejected with a descriptive error
- [ ] Files > 50 MB but <= 100 MB succeed with a warning field in the JSON response
- [ ] Files <= 50 MB succeed with no warning
- [ ] Size check happens before the file copy (no wasted I/O)
- [ ] Error message mentions GitHub's file size limit so users understand why

## Tests
- [ ] Test: file over 100 MB → error (use a sparse/temp file)
- [ ] Test: file between 50-100 MB → success with warning field present
- [ ] Test: file under 50 MB → success with no warning field
- [ ] Run: `cargo test -p swissarmyhammer-kanban attachment` — all pass