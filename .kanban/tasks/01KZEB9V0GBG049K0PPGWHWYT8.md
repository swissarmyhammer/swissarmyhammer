---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8f80
title: 'validators: `supersedes` accepts one name or a list'
---
Change `Rule.supersedes` from `Option<String>` to a one-or-many value.

Why: one workspace tool run can replace more than one prompt rule. One `cargo clippy` run finds cognitive complexity and long functions. That rule must supersede `cognitive-complexity` AND `function-length`.

Requirements:
- Frontmatter accepts `supersedes: name` and `supersedes: [a, b]`. Both parse.
- The suppression plan inserts one entry per named rule per matched file.
- Doctor rows and the fallback note show every named rule.
- The README tool-rule section states the list form.
- Existing single-name rules parse unchanged. Add a parser test for each form. #tool-validators