---
assignees:
- claude-code
depends_on:
- 01KTC7HVY5VZZWBW7S7W7FBQFY
- 01KTC7GT2DQ84KH443BRC75SHJ
position_column: todo
position_ordinal: '8480'
project: remove-prompts
title: Strip prompt-only types from the rendering crate, keep the engine
---
## What
Reduce `crates/swissarmyhammer-prompts` to only the shared Liquid rendering engine that skills/agents need, deleting prompt-specific concepts. The kept surface is what callers actually use: `PromptLibrary` with `render_text`, `render`, `add`, `add_directory`; the `_partials` adapter; the resolver's partial loading. Everything prompt-feature-specific goes.

CRITICAL — keep these (proven callers): `PromptLibrary::render_text` is called by `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs`, `.../agent/mod.rs`, `.../code_context/detect.rs`, `crates/claude-agent/src/agent.rs`, `crates/mirdan/src/install.rs`, `crates/llama-agent/src/acp/config.rs`. `PromptPartialAdapter` + `get_builtin_partials` feed `{% include %}`/`{% render %}` partials. Do not break these.

Candidates to remove (verify each with `get callgraph` inbound first; remove only those with no remaining non-prompt callers):
- `PromptFilter` (`src/prompt_filter.rs`) — prompt list filtering for the deleted CLI.
- `PromptLoader` (`prompts.rs` ~line 1070) — directory/file prompt loading for the deleted CLI/MCP list.
- `PromptSource` enum (`lib.rs` ~line 51) — only used to tag prompt origins for listing.
- `PromptLibrary::search`, `list`, `list_names`, `list_filtered`, `remove`, `get` IF only the removed CLI/MCP surface used them. Keep `get`/`add` if the partial/skill path needs them.
- File watcher prompt special-casing in `crates/swissarmyhammer-tools/src/mcp/file_watcher.rs` (`is_prompt_file`, `is_any_prompt_file`, prompt-extension constants, `PromptResolver` import) — remove prompt-directory watching; keep any skill/workflow watching.
- `is_prompt_visible` in `crates/swissarmyhammer-common/src/prompt_visibility.rs` — delete the file and its module export once no callers remain.

Keep `parse_frontmatter`/`FrontmatterResult` (`src/frontmatter.rs`) — shared frontmatter parsing; confirm skills use it before assuming otherwise.

## Acceptance Criteria
- [ ] `PromptFilter`, `PromptLoader`, `PromptSource` removed (or whichever have zero remaining callers after CLI/MCP removal).
- [ ] `is_prompt_visible` and `prompt_visibility.rs` deleted; module export removed from `swissarmyhammer-common`.
- [ ] File watcher no longer special-cases prompt files.
- [ ] `render_text`, `render`, partial adapter, and partial loading remain and work.
- [ ] `cargo build` succeeds for `swissarmyhammer-prompts`, `swissarmyhammer-tools`, `swissarmyhammer-common`.

## Tests
- [ ] Keep `skills_rendering_test.rs` / `all_skills_render_test.rs` green (proves the engine survived).
- [ ] Update unit tests in `prompts.rs` that exercise removed APIs — delete tests for removed symbols, keep render tests.
- [ ] `cargo test -p swissarmyhammer-prompts -p swissarmyhammer-tools -p swissarmyhammer-common` is green.

## Workflow
- Use `/tdd`. For each symbol, run `get callgraph` inbound BEFORE deleting; if a non-prompt caller exists, keep it.