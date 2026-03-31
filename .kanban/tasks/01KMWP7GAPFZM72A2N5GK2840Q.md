---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff9180
title: Add tests for merge_md ParseFailure and edge cases
---
swissarmyhammer-merge/src/md.rs:43-95\n\n`pub fn merge_md(base, ours, theirs, opts) -> Result<String, MergeError>`\n\n8 tests cover clean merges, conflicts, and no-frontmatter cases. Missing:\n- `MergeError::ParseFailure` when frontmatter contains invalid YAML\n- One side has frontmatter, other sides don't (asymmetric frontmatter presence — partially covered by `ours_adds_frontmatter_to_plain_file` but not the theirs-adds case)\n- Empty frontmatter on all sides merges cleanly (all fields removed yields None)\n- JSONL changelog resolution through frontmatter (the opts.jsonl_path path is never exercised in md tests) #coverage-gap