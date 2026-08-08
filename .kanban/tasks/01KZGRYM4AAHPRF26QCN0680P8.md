---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa580
title: 'no-commented-code: ruff ERA tool rule for Python + a comment-reparse tree-sitter probe'
---
The `no-commented-code` rule is a pure prompt rule today: the LLM reads comments and decides by eye. Move it up the tiers.

Step 1 — Python tool rule (tier 3):
- `no-commented-code-python` in `code-hygiene/rules/`: `ruff check --isolated --no-cache --select ERA --output-format json "$@"` piped through jq. `supersedes: no-commented-code`, match `**/*.py` + project_types [python].
- Inline suppression is `# noqa: ERA001`. State it in the rule body.
- Fail/pass fixture pair. The fail fixture holds a commented-out function and a commented-out block of statements; the pass fixture holds real doc comments, a TODO with prose, and a "don't do this" example — the carve-outs must not fire.

Step 2 — comment-reparse probe (tier 2, every parsed language):
- New tree-sitter probe `commented-code` beside the complexity probe: for each comment block (consecutive comment nodes) in a changed file, strip the comment markers and re-parse the text with the file's own grammar. Report a row when the block is over 5 lines AND the reparse yields statements with a low error-node ratio. Each row: file, line, line count, statement count, error ratio.
- The probe measures; the prompt rule decides. Rewrite `no-commented-code.md` in the complexity-rule style: "the rows are computed for you — report the rows, apply the carve-outs (doc examples, `don't do this` samples, TODO sketches), never scan comments by eye, and an empty list for a parsed file means nothing to report."
- Register in the probe catalog; the catalog row builds FROM the impl; wire `probes: [commented-code]` into the code-hygiene VALIDATOR.md.
- Unit tests per language family: a Rust block comment holding real code yields a row; a rustdoc example does not (doc comments are their own node kinds — exclude them in the probe, not in prose).

Languages without a grammar keep the plain prompt path, as with the complexity rule.

#tool-validators