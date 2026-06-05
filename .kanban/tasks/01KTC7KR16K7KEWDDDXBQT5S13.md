---
assignees:
- claude-code
depends_on:
- 01KTC7JKNV1ZM5F6JFPK1X7E2G
position_column: todo
position_ordinal: '8680'
project: remove-prompts
title: Remove prompt integration tests, fixtures, and library re-exports
---
## What
Delete tests and re-exports that exist solely to exercise the removed prompt feature. Do NOT touch the ACP/agent "prompt turn" tests (e.g. `crates/acp-conformance/tests/integration/prompt_turn.rs`) — those are ACP protocol "prompt turns", a different concept entirely, KEEP them.

Files to delete (prompt-feature tests):
- `crates/swissarmyhammer/tests/integration/builtin_prompt_rendering.rs` — tests the removed builtin prompts.
- `apps/swissarmyhammer-cli/tests/integration/prompt_performance.rs` (if not already removed by the CLI task).
- `crates/swissarmyhammer-config/` prompt-only tests if any reference removed types.

Files to edit:
- `crates/swissarmyhammer/src/lib.rs` — remove the `swissarmyhammer_prompts` re-export of removed types (`Prompt`, `PromptLoader`, `PromptFilter`, `PromptSource`); keep re-exports of the surviving render engine under the new crate name.
- `crates/swissarmyhammer/tests/test_home_integration.rs` — remove prompt-directory assertions (e.g. `.prompts/` creation), keep skill/workflow ones.
- `crates/swissarmyhammer-mcp-proxy/tests/integration/end_to_end.rs` — remove any assertion that the proxy lists/serves prompts; keep skill/tool assertions.
- `apps/swissarmyhammer-cli/tests/integration/mcp_integration.rs` — remove prompt list/get expectations.

Carefully distinguish: search each test file for `prompt` and classify each hit as (a) sah-prompt-feature -> remove, or (b) ACP/MCP protocol "prompt"/"prompt turn"/system-prompt -> KEEP. When unsure, prefer keeping and leave a TODO.

## Acceptance Criteria
- [ ] `builtin_prompt_rendering.rs` deleted.
- [ ] `swissarmyhammer/src/lib.rs` no longer re-exports removed prompt types.
- [ ] No test references the removed sah-prompt CLI/MCP feature.
- [ ] ACP `prompt_turn` and protocol prompt tests are untouched and still pass.
- [ ] `cargo build --workspace` and affected test crates compile.

## Tests
- [ ] `cargo test -p swissarmyhammer -p swissarmyhammer-cli -p swissarmyhammer-mcp-proxy` is green.
- [ ] `cargo test -p acp-conformance` still green (proves we didn't break ACP prompt turns).

## Workflow
- Use `/tdd` — run the affected test crates first to see what breaks after the type removals, then prune dead prompt tests until green.