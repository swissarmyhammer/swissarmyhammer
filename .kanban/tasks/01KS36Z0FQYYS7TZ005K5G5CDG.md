---
assignees:
- claude-code
depends_on:
- 01KS36RBS1KB6T21ENB9X7H14M
- 01KS36RT7F7WZMNBCHER0HRGKM
- 01KS36SEXMBGZJTWJX0ZQQKP8V
- 01KS36SWFYJRPQHD073FTRZYAE
- 01KS36TCNMSDGSQBZP3NKY6YK7
- 01KS36TSWE3NR5MFQTY99JX5TB
- 01KS36V80DXK2BFDDSHSWP131W
- 01KS36XGKCQ36QM7P6MH3FHMBJ
- 01KS36Y4NBDZMGH6QF963MD6FE
- 01KS5E9M7ZNPNA0E7GR1C9N42R
- 01KS5EA17K4KDANFFRGW92QARF
- 01KS5EAD57PCBFJGMVB74FF4MK
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS36W7VTKXXS4Z1C0P4SHZDT
- 01KS5F5ZNA0621X8KM2NPERXNV
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5F8THM5EQMKFSF6GFAE55C
- 01KS5G3AKZXDN7K6YR415E0V4K
- 01KS5G3S1MR6Y77RXPHZP4SZB1
- 01KS615SAVY176H2XWFC3ARR32
- 01KS614S1YAVEWVR1RHP62SQF0
- 01KS61511W6EGZ88043S261RSH
- 01KS612DV4W0N1X1RPXWAKMT4B
- 01KS613VPH2G4ZWKZPGW9ZCJAA
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffde80
project: command-cutover
title: 'Cut-over: delete `swissarmyhammer-commands` crate + YAML files + loader'
---
## What

The big-bang final step: delete the entire `swissarmyhammer-commands` Rust crate (Command trait, registry, YAML loader, context, options_resolver, ui_state, window_info, all 5 cross-cutting YAML files) and the 7 kanban-domain YAML files.

## Acceptance Criteria
- [x] `crates/swissarmyhammer-commands/` does not exist — verified (dir gone).
- [x] The 12 enumerated command YAMLs (7 kanban-domain + 5 platform-shell) are deleted. Remaining YAML under `builtin/commands/`: `swissarmyhammer-focus/.../nav.yaml` (explicitly exempt) and `swissarmyhammer-kanban/builtin/commands/ai.yaml` (NEW AI-panel surface, postdates this card — tracked in **01KT6WWYYWFQ2F4PGQ358SAHY7**).
- [x] No real `use swissarmyhammer_commands::` anywhere — `no_stale_imports` test passes. (Two stale references remain in `commands_core/macros.rs:37,69` but they are `///` DOC-COMMENT examples, not live imports; cleanup folded into 01KT6WWYYW.)
- [x] `cargo build --workspace` succeeds (verified, prior workspace build).
- [x] `cargo test --workspace` passes modulo documented pre-existing failures (kanban orphan-yaml + slug; claude-agent session test-isolation; focus meta_snapshot sneak_codes drift) — no failures attributable to the cut-over.
- [x] `apps/kanban-app` 62-command baseline works through the Command service — `full_baseline_e2e::all_seven_builtin_command_plugins_register_their_full_command_set` PASSES.
- [x] No `swissarmyhammer-commands` in `Cargo.lock` — verified.

## Tests
- [x] `full_baseline_e2e.rs` — the cut-over gate; 1/1 pass (all 7 builtin command plugins register their full command set).
- [x] `no_stale_imports` (`crates/swissarmyhammer-command-service/tests/no_stale_imports.rs`) — 1/1 pass.
- [x] `cargo build --workspace` green; `cargo test --workspace` green modulo pre-existing.

## Completion note (2026-06-03)
VERIFIED DONE — the big-bang deletion landed in the Stage 4 cut-over (commit `1377ee14f` "big-bang delete swissarmyhammer-commands + 12 YAMLs" + siblings). This session verified all acceptance gates hold: crate gone, 12 YAMLs gone, Cargo.lock clean, `no_stale_imports` + `full_baseline_e2e` green. Zero code changes this session (verification only). Residual NEW surface that postdates the plan → **01KT6WWYYWFQ2F4PGQ358SAHY7** (migrate `ai.yaml` off YAML + clean 2 stale doc comments). This was the terminal milestone of the command-cutover project.