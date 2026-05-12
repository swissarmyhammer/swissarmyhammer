---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff80
title: Stop-hook validator prompts must include changed files and diffs
---
**Observed (2026-04-27 qwen Stop-hook test run, after `01KQ34TAVZNR7FBYKNFSH0F19T` landed):** every rule in `code-quality` returned a "passed" verdict with messages like:

- `cognitive-complexity` → "No code content provided for analysis"
- `function-length` → "No file content was provided to analyze"
- `missing-docs` → "No file content was provided to validate for missing documentation"
- `no-magic-numbers` → "No code provided to validate."
- `no-string-equality` → "No code content was provided for analysis"
- `no-test-cheating` → "No test files were provided for inspection"

The rules weren't wrong — they correctly described what they saw. The rendered rule prompt for Stop hooks is missing both the list of changed files AND the diffs. Validators have no idea what to validate against.

## Where the data flow breaks

Two related defects, both stemming from the per-rule-session refactor (`01KQ34TAVZNR7FBYKNFSH0F19T`) not fully completing the "self-contained per-rule prompt" goal:

### Defect 1: `prepare_validator_context` short-circuits for Stop hooks

`avp-common/src/turn/diff.rs::prepare_validator_context` (line 116-146):

```rust
let has_diffs = diffs.is_some_and(|d| !d.is_empty());
let is_diff_tool = DIFF_TOOLS.contains(&tool_name.as_str());

if !(is_diff_tool && has_diffs) {
    return input;        // ← early return: diffs never embedded for Stop
}
```

`DIFF_TOOLS = ["Edit", "Write"]`. For Stop-hook input there is no `tool_name` field — Stop is a turn-end event, not a tool event — so `is_diff_tool = false` and the function returns the input untouched, dropping the `diffs` argument on the floor.

The chain link upstream knows this is going to happen: `chain/links/validator_executor.rs::prepare_diffs` (line 130) deliberately does NOT call `prepare_validator_context` for Stop hooks; it passes raw diffs through to the runner expecting the runner to embed them. The runner does try — `runner.rs::execute_rulesets` (line 1172-1177):

```rust
let context_clone = if raw_diffs.is_some() {
    let filtered_diffs = filter_diffs_for_ruleset(raw_diffs, ruleset);
    crate::turn::prepare_validator_context(context.clone(), filtered_diffs.as_deref())
} else {
    context.clone()
};
```

— but `prepare_validator_context` rejects the call because the input has no `tool_name`. The diffs ARE filtered per-ruleset and reach this point, then disappear inside the function.

### Defect 2: changed-files list is never rendered into the rule prompt

`runner.rs::execute_ruleset` accepts `changed_files: Option<&[String]>` (line 898) and the docstring on lines 900-909 explicitly justifies ignoring it:

> "the per-rule prompt rendering does not need them: the `context` argument is already pre-populated by `prepare_validator_context` (see `turn::diff`) with the rendered hook context, including the changed file list and any diff blocks."

That claim is wrong on two counts:
1. Per Defect 1, the diff blocks aren't actually in the context for Stop hooks.
2. `prepare_validator_context` *never* embeds a "changed file list" — it only embeds `_diff_text` (the unified-diff content). The list of paths, separate from diff content, is never rendered anywhere.

The rule template at `builtin/prompts/.system/rule.md` (29-66) confirms: `{{ rule_name }}`, `{{ rule_description }}`, `{{ rule_severity }}`, `{{ hook_context }}`, `{{ rule_body }}`. No `{% if changed_files %}` section.

## What the task description for the per-rule-session work explicitly required

From `01KQ34TAVZNR7FBYKNFSH0F19T`:

> "Each rule's prompt should be self-contained: hook context, **changed files**, the rule body, response-format instructions."

Changed files were called out explicitly. They didn't land. The reviewer didn't catch it because the acceptance criterion was "each rule sees a distinct session_id" — covered isolation, not content.

## What to change

### 1. Fix `prepare_validator_context` to honor caller-supplied diffs

The `is_diff_tool` short-circuit is the wrong gate. The caller (validator_executor for PostToolUse, runner for Stop) is the one that knows whether diffs are relevant — it already filtered them per ruleset. If the caller supplied non-empty diffs, embed them. The check should collapse to:

```rust
let Some(diffs) = diffs.filter(|d| !d.is_empty()) else {
    return input;
};
// Edit/Write: also strip bloated tool_input/tool_result fields.
// Other inputs (Stop): just embed _diff_text.
if DIFF_TOOLS.contains(&tool_name.as_str()) {
    strip_object_fields(&mut input, "tool_result", STRIP_TOOL_RESULT_FIELDS);
    strip_object_fields(&mut input, "tool_input", STRIP_TOOL_INPUT_FIELDS);
}
embed_diff_text(&mut input, diffs);
input
```

The bloat-stripping is the only piece that's specifically about Edit/Write — the diff embedding itself is universal. Restructure so embedding always happens when the caller passed diffs, and stripping is conditional on it being an edit-tool input.

### 2. Add `changed_files` to the rule template and thread it through

Update `avp-common/src/validator/executor.rs::render_rule_prompt` to accept `changed_files: Option<&[String]>` and `executor.rs::RulePromptContext` to carry them. Update the call site in `runner.rs::execute_rule_in_fresh_session` (around line 1098) to pass them in.

Update `builtin/prompts/.system/rule.md`:

```liquid
{% if changed_files and changed_files != empty %}
## Files Changed This Turn

The following files were modified during this turn. Focus your analysis on these:

{% for f in changed_files %}- {{ f }}
{% endfor %}
{% endif %}
```

Place it before the rule body so the model sees the file list as orientation.

### 3. Update the misleading comment in `execute_ruleset`

Lines 900-909 currently claim `changed_files` is intentionally unused because the context is pre-populated. After (1) and (2) land, this is no longer true — `changed_files` is consumed by the per-rule prompt builder. Replace the comment with one describing what the parameter is actually for.

### 4. Tests

- Unit test for `prepare_validator_context`: pass a Stop-hook-style input (no `tool_name`) plus a non-empty `Vec<FileDiff>`. Assert the returned value contains `_diff_text` matching the diff content. Today this test would fail.
- Unit test for `render_rule_prompt`: pass `changed_files = Some(&["foo.rs", "bar.rs"])`. Assert the rendered output contains both filenames.
- Integration test (`avp-common/tests/`): construct a Stop-hook context with two changed files and a diff for each, run `ValidatorRunner::execute_ruleset` against a recorded `PlaybackAgent`, capture the prompt that was sent (via `RecordingAgent`), assert the prompt contains the changed-files list and the diff content.

## Acceptance

- A Stop-hook `code-quality` run where the user edited `swissarmyhammer-common/src/sample_avp_test.rs` produces a rule prompt containing:
  - The path `/Users/wballard/.../sample_avp_test.rs` in a `## Files Changed This Turn` section.
  - The full unified diff for that file in a fenced code block, embedded via `_diff_text`.
- Validators no longer reply "No code content provided for analysis." Specifically, `no-magic-numbers` should flag the magic numbers in the test fixture (8675309, 8421, 4096, 5000, etc.); `no-hard-code` should flag the `return 42`; `missing-docs` should flag the undocumented `pub` items.
- `cargo test -p avp-common` and `cargo clippy -p avp-common --all-targets -- -D warnings` are clean.

## Pairs with

- `01KQ35MHFJQPMEKQ08PZKBKFY0` (validator tools / in-process MCP fallback). Once tools are wired AND prompts include diffs/file lists, validators have both ways to ground themselves: read the file via tool calls if they need more, OR work directly from the supplied diff if it's enough. Today they have neither.

## Why this is its own task

The per-rule-session refactor task explicitly listed "changed files" as required content but didn't land it. Reviewer didn't catch it. The fix is small and focused — patching one short-circuit and one prompt-render parameter — and shouldn't be folded into the next refactor. #avp