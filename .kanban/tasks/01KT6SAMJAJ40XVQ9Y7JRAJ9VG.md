---
assignees:
- claude-code
depends_on:
- 01KT6R6HR3KJT6JVNDRAJV8V4T
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe880
project: short-ids
title: 'Short IDs: filter DSL ^ref resolves short ids'
---
Make `^<short>` work in the filter DSL, so a filter like `^8rfp1r` matches the referenced task.

## Background
- Parser already tokenizes `^` → `Expr::Ref(String)` (`crates/swissarmyhammer-filter-expr/src/parser.rs`, Lezer `Ref` token in `apps/kanban-app/ui/src/lang-filter/filter.grammar`). No grammar change needed.
- Eval: `Expr::Ref(id) => ctx.has_ref(id)` (`crates/swissarmyhammer-filter-expr/src/eval.rs`). `has_ref` currently matches "the entity references this card id (via depends_on or id)".

## Scope
- Update the kanban `FilterContext::has_ref` implementation so the ref string resolves a 7-char short id (and full ULID) to a task id before matching against this task's id and its depends_on entries. Case-insensitive.
- Keep `swissarmyhammer-filter-expr` generic — short-id resolution belongs in the kanban-side `FilterContext` impl, not the expr crate.

## Acceptance
- `^<short>` filter selects the task whose short id matches, and tasks that depend_on it (matching today's full-id semantics).
- Full-ULID refs still work; mixed-case short id works.

Depends on core derivation/resolver.