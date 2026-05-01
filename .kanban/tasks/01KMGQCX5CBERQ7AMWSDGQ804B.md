---
assignees:
- claude-code
depends_on:
- 01KMGQCDYS8CFHVK5BR517MER4
position_column: done
position_ordinal: ffffffffffdb80
title: 'CLI subcommand: `sah merge {jsonl,yaml,md} %O %A %B`'
---
## What
Add a `merge` subcommand to the CLI that dispatches to file-type-specific merge drivers. Three subcommands: `jsonl`, `yaml`, `md`.

Git calls merge drivers as: `sah merge jsonl %O %A %B` where %O=base, %A=ours (write result here), %B=theirs.

**Files to create/modify:**
- `swissarmyhammer-cli/src/cli.rs` — add `Merge { subcommand: MergeSubcommand }` variant to `Commands` enum, add `MergeSubcommand` enum with `Jsonl`, `Yaml`, `Md` variants, each with `{ base, ours, theirs }` positional args. `Yaml` and `Md` also accept optional `--jsonl-path` for changelog-aware conflict resolution.
- `swissarmyhammer-cli/src/commands/merge/mod.rs` — new module, dispatch to subcommand handlers
- `swissarmyhammer-cli/src/commands/merge/jsonl.rs` — reads 3 files, calls `merge_jsonl()`, writes result to `ours` path
- `swissarmyhammer-cli/src/commands/merge/yaml.rs` — reads 3 files, derives sibling `.jsonl` path from `%A`, calls `merge_yaml()`, writes result
- `swissarmyhammer-cli/src/commands/merge/md.rs` — reads 3 files, derives sibling `.jsonl` path from `%A`, calls `merge_md()`, writes result
- `swissarmyhammer-cli/src/commands/mod.rs` — add `pub mod merge;`
- `swissarmyhammer-cli/src/main.rs` — add `Some((\"merge\", sub_matches)) => handle_merge_command(sub_matches)` to `route_subcommand`
- `swissarmyhammer-cli/Cargo.toml` — add `swissarmyhammer-merge` dependency

**Behavior:**
- Exit 0: merge succeeded, result written to %A
- Exit 1: conflict detected (git will show conflict markers or abort)
- Exit >=2: error (git treats as fatal)
- Stderr: diagnostic messages (git shows these to user)

**JSONL path derivation for yaml/md:** The `%A` path is a temp file, but git also sets `$MERGED` env var with the real path. Use that to derive the sibling `.jsonl`. If not available, skip changelog resolution.

## Acceptance Criteria
- [ ] `sah merge jsonl base ours theirs` produces correct merged output
- [ ] `sah merge yaml base ours theirs` produces correct merged output  
- [ ] `sah merge md base ours theirs` produces correct merged output
- [ ] Exit code 0 on success, 1 on conflict for all three
- [ ] `sah merge --help` shows available file types
- [ ] YAML/MD drivers derive JSONL sibling path for newest-wins resolution

## Tests
- [ ] `swissarmyhammer-cli/src/commands/merge/` — integration tests with temp files per type
- [ ] Test: each subcommand reads files, merges, writes output
- [ ] Test: conflict case returns exit code 1
- [ ] `cargo nextest run -p swissarmyhammer-cli merge` #merge-driver