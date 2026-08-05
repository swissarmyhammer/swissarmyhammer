---
assignees:
- claude-code
depends_on:
- 01KZ9356Y8XTJ6A28KQCBNFE97
- 01KZ935GJX1YS2EAD7C2HK89AJ
position_column: todo
position_ordinal: ff8280
title: missing-docs runners for Rust and Python, with fixtures
---
Ship the first two tool rules for missing docs, in the `code-hygiene` set.

Both are rule files in `builtin/validators/code-hygiene/rules/` with a `tool` block and `supersedes: missing-docs`. The prompt rule `missing-docs.md` stays unchanged as the fallback and as the rule for languages with no tool rule yet.

Rust tool rule (`missing-docs-rust.md`):
- match: files `**/*.rs`, project_types [rust]. `tool.scope: workspace`.
- run: `cargo clippy --message-format=json -- -W missing_docs` piped through `jq -c` to select diagnostics with code `missing_docs` and emit `{file, line, message}` lines. The pipe is the whole mapping and the whole filter.
- Engine keeps only findings in changed files (workspace scope).
- Doctor: `which cargo-clippy jq`. Install: `rustup component add clippy`.

Python tool rule (`missing-docs-python.md`):
- match: files `**/*.py`, project_types [python]. `tool.scope: files`.
- run: `ruff check --select D1 --output-format json "$@"` piped through `jq -c` to emit `{file, line, message}` lines.
- Doctor: `which ruff jq`. Install: pinned ruff via uv / pipx / brew.

Both:
- Test each pipeline in a terminal first; the frontmatter holds exactly that pipeline.
- Ship `fixtures/<name>.fail.<ext>` (one undocumented public item) and `fixtures/<name>.pass.<ext>` (fully documented).
- Exemptions live in tool config or inline suppressions, not prose.

Acceptance:
- Real-pipeline test on this repo: `review working` with an undocumented pub item reports it from the Rust tool rule, with zero LLM calls for that pair.
- Fixture checks pass in doctor for both tool rules.

#tool-validators