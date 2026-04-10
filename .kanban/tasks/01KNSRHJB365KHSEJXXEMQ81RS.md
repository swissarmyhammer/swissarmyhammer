---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb580
title: Add tests for get_rename_edits live LSP path
---
ops/get_rename_edits.rs:66-128\n\nCoverage: 33.3% (6/18 lines)\n\nUncovered lines: 78, 82, 87, 89-93, 98, 100, 104, 114\n\n```rust\nfn get_rename_edits(ctx: &LayeredContext, opts: &GetRenameEditsOptions) -> Result<RenameEditsResult, CodeContextError>\n```\n\nThe live-LSP path uses `lsp_multi_request_with_document` to send two requests: `prepareRename` to check renameability, then `rename` to get workspace edits.\n\nTest scenarios:\n- Mock LSP returning null from `prepareRename` → `can_rename: false`\n- Mock LSP returning non-null from `prepareRename` but null from `rename` → `can_rename: false`\n- Mock LSP returning valid workspace edit from `rename` → `can_rename: true`, edits populated\n\n#coverage-gap #coverage-gap