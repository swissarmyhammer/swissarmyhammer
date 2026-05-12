---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8a80
title: PostToolUse validator prompts duplicate file content 3× (tool_input + tool_result + originalFile)
---
**Token waste in every PostToolUse validator prompt for Edit/Write tools.**

## Observed (2026-04-27 qwen test, log lines 7-200)

The `security-rules:input-validation` rule prompt for a single Write of `sample_avp_test.rs` was 10,908 chars, of which ~70% was the same file content rendered three times:

```yaml
tool_input:
  content: |
    use std::time::Duration;
    pub struct RetryClient { ... }       # full file body
    ...
tool_name: Write
tool_result:
  content: |
    use std::time::Duration;
    pub struct RetryClient { ... }       # full file body AGAIN
    ...
  filePath: ...
  originalFile: |
    use std::time::Duration;
    pub struct RetryClient { ... }       # full file body A THIRD TIME
    ...
```

For a fresh Write, all three are identical. For an Edit, `originalFile` would differ but the prompt only needs the diff between old and new — not both copies.

## Why it's wasteful

- 3× tokens for the same data → 3× cost per rule × N rules in the matched ruleset.
- For the 70-line fixture this is ~7KB extra; for a real codebase change touching a 500-line file across multiple validators it's substantial.
- Models burn context window on redundancy that adds zero information.
- Bigger rulesets (`code-quality` has 10 rules) multiply the waste.

## What should be in the prompt instead

A single rendering: either the **post-edit content** (for Write/Edit, where the validator needs the new state) OR the **diff** (which carries both old and new in compact form). Not all three.

Looking at `avp-common/src/turn/diff.rs::prepare_validator_context` (lines 116-146):

```rust
if !(is_diff_tool && has_diffs) {
    return input;        // ← early-returns for Edit/Write WITHOUT non-empty diffs
}
strip_object_fields(&mut input, "tool_result", STRIP_TOOL_RESULT_FIELDS);
strip_object_fields(&mut input, "tool_input", STRIP_TOOL_INPUT_FIELDS);
// Embed diff_text...
```

The function HAS bloat-stripping logic (`STRIP_TOOL_RESULT_FIELDS = ["originalFile", "oldString", ...]`, `STRIP_TOOL_INPUT_FIELDS = ["old_string", "new_string", "replace_all"]`). But it only fires when `has_diffs` is true *and* `is_diff_tool` is true. For a Write where the chain has computed and passed in the diff, both should be true — yet the prompt still contains all three copies. So either:

1. The chain link isn't passing diffs to `prepare_validator_context` for Write (only for Edit?), so the function early-returns and skips the strip.
2. The strip fields list is missing `content` — both `tool_input.content` and `tool_result.content` are full file bodies that should be stripped when a diff is embedded.

## What to change

### 1. Strip both `content` fields from tool_input and tool_result when a diff is embedded

Update `STRIP_TOOL_RESULT_FIELDS` to include `"content"` (full file is redundant once the diff is in `_diff_text`).
Update `STRIP_TOOL_INPUT_FIELDS` to include `"content"` for the same reason.

### 2. Verify the chain link passes diffs for Write

`chain/links/file_tracker.rs` and `chain/links/validator_executor.rs` should be computing a `FileDiff` for every Write/Edit. If Write is being treated as "no prior content, no diff to compute" and the diff list is empty, the bloat-stripping is skipped. Either the diff for a Write should be `--- /dev/null` + full new content, OR the strip should happen unconditionally on Edit/Write (with `_diff_text` set to a "this is a new file" marker if no prior content existed).

### 3. The `originalFile` field is also problematic for Edits

For an Edit, `originalFile` is the pre-edit version. If `_diff_text` is embedded, `originalFile` is redundant with the `-` lines of the diff. Strip it (already in the list — verify it's actually being applied).

## Cross-reference

This dovetails with `01KQ7EQFSMMYW80HC8PQZ2YATV` ("Stop-hook validator prompts must include changed files and diffs"). That task is for Stop hooks where the issue is the *opposite* (no content). This task is for PostToolUse where the issue is too much content. Both share the `prepare_validator_context` short-circuit logic — fixing one likely simplifies the other.

If the implementer takes both at once, that's fine. If separate: this card is the smaller delta because PostToolUse already works end-to-end, just inefficiently.

## Tests

- Unit test in `turn/diff.rs`: pass an Edit-style hook input with non-empty `tool_input.content`, `tool_result.content`, and `tool_result.originalFile`, plus a `FileDiff`. Assert the rendered output has exactly one copy of the file content (in the diff blocks) and zero copies of the bloat fields.
- Snapshot test of an actual PostToolUse rendered prompt (use `RecordingAgent` fixtures from `01KQ7FWFR4V364AYF29DGGBZ87`): assert the prompt size is bounded (e.g. < 1.5× the file's diff size, not 3×).
- Token count assertion: render a known prompt before and after, assert the after is at least 50% smaller for typical Write/Edit inputs.

## Acceptance

- A Write of a 70-line file produces a rule prompt no larger than the diff + rule body + boilerplate — no duplicated copies of the file content.
- Edit prompts contain a unified diff but not separate `originalFile` / new content blocks.
- `cargo test -p avp-common` and `cargo clippy -p avp-common --all-targets -- -D warnings` are clean.

## Why this isn't urgent

PostToolUse validators are *working correctly* — the verdicts produced this morning were grounded and accurate, just with 3× the token spend. This is an efficiency cleanup, not a correctness bug. It does, however, become important once the user is hitting context-window limits or running a 30B model where token cost matters. Land it after `01KQ8CXYMBGN1VTV4S89FGQYCA` (the Stop-hook regression) which is the actual blocker. #avp