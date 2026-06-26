---
assignees:
- claude-code
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
position_column: todo
position_ordinal: a680
project: expect
title: expect MCP tool skeleton + op dispatch + noun-first CLI wiring
---
## What
Create the op-dispatched MCP tool that surfaces all `expect` ops, modeled on `diagnostics`/`review`. Ops are stubs for now (return "not implemented"); later tasks fill them in. This unblocks every op-bearing task.

- New dir `crates/swissarmyhammer-tools/src/mcp/tools/expect/`:
  - `mod.rs` — `ExpectTool`; one zero-sized `Operation` struct per `<verb> <noun>` op id from the domain grid in `ideas/expect.md` §"Operations" (e.g. `ExpectationCreate`/`"create expectation"`, `ExpectationGet`, `ExpectationDelete`, `ExpectationObserve`, `ExpectationCheck`, `ExpectationsList`, `ExpectationsObserve`, `ExpectationsCheck`, `ObservationGet`, `ObservationDelete`, `ObservationEvaluate`, `ObservationApprove`, `ObservationsList`, `ObservationsEvaluate`, `ObservationsApprove`, `GoldenGet`, `GoldenDelete`, `GoldenEvaluate`, `GoldensList`, `GoldensEvaluate`, `SurfaceGet`, `SurfacesList`). Use the `#[operation(verb=…, noun=…, description=…)]` macro from `swissarmyhammer-operations-macros` (see `crates/swissarmyhammer-kanban/src/board/init.rs` for usage). Note: op id is `"verb noun"`; CLI renders noun-first via cli_gen.
  - `EXPECT_OPERATIONS: Lazy<Vec<&'static dyn Operation>>`.
  - `impl McpTool for ExpectTool`: `name()="expect"`, `description()=include_str!("description.md")`, `schema()=generate_mcp_schema_wire`, `schema_full()=generate_mcp_schema_full`, `cli_category()=Some("expect")`, `operations()=&EXPECT_OPERATIONS`, and `execute()` with a `match op_str { … }` dispatch (string match on `op`, unknown ⇒ `invalid_params`). Stub each op via `op_tool_helpers::json_result` of a placeholder.
  - `impl_default_doctorable!(ExpectTool)` + `impl_empty_initializable!(ExpectTool)` for now (real impls land in the doctor/init tasks, which will replace these).
  - `description.md`.
- Register `pub mod expect;` in `crates/swissarmyhammer-tools/src/mcp/tools/mod.rs`; add `register_expect_tools(...)` and wire into `create_fully_registered_tool_registry()` (`tool_registry.rs:~2189`), `server.rs` serve path, `tool_config.rs`, `lib.rs`/`mcp/mod.rs` re-exports, and `health_registry.rs`.
- The `sah` CLI surfaces it automatically via `dynamic_cli.rs` `build_operation_tool_subcommand` (reads `operations()`/`schema_full()`); verify `sah expect …` and the noun-first tree appear.

## Acceptance Criteria
- [ ] `expect` appears in the MCP tool registry; `tools/list` shows it with the slim wire schema (just the `op` enum).
- [ ] `sah expect expectation check --help` (and other noun/verb pairs) render via cli_gen — noun-first, one command per noun.
- [ ] Every op in the domain grid dispatches (stub returns a structured "not implemented yet" JSON, no panic on unknown op → `invalid_params`).
- [ ] `cargo build -p swissarmyhammer-tools` succeeds.

## Tests
- [ ] Unit test in `expect/mod.rs`: round-trip the op list through `cli_gen::test_support` (collect verb/noun pairs) asserting each grid cell is present and noun-first parse works.
- [ ] Registry test: `create_fully_registered_tool_registry()` contains a tool named `expect` (mirror existing registry tests).
- [ ] `cargo nextest run -p swissarmyhammer-tools expect` passes.

## Workflow
- Use `/tdd`. Copy `tools/diagnostics/mod.rs` as the skeleton.