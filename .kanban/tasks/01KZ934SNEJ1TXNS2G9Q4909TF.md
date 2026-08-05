---
assignees:
- claude-code
depends_on:
- 01KZ9497ZJ3WRCAG1Z6YGT2RRE
position_column: todo
position_ordinal: fd80
title: 'Tool-rule schema: same rule metadata, added tool block'
---
Add the `tool` rule kind. A tool rule is a normal rule file in `rules/` with the same core metadata, plus a `tool` block in frontmatter. There is NO separate runner file, directory, schema, or matcher.

The contract is `builtin/validators/README.md`. Implement that spec exactly.

Work:
- Extend the existing rule frontmatter types in `swissarmyhammer-validators/src/validators/types.rs`. Add optional `tool` and optional `supersedes`.
- The `tool` block is small: `scope` (files|workspace), `run` (a shell script — the pipeline IS the mapping, like skills embed shell), `doctor` (check_command, check_version_command), `install.commands`. There is NO output/format/jq/regex/filter configuration and NO exit.findings key.
- Stdout contract of `run`: one finding per line, either `path:line: message` or a `jq -c` style JSON object `{file, line, message}`. Empty stdout = clean. Exit 0 = judged. Nonzero exit = tool broke.
- Matching REUSES `ValidatorMatch` (`match:` block, `files:` globs, `@file_groups` expansion). Do not build a second matcher. The `project_types` match key is task ^ygt2rre, not this task.
- Allow a rule-level `match` that NARROWS the set's match (intersection). Today rules inherit and cannot override — change this to narrow-only. A rule never matches a file its set does not match.
- Existing prompt rules parse unchanged. A rule with a `tool` block is a tool rule.
- Expose the tool block and supersedes on `list validators` / `get validator` output.

Acceptance:
- A tool rule in any layer (builtin/user/project) loads by the existing precedence.
- A rule-level match intersects the set match, proven by test against the SAME matcher code path the sets use.
- Both stdout line shapes parse into Finding fields, proven by test.
- A malformed tool block reports one clear error and does not break the set.

#tool-validators