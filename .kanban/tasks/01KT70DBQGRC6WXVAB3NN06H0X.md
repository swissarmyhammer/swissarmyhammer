---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe880
project: plugin-examples
title: Fix failing example_layering_e2e::committed_examples_coload_across_layers (probe server-name mismatch)
---
Real failing test on the branch (confirmed failing on committed HEAD, independent of the default-class entry card 01KT43M0).

`crates/swissarmyhammer-plugin/tests/example_layering_e2e.rs::committed_examples_coload_across_layers` fails with `UnknownServer`: the `kanban-builtin-probe` bundle registers its in-process module under the canonical name `register("kanban", { rust: "kanban" })`, but the test routes/asserts the server name `kanban-builtin-probe`.

Root cause: the builtin-command-wiring card (01KT4E65) deliberately changed the probe to register under the shared canonical `"kanban"` name (so it shares the single-activation in-process module with the command plugins instead of starving them). The `example_layering_e2e` test wasn't updated to match, so it's been red since that change landed (commit 87c353dd5).

## Fix
- Update `example_layering_e2e.rs` to route/expect the `"kanban"` server name (matching the probe's canonical registration), OR — if the example is meant to demonstrate a distinctly-named server — give the layering example its own probe bundle with a distinct module that doesn't collide with the shared `kanban` activation. Prefer updating the test to the canonical name unless the example's pedagogical point requires a distinct server.
- Confirm `cargo test -p swissarmyhammer-plugin` is fully green afterward (this is currently the only non-pre-existing plugin-crate failure).

Note: relates to the single-activation caveat from 01KT4E65 — in-process `{rust}` modules are shared only when registered under the same public name.