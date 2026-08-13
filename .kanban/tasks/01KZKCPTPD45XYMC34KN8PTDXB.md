---
assignees:
- claude-code
position_column: todo
position_ordinal: ffab80
title: rename unused-code-go to dead-code-go
---
Every dead-code tool rule uses the name `dead-code-<lang>`. Go is the one exception: `unused-code-go`.

The name is the only defect. The rule works — it declares `supersedes: dead-code` like the others.

## Work

- Rename `builtin/validators/code-hygiene/rules/unused-code-go.md` to `dead-code-go.md`.
- Set `name: dead-code-go` in the frontmatter.
- Rename the two fixtures: `unused-code-go.pass.go.tmpl` and `unused-code-go.fail.go.tmpl`.
- Find and correct every reference to the old name. Look in `builtin/validators/README.md`, `builtin/validators/code-hygiene/VALIDATOR.md`, and the crate tests.

## Done when

- No file or text says `unused-code-go`.
- The fixture test for `dead-code-go` passes. #tool-validators #objectivity