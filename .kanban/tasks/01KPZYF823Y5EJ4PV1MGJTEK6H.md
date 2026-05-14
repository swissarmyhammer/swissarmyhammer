---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff480
project: skills-guide-review
title: Add Troubleshooting section to skills with common failure modes
---
## What

The Anthropic guide's Chapter 2 template includes a `## Troubleshooting` section ("Error: [common error]… Cause… Solution…"). A few builtin skills have well-known failure modes that warrant this section:

- `lsp` — "LSP server installed but still degraded" (needs project restart, `compile_commands.json`, `tsconfig.json`).
- `code-context` — "index empty" (startup cleanup not run yet), "search returns nothing for known symbol" (wait for indexing).
- `coverage` — "coverage tool not installed" (fall-through to next tool).
- `commit` — ".kanban changes unstaged" (already covered in body — may just need restructuring).
- `test` — "test hangs" (use timeout).

## Acceptance Criteria

- [x] Each of the listed skills has a `## Troubleshooting` section with Error / Cause / Solution entries for at least its top two failure modes.
- [x] Entries are concrete — actual commands or checks, not vague advice.

## Tests

- [x] Verify each listed error is reproducible (or a documented historical issue) — not invented.

## Reference

Anthropic guide, Chapter 2 — "Writing the main instructions" template (Troubleshooting subsection). #skills-guide