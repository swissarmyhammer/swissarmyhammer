---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff8f80
title: 'hookable_agent.rs: new HookEvent variants have required String fields where Optional makes more sense'
---
agent-client-protocol-extras/src/hookable_agent.rs:125-182

The new `HookEvent` variants use required (non-Optional) `String` fields for properties that can legitimately be absent at the protocol level:

- `Elicitation::mcp_server_name: String` — the input type `ElicitationInput` has `mcp_server_name: Option<String>`
- `Elicitation::message: String` — `ElicitationInput` has `message: Option<String>`
- `ElicitationResult::action: String` — `ElicitationResultInput` has `action: Option<String>`
- `InstructionsLoaded::file_path: String` — `InstructionsLoadedInput` has `file_path: Option<String>`
- `ConfigChange::source: String` — `ConfigChangeInput` has `source: Option<String>`
- `WorktreeCreate::worktree_path: String`, `branch_name: String` — both optional in the input type

This creates an impedance mismatch: constructors of `HookEvent` must provide a fallback string (e.g., `""`) for missing protocol fields, losing the optionality information. Matchers that use `matcher_value()` will get `Some("")` instead of `None` for absent fields, which may trigger unintended hooks.

Suggestion: mirror the optionality from the input types in the `HookEvent` variants. #review-finding