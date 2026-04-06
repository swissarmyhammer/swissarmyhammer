---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9180
title: Attachment field shows empty drop target even when attachments exist on disk
---
## What\n\nThe attachment field in the inspector shows \"Drop files here\" instead of the attachment list, even when attachment files exist in `.kanban/tasks/.attachments/`.\n\n**Root cause:** The task entity YAML files have `attachments: []` — the filenames aren't being stored when attachments are added.\n\n**MANDATORY: TDD — write the failing test FIRST, then implement the fix.**\n\n**Investigation needed:** Trace the attachment add flow in Rust.\n\n**Files to modify:**\n- `swissarmyhammer-entity/src/context.rs` — fix the attachment add flow to persist filenames\n- Tests written FIRST proving the round-trip works\n\n**MANDATORY: All dispatch via useDispatchCommand. Run tsc --noEmit before done.**\n\n## Acceptance Criteria\n- [ ] Rust test written FIRST: add attachment → read entity → attachments field contains filename (RED then GREEN)\n- [ ] After dropping a file, the attachment appears in the list\n- [ ] The task YAML `attachments:` field contains the attachment filenames\n\n## Tests\n- [ ] New Rust test: attachment add round-trip (RED first, then GREEN)\n- [ ] `cargo test -p swissarmyhammer-entity` — all pass\n- [ ] Manual: drop file on attachment field, verify it appears"