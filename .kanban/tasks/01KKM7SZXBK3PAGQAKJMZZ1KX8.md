---
assignees:
- assistant
depends_on:
- 01KKM7RX9ZTK4VKMB6P9NPWHWW
position_column: done
position_ordinal: ffffeb80
title: Update swissarmyhammer-common directory alias and all consumers
---
## What
Update the `SwissarmyhammerDirectory` type alias in `swissarmyhammer-common/src/directory.rs` and all ~60+ references across the codebase that use it via `SwissarmyhammerDirectory::dir_name()` or `SwissarmyhammerDirectory::from_git_root()`.

### Key changes
- `swissarmyhammer-common/src/directory.rs`: Update DIR_NAME const, update doc comments from `.swissarmyhammer` to `.sah`
- `swissarmyhammer-config/src/discovery.rs`: Uses `SwissarmyhammerDirectory::dir_name()` — will automatically pick up new name. Update comments and test assertions that hardcode `.swissarmyhammer`
- `swissarmyhammer-config/src/model.rs`: ~10 test references
- `swissarmyhammer-cli/src/main.rs`: Uses `SwissarmyhammerDirectory::from_git_root()`
- `swissarmyhammer-cli/src/commands/doctor/mod.rs`: doctor command checks
- `swissarmyhammer-cli/src/commands/install/components/mod.rs`: install path
- `swissarmyhammer-tools/src/mcp/unified_server.rs`: log dir name
- `swissarmyhammer-tools/src/mcp/tools/questions/persistence.rs`: questions dir
- `claude-agent/src/session.rs`: session dir
- `swissarmyhammer-common/src/test_utils.rs`: test helpers
- `swissarmyhammer-common/src/utils/paths.rs`: deprecated path util
- All integration tests in `swissarmyhammer-config/tests/integration/` (~8 files)
- All integration tests in `swissarmyhammer-cli/tests/integration/` (~5 files)

### Key insight
Most of these use `SwissarmyhammerDirectory::dir_name()` which is derived from the config — so the rename in card 1 propagates automatically. The work here is updating:
1. Hardcoded `.swissarmyhammer` string literals in comments and test assertions
2. Test assertions like `ends_with(\"workspace/.swissarmyhammer\")`

## Acceptance Criteria
- [ ] No hardcoded `.swissarmyhammer` string literals remain anywhere in `.rs` files (grep clean)
- [ ] All integration tests pass with `.sah` directory name
- [ ] Doc comments updated

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-common`
- [ ] `cargo nextest run -p swissarmyhammer-config`
- [ ] `cargo nextest run -p swissarmyhammer-cli`
- [ ] `cargo nextest run -p swissarmyhammer-tools`