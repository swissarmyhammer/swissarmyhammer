---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffed80
title: 'Coverage: parse_code_actions + parse_workspace_edit in ops/get_code_actions.rs'
---
crates/code-context/src/ops/get_code_actions.rs

Coverage: incomplete parsing paths

Test the JSON parsing logic for LSP code actions and workspace edits. Construct representative JSON responses (quickfix, refactor, source actions) and verify parse_code_actions produces correct structured output. Test parse_workspace_edit with document changes and text edits. Cover: empty response, single action, multiple actions, action with workspace edit, malformed JSON. #coverage-gap