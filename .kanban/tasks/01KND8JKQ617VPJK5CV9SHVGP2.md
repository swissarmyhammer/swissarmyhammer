---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffee80
title: Remove command-safety builtin validator and associated tests
---
## What

Remove the `command-safety` builtin validator entirely. It triggers on every PreToolUse shell command, spawns an LLM agent to evaluate safety, and significantly slows down the dev experience.

### Files to remove:
- `builtin/validators/command-safety/` directory (VALIDATOR.md + rules/safe-commands.md)

### Files to modify:
- `avp-common/src/builtin/mod.rs` — Remove any tests asserting command-safety exists or has PreToolUse trigger
- `avp-common/tests/pre_tool_use_integration.rs` — Remove or update tests that reference command-safety
- `avp-common/tests/stop_validators_integration.rs` — Remove any references to command-safety
- `avp-common/tests/ruleset_integration.rs` — Remove any references to command-safety
- Any other test files referencing `command-safety` or `safe-commands`

### Approach (TDD):
1. Find all references to command-safety/safe-commands across the codebase
2. Remove the builtin validator directory
3. Update all tests — remove assertions about command-safety, update counts of builtin validators
4. Run full test suite

## Acceptance Criteria
- [ ] `builtin/validators/command-safety/` directory removed
- [ ] No test references to command-safety or safe-commands remain
- [ ] Builtin validator count tests updated
- [ ] `cargo nextest run` passes
- [ ] `cargo clippy -- -D warnings` clean

## Tests
- [ ] Run `cargo nextest run` — all pass
- [ ] Grep for 'command-safety' and 'safe-commands' returns zero hits in .rs files"