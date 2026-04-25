---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff380
project: skills-guide-review
title: Add `compatibility` field to skills with tool prerequisites
---
## What

The Anthropic guide (Chapter 2, "Field requirements") defines the optional `compatibility` field to "indicate environment requirements: e.g. intended product, required system packages, network access needs". Several builtin skills have hard dependencies that warrant this:

- `code-context`, `detected-projects`, `explore`, `map`, `lsp`, `commit`, `coverage`, `deduplicate`, `plan`, `task`, `implement`, `review`, `double-check` — depend on the `code_context` MCP tool.
- `shell` — depends on the `shell` MCP tool / shelltool.
- `kanban`, `task`, `plan`, `finish`, `implement`, `review`, `test`, `test-loop`, `coverage`, `deduplicate` — depend on the `kanban` MCP tool.
- `finish`, `test-loop` — depend on `ralph` MCP tool and Stop-hook capable harness.

Declaring these makes the skill portable across Claude.ai, Claude Code, and API as the guide intends.

## Acceptance Criteria

- [x] Each skill that requires non-builtin tooling has a `compatibility` string (1–500 chars) describing it.
- [x] Content mentions the concrete MCP server or system-tool dependency.

## Tests

- [x] Spot-check three skills to confirm frontmatter is valid YAML after the change.

## Reference

Anthropic guide, Chapter 2 — "Field requirements / compatibility". #skills-guide