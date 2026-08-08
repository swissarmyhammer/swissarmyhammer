---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa580
title: 'no-commented-code: ruff ERA tool rule for Python + a comment-reparse tree-sitter probe'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT downgrade this to a probe that feeds a prompt rule. The parse verdict decides. Zero LLM calls for this rule on matched files is the acceptance bar.
- Do NOT keep the prompt rule running for matched files. Supersede it.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output.

## The work

Correction: the first version of this card made the tree-sitter reparse a probe feeding the prompt rule. That was wrong. The verdict is objective — comment content either parses as code for the file's language or it does not. No LLM reads it.

1. New sah op (in swissarmyhammer-sem / code-context, where the grammar roster lives): for each file argument, extract comment blocks with tree-sitter, strip the comment markers, and re-parse the text with the file's own grammar. A block over 5 lines whose reparse yields 2 or more statements/items with an error-node ratio under a fixed threshold IS commented-out code. Emit one line per block: `path:line: commented-out code (<n> lines parse as <language>)`. Exclude doc-comment node kinds structurally — a documentation example is never a finding, by grammar node kind and not by prose.

2. Tool rule `no-commented-code-parsed` in `code-hygiene/rules/`: files scope, `run: sah <op> "$@"`, `supersedes: no-commented-code`. The match lists the extensions the grammar roster covers, explicitly. A language without a grammar keeps the prompt rule — fallback by match, the designed degradation. Doctor: the tool is sah itself, so `check_command` names the sah binary; no install commands. Resolve the binary the way the engine invokes itself (env or current_exe), never a bare PATH assumption.

3. The exemption contract is structural, never prose: put intentional example code in a doc comment, or keep the block at 5 lines or fewer. State this in the rule body.

4. Drop the separate ruff ERA rule from the earlier plan — one owner per finding, and the reparse op covers Python. Note ERA in the rule body as the cross-check used to validate the Python fixtures.

5. Fixtures: fail fixtures hold a commented-out function of 6+ lines for at least Rust, Python, and TypeScript; pass fixtures hold a doc-comment example, a TODO with prose, and a short 2-line snippet. Unit tests in the sem crate for the extractor per language family. Extend the shipped-rules acceptance test. Acceptance: a review of a file whose only defect is a commented-out block reports it with zero LLM validator calls for that rule.

#tool-validators #objectivity