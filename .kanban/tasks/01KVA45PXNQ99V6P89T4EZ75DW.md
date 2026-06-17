---
comments:
- actor: claude-code
  id: 01kva4mc7y7xdx3mxvw76z6vy7
  text: |-
    Picked up. Operations-crate core change landed (TDD RED->GREEN verified):
    - Relocated/renamed wire_schema_signatures_* -> full_schema_signatures_cover_every_op_with_ordered_required_names, asserting generate_mcp_schema_full(...)["x-op-signatures"]. Watched it fail (None unwrap) before the generator change, passes after.
    - Moved signature-map construction from generate_mcp_schema_wire into generate_mcp_schema_full (inserts "x-op-signatures"). required_param_names_for_op stays shared.
    - WIRE_DROPPED_KEYS now [&str; 5] including "x-op-signatures".
    - wire_schema_omits_dropped_keys now iterates WIRE_DROPPED_KEYS (auto-covers x-op-signatures).
    - Doc comments updated on generate_mcp_schema (alias), _full, _wire, WIRE_DROPPED_KEYS, required_param_names_for_op.
    cargo test -p swissarmyhammer-operations schema:: => 19 passed.

    Live grep of source files asserting x-op-signatures (non-.kanban):
    swissarmyhammer-kanban/src/schema.rs (4 sites), swissarmyhammer-agents/src/schema.rs, swissarmyhammer-skills/src/schema.rs, tools: questions/mod.rs, web/schema.rs, web/mod.rs, shell/mod.rs, ralph/execute/mod.rs, git/changes/mod.rs, files/schema.rs, files/mod.rs, code_context/schema.rs, code_context/mod.rs.
    Note: apps/swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs does NOT match the grep (no wire x-op-signatures assertion). Sweeping per-tool sites next.
  timestamp: 2026-06-17T06:34:52.542685+00:00
- actor: claude-code
  id: 01kva6axbe6w17v9wtwpfcf4db
  text: |-
    Sweep complete + all verification green.

    Files touched (grep-confirmed, all source x-op-signatures now full-only):
    - crates/swissarmyhammer-operations/src/schema.rs (core: moved insertion to _full, WIRE_DROPPED_KEYS->[5], docs, test relocate/rename)
    - crates/swissarmyhammer-kanban/src/schema.rs (4 sites + added full assert in test_full_schema_structure_keeps_heavy_keys; renamed test_detail_param_absent_from_wire_signatures_but_in_full_schema -> ..._from_signatures_...)
    - crates/swissarmyhammer-agents/src/schema.rs
    - crates/swissarmyhammer-skills/src/schema.rs
    - crates/swissarmyhammer-tools/src/mcp/tools/web/schema.rs, web/mod.rs
    - crates/swissarmyhammer-tools/src/mcp/tools/files/schema.rs, files/mod.rs
    - crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs, code_context/mod.rs
    - crates/swissarmyhammer-tools/src/mcp/tools/questions/mod.rs (switched literal list -> WIRE_DROPPED_KEYS)
    - crates/swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs (literal list -> const)
    - crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs (literal list -> const)
    - crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs (literal list -> const)
    - standards/mcp.md (Schema Generation now documents WIRE op-enum-only vs FULL; x-op-signatures listed under full + WIRE_DROPPED_KEYS)

    apps/swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs: no x-op-signatures reference, no change needed (matches grep, contradicts no part of contract).

    Test results:
    - cargo test -p swissarmyhammer-operations schema:: => 19 passed, 0 failed
    - cargo test -p swissarmyhammer-skills => 115+2+2+2+0 passed, 0 failed
    - cargo test -p swissarmyhammer-agents => 110 passed, 0 failed
    - cargo test -p swissarmyhammer-kanban => 23 'test result: ok' lines, 0 failed/0 warnings (incl. lib schema:: 15 passed)
    - cargo test -p swissarmyhammer-tools => 11 'test result' lines, 0 failed, 0 warnings/errors
    - cargo build (full workspace) => Finished, 0 warnings, 0 errors

    RED->GREEN verified on the relocated full-schema signature test (panicked on None unwrap before generator change, passes after).
  timestamp: 2026-06-17T07:04:39.534603+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb780
project: cli-schema-gen
title: Move x-op-signatures from the WIRE schema to the FULL schema only
---
## Context

The operation-tool schema generator (`crates/swissarmyhammer-operations/src/schema.rs`) emits two surfaces:

- **WIRE** (`generate_mcp_schema_wire`, ~lines 203–230) — model-facing, sent over MCP `tools/list`. Today it carries the `op` enum + `required: ["op"]` **and** an `x-op-signatures` map (per-op required-param-name lists, built via `required_param_names_for_op`).
- **FULL** (`generate_mcp_schema_full`, ~lines 110–158, aliased `generate_mcp_schema`) — in-process only, consumed by the CLI generator (`cli_gen::build_commands_from_schema`, which reads `x-operation-schemas`). Carries the heavy keys (`x-operation-schemas`, `x-operation-groups`, `x-forgiving-input`, `examples`) but currently does **not** carry `x-op-signatures`.

`WIRE_DROPPED_KEYS` (~line 168, `[&str; 4]`) is the single source of truth for "keys FULL carries but WIRE omits": `x-operation-schemas`, `x-operation-groups`, `x-forgiving-input`, `examples`.

**Requested change:** `x-op-signatures` should live on the FULL schema only, not the WIRE schema. After this change the WIRE surface is just `{ op enum, required:["op"] }` (the most minimal model-facing schema), and the FULL schema gains `x-op-signatures` alongside the other heavy keys. Intended effect: the per-op required-field map stops shipping over the wire on every prompt; it remains available on the in-process full/documentation surface.

**Interaction with `^0h7pdd4`** (review wire/full migration + workspace guard test): that task's guard asserts "wire CONTAINS x-op-signatures", which this change inverts. This task defines the contract and must land first; `^0h7pdd4` has been set to `depends_on` this task and will write its guard against the post-change contract.

No `ARCHITECTURE.md` exists at repo root, so no architecture doc update is implied.

## What

- [x] In `crates/swissarmyhammer-operations/src/schema.rs`: remove the `x-op-signatures` insertion from `generate_mcp_schema_wire` so the wire schema's only property is `op` (keep `required: ["op"]`, `additionalProperties: true`, description). Move the signature-map construction (the `required_param_names_for_op` loop) into `generate_mcp_schema_full`, inserting `schema["x-op-signatures"] = signatures`. Keep `required_param_names_for_op` shared (still used by `operation_to_schema`).
- [x] Add `"x-op-signatures"` to `WIRE_DROPPED_KEYS` (becomes `[&str; 5]`). This keeps it the single source of truth — every workspace test that iterates the const to assert "wire omits / full contains" then covers `x-op-signatures` automatically.
- [x] Update the doc comments: the `generate_mcp_schema_wire` doc block (~lines 179–198, which currently documents `x-op-signatures` as part of the wire shape) and the `generate_mcp_schema_full` / `WIRE_DROPPED_KEYS` docs to reflect that `x-op-signatures` is now a full-only key.
- [x] Update the operations-crate unit tests in the same file: relocate `wire_schema_signatures_cover_every_op_with_ordered_required_names` (~line 676) to assert against `generate_mcp_schema_full(...)["x-op-signatures"]`; ensure `wire_schema_omits_dropped_keys` (~line 637) checks `x-op-signatures` is absent from wire (prefer iterating `WIRE_DROPPED_KEYS` over a literal list); `wire_schema_keeps_op_enum_and_top_level_shape` (~line 654, asserts wire `properties.len() == 1`) stays valid.
- [x] Flip the per-tool schema tests across the workspace that hardcode `wire["x-op-signatures"].is_object()` / `schema()["x-op-signatures"]` so they assert it on the FULL schema (`schema_full()`) and assert its ABSENCE on the wire. Known sites: `crates/swissarmyhammer-skills/src/schema.rs`, `crates/swissarmyhammer-agents/src/schema.rs`, `crates/swissarmyhammer-kanban/src/schema.rs` (multiple), and under `crates/swissarmyhammer-tools/src/mcp/tools/`: `questions/mod.rs`, `web/schema.rs`+`web/mod.rs`, `code_context/schema.rs`+`code_context/mod.rs`, `git/changes/mod.rs`, `files/schema.rs`+`files/mod.rs`, `shell/mod.rs`, `ralph/execute/mod.rs`. (Re-grep `x-op-signatures` to get the live list before editing.) Also update `apps/swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs` if it asserts wire contains the signatures. [LIVE GREP: cli test had NO x-op-signatures reference, so no change needed there.]
- [x] Update `standards/mcp.md` Schema Generation section so the documented wire surface is the `op` enum only and `x-op-signatures` is listed under the full schema.

This is a single atomic contract change: although it touches many files, the bulk are mechanical assertion flips that must land together (a partial change leaves the workspace red). Keep it as one task rather than splitting mid-contract.

## Acceptance Criteria
- [x] `generate_mcp_schema_wire(...)` output has NO `x-op-signatures` key; its `properties` contains only `op`.
- [x] `generate_mcp_schema_full(...)` output HAS `x-op-signatures`, with exactly one entry per op in the enum, each value the op's required param names (excluding `op`) in declaration order — same shape the wire map had before.
- [x] `WIRE_DROPPED_KEYS` contains `x-op-signatures` and has length 5; `generate_mcp_schema_wire` emits none of the `WIRE_DROPPED_KEYS` and `generate_mcp_schema_full` emits all of them.
- [x] `cargo build` and the full workspace test suite are green (no test still asserts `x-op-signatures` on the wire).
- [x] `standards/mcp.md` describes the wire schema as op-enum-only and lists `x-op-signatures` as a full-schema key.

## Tests
- [x] In `crates/swissarmyhammer-operations/src/schema.rs`: a test asserting `generate_mcp_schema_wire` omits `x-op-signatures` (and, via `WIRE_DROPPED_KEYS`, all dropped keys); a test asserting `generate_mcp_schema_full` carries `x-op-signatures` with the correct per-op ordered required names (move/rename the existing `wire_schema_signatures_*` test). Update `wire_schema_is_dramatically_smaller_than_full` expectations if the byte ceiling assertion is affected (wire is now smaller). [byte ceiling unaffected — still passes.]
- [x] Per-tool tests flipped to assert `x-op-signatures` on `schema_full()` and its absence on `schema()` for at least kanban, web, and shell (representative of the three crates: kanban, agents/skills, tools).
- [x] Run: `cargo test -p swissarmyhammer-operations schema:: && cargo test -p swissarmyhammer-tools && cargo test -p swissarmyhammer-kanban` — expected: green. Verify RED→GREEN by confirming the relocated full-schema signature test fails before the generator change and passes after. [verified RED via None-unwrap panic, GREEN after.]

## Workflow
- Use `/tdd` — first write the failing operations-crate tests for the new contract (wire omits / full contains `x-op-signatures`), watch them fail, then relocate the insertion and extend `WIRE_DROPPED_KEYS`, then sweep the per-tool assertion sites and docs.