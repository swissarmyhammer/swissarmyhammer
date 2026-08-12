---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztmfshs8x2k5z1275ke8ck4
  text: |-
    Picked up. Research done.

    The shipped script stands at `builtin/validators/code-hygiene/rules/missing-docs-rust.md`. It writes `set -e`, sends the cargo report to a file, and pipes `jq` into `sort -u`. Two defects follow from that shape:

    - `set -e` makes cargo's status the script's status, so a workspace clippy linted from end to end BREAKS the run when a lint stands at deny level.
    - the filter stands in a pipe that ends in `sort -u`, so a `jq` that cannot run answers exit 0 with no finding. The two siblings each closed that hole with a `filtered` status test.

    The two siblings that carry the answer:
    - `builtin/validators/code-hygiene/rules/dead-code-rust.md`, section "A workspace the tool cannot check".
    - `builtin/validators/code-hygiene/rules/complexity-rust.md`, section "A workspace the tool cannot lint".

    Each writes five `jq` calls to files, tests each status, then breaks the run in four places: no `build-finished` entry beside a nonzero status; an error-level message with a rustc code or with no code; a `custom-build` artifact whose package writes no `build-script-executed` entry; a nonzero status beside no error-level message at all.

    Test shapes stand in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`: `ShippedStagedTree`, `verify_shipped_tree_breaks`, `verify_shipped_tree_reports`, `verify_shipped_tree_breaks_without` and `FILTER_BINARY_NAME`.

    One thing to note for the next agent: `verify_shipped_tree_breaks_without` leads `PATH` with a stub, and `PATH` is process state. `dead_code_rust.rs` therefore stands EVERY test of its module under `#[serial_test::serial(env)]`. The Rust missing-docs tests stand today in `missing_docs.rs` beside Swift, Go, Python, Dart and TypeScript tests that are not serial, so the filter test cannot stand there. The Rust tests move to a module of their own, `shipped/missing_docs_rust.rs`, which is the shape `complexity_rust.rs` and `dead_code_rust.rs` already carry and which the module comment of `shipped.rs` states.
  timestamp: 2026-08-12T09:22:37.753583+00:00
- actor: claude-code
  id: 01kztn5k0ygn63xztr84x1ev2w
  text: |-
    Measured every shape by hand before the change, with clippy 0.1.97, cargo 1.97.1 and jq 1.8.2, over nine probe packages. The card's claim reproduces: `[lints.rust] unused_variables = "deny"` beside one undocumented `pub struct` gave 0 findings and exit 101 under the shipped script, and gives 1 finding at exit 0 under the new one.

    Two measurements the card did not name, and each is now a row of the rule body:

    - `RUSTFLAGS="-D warnings"` makes the rule's OWN diagnostic arrive at level `error` with the code `missing_docs`. The raw report holds that code, the filter selects on the code, and the finding stands. The earlier script broke the run exactly where the rule had something to say.
    - The filter stood in a pipe that ended in `sort -u`. With `jq` replaced by a command that exits 127, the earlier script wrote 0 findings and exited 0, so the whole rule went missing without a word. The new script gives each `jq` call its own file and its own status test, which is the shape both siblings carry.

    TDD: the seven new tests were written first and each failed for the stated reason (the script's own line was missing from the error detail). One failure was mine rather than the script's — `RUST_UNCOMPILABLE_ERROR` named the package `uncompilable-probe`, and the two-member probe names its failing member `bad`, so the fragments now name the line and cargo's words alone.

    The Rust tests moved from `shipped/missing_docs.rs` to a new `shipped/missing_docs_rust.rs`, which is the shape `complexity_rust.rs` and `dead_code_rust.rs` carry and which the module comment of `shipped.rs` states. `#[serial_test::serial(env)]` stands on the filter test alone, because that test alone writes process state, which is the ruling ^y4xyw1g recorded.
  timestamp: 2026-08-12T09:34:31.966215+00:00
- actor: claude-code
  id: 01kztn5sgfkqke8bhcjcn5zwh8
  text: |-
    ### implement — changed
    - evidence: 4 files — builtin/validators/code-hygiene/rules/missing-docs-rust.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs_rust.rs (new). `cargo nextest run -p swissarmyhammer-validators`: 688 run, 688 passed, 0 failed. `cargo fmt --check` and `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings`: clean.
    - next: /review
  timestamp: 2026-08-12T09:34:38.607204+00:00
position_column: doing
position_ordinal: '8380'
title: missing-docs-rust breaks the run for a workspace clippy linted, when a lint stands at deny level
---
`missing-docs-rust` tests the STATUS of `cargo clippy` alone: it writes `set -e` and lets cargo's exit status be the script's. `cargo clippy` gives one status to a run it could not make and to a run it made from end to end while a lint stands at deny level, so the second shape breaks the run and every `missing_docs` finding it holds is thrown away.

Measured 2026-08-12 with clippy 0.1.97, over one probe package that declares `[lints.rust] unused_variables = "deny"`, holds one unused variable, and holds an undocumented `pub struct Undocumented`:

```
$ bash <the shipped missing-docs-rust script>
error: could not compile `md-probe` (lib) due to 1 previous error; 2 warnings emitted
exit=101
```

0 findings, exit 1 to the engine — for a workspace clippy linted, whose undocumented public item the rule exists to report. `RUSTFLAGS="-D warnings"` and a crate-level `#![deny(...)]` give the same shape, and `RUSTFLAGS="-D warnings"` is what a CI machine sets.

`builtin/validators/README.md` states the answer: "One status can carry both a measured run and a broken run... The script must then test the REPORT beside the status, and accept the shared status only for the report shape a measured run writes."

Two sibling rules already make that test over the same cargo report, and each states its measurement in a table: `complexity-rust` ("A workspace the tool cannot lint") and `dead-code-rust` ("A workspace the tool cannot check"). Give `missing-docs-rust` the same four tests — `build-finished` present, no rustc error code, every compiled build script ran, and no nonzero status with no compiler error at all — and hold each with an acceptance test beside `the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile`. `ShippedStagedTree`, `verify_shipped_tree_breaks` and `verify_shipped_tree_reports` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` carry the shapes.

#tool-validators #objectivity</description>
<parameter name="tags">["tool-validators", "objectivity"]