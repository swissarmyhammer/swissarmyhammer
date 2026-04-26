---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff080
title: Changelog find_entry does O(n) full scan on every undo/redo
---
**swissarmyhammer-store/src/changelog.rs:108-111**\n\n`find_entry()` calls `read_all()` which deserializes the entire JSONL file, then does a linear scan. This is called on every `undo()` and `redo()` in `StoreHandle`. With a long-lived store, the changelog grows unboundedly and every undo/redo pays the full scan cost.\n\n**Severity: warning**\n\n**Suggestion:** Either:\n1. Read the JSONL in reverse (most recent entries first) and stop at first match, since undo targets are usually recent entries.\n2. Maintain an in-memory index of entry IDs to file byte offsets.\n3. At minimum, read lines lazily and short-circuit on match instead of collecting all into a Vec first.\n\n**Subtasks:**\n- [ ] Implement reverse-scan or indexed lookup in `find_entry`\n- [ ] Add a benchmark or test with a large changelog to validate improvement\n- [ ] Verify correctness of undo/redo with the new implementation" #review-finding