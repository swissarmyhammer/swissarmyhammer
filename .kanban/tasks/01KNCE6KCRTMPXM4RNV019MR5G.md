---
assignees:
- claude-code
depends_on:
- 01KNCE5AHN1SHZHXKMC38AP5AK
- 01KNCE5ZJ49DZHB4FM7H1747PE
position_column: done
position_ordinal: ffffffffffffffffffdd80
title: Migrate builtin PostToolUse validators to Stop trigger
---
## What

Change the builtin file-content validators from `trigger: PostToolUse` to `trigger: Stop` where appropriate. These validators currently fire on every Write/Edit call, causing slowdown. Moving to Stop makes them batch-validate once per turn.

### Files to modify:
- `builtin/validators/code-quality/VALIDATOR.md` — Change `trigger: PostToolUse` → `trigger: Stop`, remove `match.tools`
- `builtin/validators/test-integrity/VALIDATOR.md` — Same change

### What stays the same (tool-based, must block immediately):
- `builtin/validators/command-safety/VALIDATOR.md` — Stays `trigger: PreToolUse` (must block BEFORE shell execution)
- `builtin/validators/security-rules/VALIDATOR.md` — Stays `trigger: PostToolUse` (catching secrets is a right-now problem, can't wait until Stop)
- All `match.files` patterns remain — they'll now filter against accumulated changed files instead of the single file being edited

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST, then implement.

1. Write/update tests that assert builtin validators have correct trigger types
2. Change frontmatter: `trigger: PostToolUse` → `trigger: Stop` for code-quality and test-integrity
3. Remove `match.tools` section from migrated validators (Stop hooks don't have a tool_name)
4. Keep `match.files` with `@file_groups/source_code` and `@file_groups/test_files` patterns
5. Update validator body descriptions if they reference \"this file\" to reference \"changed files\"

## Acceptance Criteria
- [ ] code-quality and test-integrity trigger on Stop
- [ ] command-safety remains PreToolUse (unchanged)
- [ ] security-rules remains PostToolUse (unchanged — secrets must be caught immediately)
- [ ] File patterns are preserved and work against accumulated changed files
- [ ] Validators only fire when relevant files were changed in the turn

## Tests
- [ ] Unit test asserting each builtin validator's trigger type
- [ ] Verify builtin validator loading still works: `cargo nextest run -p avp-common -- builtin`
- [ ] Run `cargo nextest run` (full suite) to catch regressions"