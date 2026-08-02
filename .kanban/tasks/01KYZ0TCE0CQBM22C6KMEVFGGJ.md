---
assignees:
- claude-code
position_column: todo
position_ordinal: dd80
title: Singular tag key docs repeat the corrected one-element alias claim
---
`^n36mc1q` corrected two places that described the singular `assignee` key as "a one-element alias". The identical phrasing survives for the singular `tag` key:

- `crates/swissarmyhammer-kanban/src/dispatch.rs:250`
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/description.md:26`

Both blame to `74d0cacc48` (2026-07-30), not to the `^n36mc1q` commits, so the reviewer correctly ruled them out of that task's scope.

## This is docs only — the code is already right

`tag_refs` routes the singular `tag` key through `list_param`, exactly as `assignee` now does. So `tag: ["urgent"]` already works. The defect is that the docs describe a narrowing the code does not perform, which is the mirror image of the `^n36mc1q` finding: there the doc promised shape tolerance the code lacked, here the doc implies a restriction the code does not impose.

Do not "fix" this by changing `tag_refs`. Verify first that it calls `list_param` for both keys, then correct only the prose.

## Required change

Reword both places the way `^n36mc1q` reworded the `assignee` text: the singular names the key, it does not narrow the shape. Match that wording so the two params read consistently.

`description.md` is the tool's user-facing contract, so its wording matters more than the internal doc comment.

## Acceptance

- Neither place claims the singular `tag` key takes only one element.
- A test proves `tag: ["urgent"]` and a stringified array both work under the singular key — if no such test exists, add one, because the claim in the docs should rest on a fixture rather than on reading the code.
- No behavior change; `tag_refs` is untouched and every existing test passes unedited.

Found by the review of `24b5d687e` while closing ^n36mc1q. #bug #kanban