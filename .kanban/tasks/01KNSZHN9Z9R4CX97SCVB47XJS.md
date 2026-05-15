---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffe580
title: Refactor long production functions to meet 50-line limit
---
Five functions exceed the 50-line code-quality threshold:\n\n1. `workspace.rs::CodeContextWorkspace::open` (~75 lines) - leader/follower branching with retry loop\n2. `indexing.rs::run_indexing_worker` (~90 lines) - main worker loop with rayon parallel processing\n3. `lsp_worker.rs::run_lsp_indexing_loop` (~95 lines) - LSP indexing loop with query, client acquisition, error handling\n4. `lsp_server.rs::spawn_server` (~80 lines) - duplicated if/else branches for PATH vs direct executable\n5. `lsp_communication.rs::send_request` (~70 lines) - request sending with notification skip and wrong-ID handling\n\n`spawn_server` also has high cognitive complexity (4-level nesting, duplicated branches).\n\nExtract helper functions to reduce each below 50 lines. Do not change behavior — refactor only.