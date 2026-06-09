---
assignees:
- claude-code
depends_on:
- 01KTBNPQQJ45YWGVSY10NR4RX9
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffb80
project: local-review
title: Migrate safety rules to focused review-time validators (no-secrets, injection, command-safety, test-integrity)
---
## What
With hooks retired, the safety/integrity rules no longer fire in real time — they become review-time validators. Re-home the existing `builtin/validators/security-rules/` (input-validation, no-secrets), `builtin/validators/test-integrity/` (no-test-cheating), and the command-safety rules into the no-`trigger` format as FOCUSED, one-concern validators (same split philosophy as the quality validators):

| Validator | Concern | `probes` | severity |
|-----------|---------|----------|----------|
| `no-secrets` | secret-looking literals committed to code | (none) | blocker |
| `injection` | unvalidated input → SQL/command/XSS/template injection | (none) | blocker |
| `command-safety` | dangerous shell patterns in scripts/commands in the diff | (none) | blocker |
| `test-integrity` | test cheating + the `no-hard-code` "return 42 to pass a test" rule (moved here from code-quality) | (none) | blocker |

- All are in-file judgments → no probes.
- **Document the real-time→review-time shift, especially for `command-safety`:** as a hook it blocked a *proposed* command (`rm -rf /`) before execution; as a review validator there is no proposed command, so it reviews **shell scripts / commands embedded in the diff** for dangerous patterns — narrower, and after-the-fact. The rule body must say this plainly so it's not mistaken for a real-time guard. Same note applies generally: confirmed secrets/injection now stop work via the review-column gate (blocker), not a pre-execution block.

## Acceptance Criteria
- [ ] `no-secrets`, `injection`, `command-safety`, `test-integrity` exist as separate focused validators in the new format, blocker severity, no `trigger`, no probes.
- [ ] `no-hard-code` lives in `test-integrity`; the old `security-rules`/`test-integrity` multi-rule sets are re-homed.
- [ ] `command-safety` (and a general note) document the real-time→review-time change.
- [ ] `check validators` reports OK for the migrated set.

## Tests
- [ ] Loader/parse test: the migrated validators parse and match expected paths; `check validators` OK; `cargo test -p swissarmyhammer-validators` green.
- [ ] (Behavioral proof — a planted secret producing a confirmed blocker through the engine — lives in the end-to-end task.)

## Workflow
- Use `/tdd` for the parse/`check validators` tests; the rest is content migration. Reuse the existing rule wording; only reformat, split, and re-home. Depends on the focused-validators task so the authoring pattern already exists.