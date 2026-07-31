---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: ralph check emits decision:"allow", which is not a valid Stop hook value
---
`ralph check` returns `{"decision": "allow"}` when no instruction is active. That value does not exist in the Claude Code Stop hook schema.

Per https://code.claude.com/docs/en/hooks:

> Stop and SubagentStop hooks use top-level `decision` field with value `"block"` to prevent Claude from stopping. To allow the turn to end normally, **omit `decision`** from your JSON or exit 0 without JSON.

`"block"` is the only valid value. Allowing is expressed by the ABSENCE of the field.

So the allow path only works if Claude Code silently ignores an unrecognized `decision` value. That is undefined behavior, not a contract. `"allow"` is a valid value for a *different* event — `PreToolUse` uses `hookSpecificOutput.permissionDecision: "allow" | "deny" | "ask"` — which is likely where it came from.

## Sites

- `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:347` — no active instruction
- `:379` — fallback
- `:367` is correct: `"decision": "block"` with a `reason`

Six tests assert `json["decision"] == "allow"` and must change with it: lines ~607, 676, 1000, 1062, and the two around 976/988 assert `"block"` and stay.

Note `:1039` carries the comment "Must have `decision` and `reason` per Claude Code Stop hook spec" and asserts `decision` is present. For the allow case that assertion is backwards — absence is the contract.

## Required change

1. Emit no `decision` key on the allow path. Return the other diagnostic fields if useful, or an empty object. Do not emit `"allow"`, and do not substitute `"approve"` — that is the legacy `PreToolUse` vocabulary, also wrong for Stop.
2. Keep `{"decision": "block", "reason": "..."}` exactly as is; `reason` is required when blocking.
3. Update the six tests to assert the real contract: block ⇒ `decision == "block"` and `reason` non-empty; allow ⇒ `decision` key ABSENT.
4. Check `HookOutput` in `crates/agent-client-protocol-extras/src/hook_config.rs` — it types `decision` as `Option<String>`, so our own reader accepts any string. Consider a typed enum so an invalid value cannot round-trip silently. This is the same accept-then-silently-discard family as ^1t92gnj, ^t7ebyn8, ^634hqth and ^ezgxksb.

## Acceptance

- `echo '{"session_id":"x"}' | sah tool ralph ralph check --` with no active instruction emits JSON with NO `decision` key.
- With an instruction active it emits `decision: "block"` and a non-empty `reason`.
- A test asserts the absence, not a different string. Prove RED first.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Found by the user reviewing ^634hqth's output. That card made the output JSON and strict-parseable, which was necessary but not sufficient — parseable is not the same as semantically valid, and I verified only the former. #bug #ralph