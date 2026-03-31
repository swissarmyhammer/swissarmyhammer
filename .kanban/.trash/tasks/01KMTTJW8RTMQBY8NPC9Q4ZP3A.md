---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: '[NIT] File-read boilerplate duplicated across three CLI merge driver files'
---
`swissarmyhammer-cli/src/commands/merge/jsonl.rs`, `merge/yaml.rs`, `merge/md.rs` (~30 lines each)\n\nAll three CLI driver files contain nearly identical logic for reading the base, ours, and theirs paths from the command line and returning early with a non-zero exit code on I/O error.\n\nExtract a shared helper, e.g.:\n```rust\nfn read_three_files(base: &Path, ours: &Path, theirs: &Path) -> Result<(String, String, String), i32>\n```\nand call it from each driver. This reduces the surface area for diverging error-handling behavior across the three drivers." #review-finding