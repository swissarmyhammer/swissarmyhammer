---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffdb80
title: Add tests for get_code_actions live LSP path
---
ops/get_code_actions.rs:76-139 + try_resolve_action:296-321\n\nCoverage: 61.5% (48/78 lines)\n\nUncovered lines: 86, 89-93, 95-96, 101, 106-107, 109-110, 112-113, 121-125, 130, 132, 296, 298, 300, 305-306, 309, 317, 319\n\nTwo functions:\n1. `get_code_actions` live-LSP path - builds params with URI/range, optional kind filter, sends textDocument/codeAction, filters results, resolves actions without edits\n2. `try_resolve_action` - sends codeAction/resolve for actions lacking inline edits\n\nTest scenarios:\n- Mock LSP returning code actions → verify actions returned\n- Null response → empty result\n- Kind filter → only matching kinds returned\n- Action without edits → try_resolve_action called\n- try_resolve_action: action with no kind → None; resolve returning edits → Some(Some(edits)); resolve returning null/empty → None\n\n#coverage-gap #coverage-gap