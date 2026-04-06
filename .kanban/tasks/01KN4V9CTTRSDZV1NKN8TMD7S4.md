---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd980
title: Trash directory not excluded from flush_changes scan
---
**swissarmyhammer-store/src/handle.rs:322-337**\n\n`flush_changes()` scans all files in the root directory with the matching extension. The `.trash/` subdirectory is not scanned (since `read_dir` only reads immediate children, not recursively), so this is not a bug currently. However, `changelog.jsonl` could be picked up if the extension were `jsonl`, and any dotfile like `.tmp_*` temp files that survive a crash could be picked up.\n\nThe scan also does not filter out the temporary files created by `atomic_write()` (line 404: `.tmp_{ulid}`). If a crash leaves a `.tmp_*` file behind, and it happens to have the right extension, it would be reported as a new item.\n\n**Severity: nit**\n\n**Suggestion:** Filter out files whose stem starts with `.` or `.tmp_` in the flush scan.\n\n**Subtasks:**\n- [ ] Add filter to skip files with stems starting with `.` or `.tmp_`\n- [ ] Add test: leftover temp file is not reported as a new item\n- [ ] Verify fix" #review-finding