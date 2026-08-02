---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz1d8qaamvwyhcmppc91gm1a
  text: |-
    Research + discovery before writing code.

    Checked the claim on the card by writing the RED test first, as required. Result: the test passed immediately, before any production change. `serde` 1.0.228's derive already treats a missing key for a field literally typed `Option<T>` as `None`, with no `#[serde(default)]` needed — verified with a scratch test on `HookSpecificOutput::PreToolUse` (JSON, bare tag only), on the whole `HookOutput` (existing `hook_stdout_parses_json` test already exercises this), and on the YAML path (`serde_yaml_ng` behaves the same). So the card's technical premise — "Serde therefore requires each key to be present" — does not hold against the current serde version; a partial `hookSpecificOutput` was already deserializing and being honored before this change.

    This is not a true conflict (no rule contradicts another, nothing fails to compile, nothing fights a documented contract) — it is a factual claim in the card that turned out to be wrong when tested. The card's two required changes are still concrete, correct asks independent of that premise, so I implemented both:

    1. Added explicit `#[serde(default)]` to every `Option` field on every `HookSpecificOutput` variant (PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit, Stop, SessionStart, Notification) — the full sibling audit the card asked for, not just PreToolUse. This makes the contract explicit rather than resting on serde's implicit Option-defaulting heuristic (which is keyed on the field's type being spelled literally as `Option<...>`, and would silently stop applying if that spelling ever changed).
    2. Documented the deliberate decision for genuinely unparseable `hookSpecificOutput` directly on the `HookSpecificOutput` doc comment: an unrecognized `hookEventName` tag is a hard deserialize error from `parse_hook_stdout`, and the caller (`interpret_exit_0_stdout`) already logs that failure at `tracing::warn!` (command, error, raw stdout) before falling back to `Allow` — visible, not silent.
  timestamp: 2026-08-02T14:15:25.258764+00:00
- actor: claude-code
  id: 01kz1d90nw78dqfc8mw3rtca1n
  text: |-
    ### implement — changed
    - evidence: crates/agent-client-protocol-extras/src/hook_config.rs — added `#[serde(default)]` to every `Option` field on all 7 `HookSpecificOutput` variants (PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit, Stop, SessionStart, Notification); documented the deliberate malformed-input decision on the type's doc comment; added 7 tests (partial `PreToolUse` deserializes and its decision is honored end to end, bare-tag/no-fields deserializes for every sibling variant, JSON and YAML paths, and an end-to-end `#[tracing_test::traced_test]` test asserting the visible `warn` log before the `Allow` fallback on a genuinely malformed tag). Discovery: the RED test passed immediately pre-fix — serde 1.0.228 already defaults missing `Option<T>` fields to `None` without `#[serde(default)]`, so the card's stated defect did not reproduce; implemented the required changes anyway since they are correct, explicit hardening independent of that premise (recorded in full in the prior comment). `cargo nextest run -p agent-client-protocol-extras`: 309/309 passed. `cargo fmt --check -p agent-client-protocol-extras`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean, 0 warnings/errors.
    - next: ready for review
  timestamp: 2026-08-02T14:15:34.844847+00:00
- actor: claude-code
  id: 01kz1dawxk6gqw70qhfrd1nbeb
  text: |-
    ### test — green
    - evidence: cargo nextest run -p agent-client-protocol-extras — 309 passed, 0 failed, 0 skipped; cargo fmt --all -- --check — clean, exit 0; cargo clippy --workspace --all-targets -- -D warnings — clean, exit 0. Confirmed every Option field on all seven HookSpecificOutput variants (PreToolUse x4, PostToolUse, PostToolUseFailure, UserPromptSubmit, Stop, SessionStart, Notification) has #[serde(default)]. Confirmed partial-JSON coverage: partial_pre_tool_use_hook_specific_output_deserializes, partial_pre_tool_use_hook_specific_output_decision_is_honored, sibling_variants_with_no_fields_set_deserialize (bare tag, no optional fields, for PostToolUse/PostToolUseFailure/UserPromptSubmit/SessionStart/Notification/Stop), and malformed_hook_specific_output_tag_is_a_parse_error_not_a_silent_allow all exercise missing-field documents, not only full ones.
    - next: ready for commit
  timestamp: 2026-08-02T14:16:36.531144+00:00
position_column: doing
position_ordinal: '8380'
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