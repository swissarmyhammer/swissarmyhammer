---
position_column: done
position_ordinal: k2
title: 'Fix test_question_tool_infer_summary_from_empty: assertion failed'
---
Test in swissarmyhammer-tools/src/mcp/tools/questions/mod.rs:266 fails with 'assertion failed: result.is_ok()'. The question tool infer summary operation returns an error when given empty input. #test-failure