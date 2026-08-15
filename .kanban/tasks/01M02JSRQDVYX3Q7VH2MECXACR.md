---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe080
title: Delete the cognitive-complexity scorer from swissarmyhammer-sem
---
Delete the complexity scorer. Not "retire or split" — delete it. Complexity is not a measured concern any more; the only size gate is function length.

The user's instruction, verbatim: *"retire it — we needed to get rid of complexity scoring — like rid, we are just doing function length, I thought I was clear."*

## What survived and why it must not

`^z2r1psf` removed the five `complexity-<lang>` tool rules, the `cognitive-complexity` prompt rule and the probe WIRING, but kept the scorer in `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` and its `complexity/` tree. The reason given was that `test_census.rs` imports nine items from it and has its own live consumer in `tree_sitter_probes.rs`.

That is a dependency to break, not a reason to keep dead code. The scorer now has no consumer of its own — it is kept alive solely by a neighbour that reached into it.

## What to do

1. Establish exactly what `test_census.rs` uses from `complexity.rs` — the nine imported items, and whether each is genuinely about complexity or is a general tree-sitter helper that merely lives there.
2. Move what `test_census` legitimately needs to where it belongs. A language-spec lookup or a node-text helper is not complexity scoring and should not be deleted with it; the scoring itself is.
3. Delete the scorer and its `complexity/` tree.
4. Sweep for anything else reaching into it, inside and outside the crate.

## Watch for

The measurement recorded on `^4dyewvd` applies here: the four `node_text` copies in this tree have four different contracts, and `spec_for_language` reads four unrelated static tables. If `test_census` needs one of those, do not unify it with its siblings while moving it — that question was settled against a shared module, and the reasoning is written into `plugins/code/mod.rs`.

## Done when

- No complexity scoring code remains in `swissarmyhammer-sem`.
- `test_census` compiles and its live consumer still works.
- `cargo nextest run --workspace` green; fmt and clippy clean.

#tool-validators