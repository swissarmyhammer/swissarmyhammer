---
assignees:
- claude-code
position_column: todo
position_ordinal: cb80
title: Partial hookSpecificOutput silently becomes Allow — Option fields lack serde(default)
---
`HookSpecificOutput::PreToolUse` in `crates/agent-client-protocol-extras/src/hook_config.rs` declares its fields as `Option<...>` **without** `#[serde(default)]`. Serde therefore requires each key to be present. A hook emitting a partial `hookSpecificOutput` fails to deserialize the whole document, and the caller falls back to `HookDecision::Allow`.

Net effect: a hook that meant to block runs, reports success, and does nothing. Same failure shape as the bug fixed on ^634hqth, where `interpret_exit_0_stdout` accepted JSON only and silently allowed anything else.

Found while implementing ^634hqth. Deliberately not fixed there — that card's scope was the ralph output format.

## Why this class keeps biting

Three defects in this family have now been found in one session:

- `add task` accepted a `tags` array, returned `ok: true`, applied nothing (^1t92gnj)
- skill install accepted a `hooks:` block and dropped it with no error (^t7ebyn8)
- a YAML-answering hook was accepted, warned about, and silently allowed (^634hqth)

Each one reported success while discarding the caller's input. Worth considering whether the hook-config types should deny unknown fields and surface a parse error rather than degrading to `Allow`, since `Allow` is the permissive direction — a malformed *deny* becomes a *permit*.

## Required change

1. Add `#[serde(default)]` to the `Option` fields on `HookSpecificOutput::PreToolUse`, and audit the sibling variants for the same omission rather than assuming `PreToolUse` is the only one.
2. Decide deliberately what a genuinely unparseable `hookSpecificOutput` should do. Silently allowing is the dangerous default; at minimum it must be logged at a level that is visible, and the decision recorded in the type's docs.

## Acceptance

- A partial `hookSpecificOutput` — one field present, the rest absent — deserializes and its decision is honored. Prove RED first.
- A test that a malformed `hookSpecificOutput` does not silently permit: either it errors, or it allows with an explicit, asserted log.
- The sibling variants are covered too, or the audit records why they need nothing.
- `cargo nextest run -p agent-client-protocol-extras`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug