---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdd80
title: Upgrade chumsky 0.11 → 0.13 in swissarmyhammer-filter-expr
---
## What

Upgrade the `chumsky` parser-combinator dependency from `0.11` to the latest stable `0.13.0`. There is exactly **one** dependency edge and **one** code site:

- **Manifest:** `crates/swissarmyhammer-filter-expr/Cargo.toml` line 13 — change `chumsky = "0.11"` to `chumsky = "0.13"`.
- **Lockfile:** `Cargo.lock` — `chumsky` entry is currently `version = "0.11.2"`; `cargo update -p chumsky` (or `cargo build`) will resolve it to `0.13.0`. Verify the new entry and its transitive deps (`hashbrown`, `regex-automata`, `serde`, `stacker`, `unicode-ident`, `unicode-segmentation`) resolve cleanly.
- **Only code consumer:** `crates/swissarmyhammer-filter-expr/src/parser.rs` (`use chumsky::prelude::*`). `eval.rs` and `lib.rs` do NOT touch chumsky.

### Migration risk: LOW
chumsky 0.10 was a from-scratch rewrite; `parser.rs` already uses the post-rewrite API (`Parser<'src, &'src str, Expr, extra::Err<Rich<'src, char>>>`, `recursive`, `foldl`/`foldr`, `choice`, `just`, `any().filter(...)`, `.to_slice()`, `.padded()`, `delimited_by`, `repeated().at_least(1)`, `or_not`, `then_ignore`, `ignore_then`, `.rewind()`, `ParseResult::has_errors`/`into_errors`/`into_output`). Per the upstream CHANGELOG, **0.12.0 is purely additive** (no `Removed`, no breaking `Changed` for these combinators) and **0.13.0** only adds `select!`/`select_ref!` cfg-attribute support. None of the APIs used here changed. Expect a clean bump with no source edits — but if the compiler surfaces any signature change (e.g. `Rich`/`extra` bounds), adapt `parser.rs` minimally to restore compilation while keeping the public API (`parse`, `Expr`, `ParseError`, `FilterContext`) byte-for-byte identical.

### Blast radius (contained)
Downstream consumers (`swissarmyhammer-kanban` task/perspective helpers, `apps/kanban-app`) only call the crate's public `parse()` / `Expr::matches()` — they never reference chumsky types. So no downstream code changes are needed as long as the public API is unchanged.

Pin to stable `0.13`. Do NOT adopt `1.0.0-alpha.8` (pre-release).

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-filter-expr/Cargo.toml` declares `chumsky = "0.13"`.
- [ ] `Cargo.lock` resolves `chumsky` to `0.13.0`; no other crate is unexpectedly downgraded/duplicated.
- [ ] `crates/swissarmyhammer-filter-expr/src/parser.rs` compiles against 0.13; public API of the crate (`parse`, `Expr`, `ParseError`, `FilterContext`) is unchanged.
- [ ] No new compiler warnings introduced in `swissarmyhammer-filter-expr`.
- [ ] Downstream `swissarmyhammer-kanban` and `apps/kanban-app` build without source changes.

## Tests
- [ ] Existing tests are the regression guard — the suites in `crates/swissarmyhammer-filter-expr/src/parser.rs` (`mod tests`) and `crates/swissarmyhammer-filter-expr/src/lib.rs` (`mod tests`) cover atoms, AND/OR/NOT, implicit-AND, precedence, grouping, keyword operators, error cases (`$$`, incomplete, unmatched paren), span info, and Display. They must all pass unchanged after the bump.
- [ ] Run `cargo test -p swissarmyhammer-filter-expr` — expect 0 failures.
- [ ] Run `cargo build -p swissarmyhammer-kanban -p kanban-app` — expect success (verifies downstream consumers still link).
- [ ] Run `cargo clippy -p swissarmyhammer-filter-expr -- -D warnings` — expect clean.
- [ ] Confirm error-message format tests (`display_impl_from_real_parse_error`, `error_has_span_info`) still pass — these assert only the positional `start..end` shape we control, not chumsky's wording, so they are robust to any error-text changes across versions.

## Workflow
- Use `/tdd` — the existing test suite IS the failing-first guard: bump the version, run `cargo test -p swissarmyhammer-filter-expr`, and only touch `parser.rs` if compilation fails. Keep edits minimal and behavior-preserving.