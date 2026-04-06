---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff480
title: 'Coverage: lsp_callees_of in layered_context.rs'
---
crates/code-context/src/layered_context.rs

Coverage: 0% (0/3 lines)

Simple DB query that returns callees from lsp_call_edges. Populate the database with known call edges, then verify the function returns the correct callee symbols. Cover: symbol with callees, symbol with none. #coverage-gap