---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb780
title: 'Integration test: Stop validator prompt contains changed files and diff blocks'
---
## What

Existing chain tests either skip the agent (`AVP_SKIP_AGENT`) or test output format but don't assert what context the validator actually received. No test verifies that the rendered prompt sent to the LLM includes the changed files list and fenced diff blocks.

### Test to write:
In `avp-common/tests/stop_validators_integration.rs` (or new file):

1. Create a temp dir, set up turn state with changed files and sidecar diffs
2. Create a test Stop RuleSet on disk
3. Use a playback agent fixture (or intercept the prompt)
4. Execute the Stop chain
5. Capture the prompt sent to the agent — assert it contains:
   - The changed file paths (e.g. `src/main.rs`)
   - Fenced diff blocks (` ```diff `)
   - The diff content matching what was written to sidecar

If playback agent fixtures don't expose the prompt, an alternative is to:
- Call `ValidatorRenderContext::render()` directly with known inputs
- Or call `render_hook_context()` with a prepared context value containing `_diff_text`
- Assert the output string contains expected YAML + diff blocks

### Approach (TDD):
Use `/tdd` workflow. Write the failing test FIRST, then fix if any wiring is broken.

## Acceptance Criteria
- [ ] Test asserts the actual prompt/context content, not just output format
- [ ] Changed files list appears in rendered context
- [ ] Diff text appears as fenced diff blocks

## Tests
- [ ] `test_stop_validator_prompt_contains_changed_files_and_diffs`
- [ ] Run `cargo nextest run -p avp-common`"