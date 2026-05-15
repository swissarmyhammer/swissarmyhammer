---
depends_on:
- 01KKF7WPK1JFZEXSJT7VPKHADX
position_column: done
position_ordinal: ff9380
title: Fix `detected-projects.md` partial to reference code_context tool
---
## What

The partial `builtin/_partials/detected-projects.md` tells agents to call `detect projects` on the **treesitter** tool, which no longer exists. Update it to reference the **code_context** tool.

## Changes

Update `builtin/_partials/detected-projects.md`:
- Change the instruction from \"call the treesitter tool\" to \"call the code_context tool\"
- Update the JSON example to make it clear it's a code_context op

## Acceptance Criteria

- Partial references code_context, not treesitter
- Generated `.skills/` and `.agents/` files reflect the change after `sah init`

## Tests

- `grep -r treesitter builtin/_partials/detected-projects.md` returns nothing