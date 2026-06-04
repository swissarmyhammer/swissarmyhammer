---
assignees:
- claude-code
depends_on:
- 01KT7A301VGSDQ0XYM808Z4C9E
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe280
project: mirdan-install
title: Unify the three safe-name predicates on mirdan::store::is_safe_name
---
Three competing "is this skill/path name safe" predicates with DIFFERENT rules invite inconsistent acceptance across deploy paths.

## The duplicates
- `mirdan::store::is_safe_name` (`crates/mirdan/src/store.rs:226`) — rejects `/ \\ ..` etc. (the canonical one).
- `mirdan::store::is_safe_relative_path` (`store.rs:246`).
- `swissarmyhammer_skills::deploy::validate_skill_name` (`crates/swissarmyhammer-skills/src/deploy.rs:124`) — alphanumeric-only (stricter, different rule). (Likely deleted in card 1 with write_and_deploy — confirm.)
- `swissarmyhammer-workspace-init::is_safe_skill_name` + its `is_safe_relative_path` copy (`components.rs:399,413`). (Crate deleted in card 5 — confirm gone.)

## Change
- Make `mirdan::store::is_safe_name` / `is_safe_relative_path` the single predicate used by every deploy path.
- Remove any surviving duplicate predicate; route callers to the mirdan one.
- If skills still needs a name check for a non-deploy reason, it should call mirdan's (now legal after card 1) or the check should live where it's actually used — do not reintroduce a divergent rule.

## Done when
- Exactly one safe-name predicate (mirdan's) governs skill/path name validation across deploy paths.
- No `validate_skill_name` / `is_safe_skill_name` duplicates remain (grep clean).
- `cargo build --workspace` green; tests pass.

Depends on the edge inversion (card 1, removes the skills predicate); overlaps card 5 (removes the workspace-init copies).