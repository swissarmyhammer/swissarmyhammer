---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffbd80
title: 'Integration test: per-ruleset diff filtering end-to-end at Stop'
---
## What

`filter_diffs_for_ruleset` is unit-tested, but there's no integration test where two rulesets with different file patterns both run at Stop and each gets only its matching diffs.

### Test to write:
In `avp-common/tests/stop_validators_integration.rs` (or new file):

1. Create a temp dir with two files: `src/main.rs` and `lib/helper.py`
2. Write sidecar diffs for both files via `TurnStateManager::write_diff()`
3. Create two test RuleSets on disk:
   - `rust-only` with `match.files: [\"*.rs\"]`, trigger: Stop
   - `python-only` with `match.files: [\"*.py\"]`, trigger: Stop
4. Load both rulesets into a `ValidatorLoader`
5. Build the Stop chain and execute (with `AVP_SKIP_AGENT` or playback agent)
6. Intercept or inspect: verify that each ruleset's execution received only its matching diffs

The challenge is observing what context each ruleset received. Options:
- Use a playback agent fixture and assert on the prompt sent to each ruleset
- Or: test at the runner level — call `execute_rulesets` directly with known diffs and rulesets, verify the `filter_diffs_for_ruleset` is applied by checking the context value passed through

### Approach (TDD):
Use `/tdd` workflow. Write the failing test FIRST, then fix if any wiring is broken.

## Acceptance Criteria
- [ ] Test proves a `*.rs` ruleset does NOT see `.py` diffs and vice versa
- [ ] Test uses real RuleSet loading (not mocked match criteria)

## Tests
- [ ] `test_stop_rulesets_receive_only_matching_diffs`
- [ ] Run `cargo nextest run -p avp-common`"