---
assignees:
- claude-code
position_column: todo
position_ordinal: cd80
title: 'hook_config.rs: merge Prompt/Agent handlers, extract route_hook_decision, lowercase errors'
---
Nine findings in `crates/agent-client-protocol-extras/src/hook_config.rs`, split out of ^634hqth. All pre-existing — they sit in the block displaced by that commit's +18-line `parse_hook_stdout` insertion at line 1160, or are merely adjacent to it.

## Consolidation (5 findings, one refactor)

- `PromptHandler` and `AgentHandler` are near-duplicates. Merge into one `EvaluatorHandler { is_agent: bool }`.
- The corresponding `build_handler` match arms merge with them.
- `$ARGUMENTS` is a repeated literal — name it.
- `route_hook_decision` should be extracted; the routing logic is inlined and repeated.

## Hygiene (4 findings)

- Three `#[error(...)]` messages are capitalized. Error `Display` messages are lowercase with no trailing punctuation.
- `HookHandler` should be sealed if it is not meant to be implemented outside the crate.

## Note on line numbers

The engine's cited lines (1179, 1238, 1290, 1471, 1476, 1140, 641, 802, 804, 806) track the pre-image and are offset by roughly 18 lines after the insertion point. Grep for the symbol.

## Acceptance

- One evaluator handler, not two.
- No repeated `$ARGUMENTS` literal.
- Every `#[error(...)]` in the file is lowercase with no trailing punctuation — sweep the whole file, not only the three cited, or a re-review reports the remainder.
- `cargo nextest run -p agent-client-protocol-extras`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Do NOT fold in ^ezgxksb (the `HookSpecificOutput::PreToolUse` serde defect) — that is a behavior bug with its own card, not hygiene. #refactor