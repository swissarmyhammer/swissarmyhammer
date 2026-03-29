---
assignees:
- claude-code
depends_on:
- 01KMGQCX5CBERQ7AMWSDGQ804B
position_column: done
position_ordinal: ffffffffff8880
title: 'Init component: GitMergeDrivers (install/uninstall all three drivers)'
---
## What
Add a new `Initializable` component `GitMergeDrivers` to the init registry that configures git to use sah merge drivers for all `.kanban/` file types.

**Files to modify:**
- `swissarmyhammer-cli/src/commands/install/components/mod.rs` — add `GitMergeDrivers` struct implementing `Initializable`, register in `register_all()`

**Init behavior (priority ~25, after ProjectStructure):**
1. Check we're in a git repo (`.git/` exists), skip if not
2. Register three merge drivers in `.git/config`:
   - `[merge \"sah-jsonl\"]` → `sah merge jsonl %O %A %B`
   - `[merge \"sah-yaml\"]` → `sah merge yaml %O %A %B`
   - `[merge \"sah-md\"]` → `sah merge md %O %A %B`
3. Create/update `.gitattributes` with three patterns:
   - `.kanban/**/*.jsonl merge=sah-jsonl`
   - `.kanban/**/*.yaml merge=sah-yaml`
   - `.kanban/**/*.md merge=sah-md`
4. Report what was done

**Deinit behavior:**
1. `git config --remove-section merge.sah-jsonl` (ignore if missing)
2. `git config --remove-section merge.sah-yaml` (ignore if missing)
3. `git config --remove-section merge.sah-md` (ignore if missing)
4. Remove the three `.kanban/` lines from `.gitattributes`
5. If `.gitattributes` is now empty, delete it

**Scope:** Only applicable to `Project` and `Local` scopes (not `User`)

**Implementation note:** Factor out the driver registration into a data-driven loop over a list of `(name, pattern, command)` tuples so adding future drivers is trivial.

## Acceptance Criteria
- [ ] `sah init` adds all three merge driver sections to `.git/config`
- [ ] `sah init` adds/updates `.gitattributes` with all three patterns
- [ ] `sah deinit` removes all three drivers and patterns
- [ ] Idempotent — running init twice doesn't duplicate entries
- [ ] Skipped when not in a git repo

## Tests
- [ ] `swissarmyhammer-cli/src/commands/install/components/mod.rs` — test in `#[cfg(test)]` module
- [ ] Test: init in temp git repo creates correct `.git/config` sections
- [ ] Test: init in temp git repo creates correct `.gitattributes` lines
- [ ] Test: deinit removes all
- [ ] Test: double init is idempotent
- [ ] `cargo nextest run -p swissarmyhammer-cli git_merge_driver` #merge-driver