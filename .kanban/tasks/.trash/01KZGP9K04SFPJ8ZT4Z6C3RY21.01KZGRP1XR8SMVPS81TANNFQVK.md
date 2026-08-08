---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa580
title: Prune the orphaned magic-numbers and naming validator sets on init
---
Two validator sets were deleted from `builtin/validators/` without being registered as retired, so `sah init` never prunes them from a user store and the review engine keeps loading them forever.

Evidence on this machine. `~/.validators/` holds 14 sets; `builtin/validators/` holds 12. The two extra sets are:

- `magic-numbers` — deleted in `df808c8cc` ("wip: rule updates for SWEBench"). It carries a `no-magic-numbers` rule.
- `naming` — deleted in `5096f15ac` ("format"). It carries a `naming-consistency` rule.

Neither name is in `crates/mirdan/src/retired_validators.rs::RETIRED_VALIDATOR_SETS`, and neither has a frozen copy under `crates/mirdan/retired-validators/`. `sah init` refreshes the built-in files and leaves everything else in place, so a store that ever held these sets keeps them, and `sah doctor` reports each one as "applies to this project (user)".

The `magic-numbers` orphan now costs correctness, not only tidiness. `code-hygiene` carries a **rule** named `magic-numbers`, so a machine with the orphan runs the old standalone set beside the new rule and reviews the same concern twice with two different rule bodies.

## Work

1. Register both sets in `RETIRED_VALIDATOR_SETS` with a frozen copy of each file under `crates/mirdan/retired-validators/<set>/`, the way the nine merged sets are registered.
2. Extend `crates/mirdan/src/retired_validators.rs::test_retired_sets_are_the_nine_merged_names` — the name and the count both change, so rename the test for what it now asserts.
3. Add the two names to `RETIRED_VALIDATOR_NAMES` in `crates/swissarmyhammer-validators/src/builtin/mod.rs`, which `test_retired_single_rule_validators_no_longer_load` reads.
4. Write a test that proves the pruner removes a set present on disk and absent from the built-ins. The existing tests assert the roster, not the deletion.

## Also worth checking

The general defect is that deleting a built-in set is a silent, unpruned change. A guard test could compare `builtin/validators/` against the union of the built-in and retired rosters and fail when a set is in neither — that turns the next such deletion into a red test instead of a stale user store. #tool-validators