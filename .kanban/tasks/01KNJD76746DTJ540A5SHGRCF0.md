---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
position_swimlane: null
title: 'FILTER-0: Rust crate — chumsky parser + AST evaluator'
---
## What

Create a new crate `swissarmyhammer-filter-expr` with two layers:

1. **Parser** (chumsky): Character-level combinators that tokenize and parse in a single pass, producing an `Expr` AST with standard boolean precedence (NOT > AND > OR), implicit AND between adjacent atoms
2. **Evaluator**: `Expr::matches(&self, ctx: &FilterContext) -> bool` where `FilterContext` provides tag lookup, assignee lookup, and ref lookup

No separate lexer crate needed — chumsky handles both tokenizing and parsing for a grammar this small.

### AST

```rust
enum Expr<'src> {
    Tag(&'src str),
    Assignee(&'src str),
    Ref(&'src str),
    And(Box<Expr<'src>>, Box<Expr<'src>>),
    Or(Box<Expr<'src>>, Box<Expr<'src>>),
    Not(Box<Expr<'src>>),
}
```

### Files to create
- `swissarmyhammer-filter-expr/Cargo.toml` — depends on `chumsky`
- `swissarmyhammer-filter-expr/src/lib.rs` — re-exports
- `swissarmyhammer-filter-expr/src/parser.rs` — chumsky parser (character-level, single pass)
- `swissarmyhammer-filter-expr/src/eval.rs` — evaluator against a trait-based context
- Add to workspace `Cargo.toml` members

### Grammar rules (implemented as chumsky combinators)
- `#tag-name` → Tag (strip sigil, body = non-whitespace, non-sigil chars)
- `@user-name` → Assignee (strip sigil)
- `^card-ref` → Ref (strip sigil)
- `&&` / `and` / `AND` → And operator
- `||` / `or` / `OR` → Or operator
- `!` / `not` / `NOT` → Not operator
- `(` `)` → grouping
- Adjacent atoms without operator → implicit AND
- Precedence: NOT > AND > OR

## Acceptance Criteria
- [ ] `parse("#bug && @will")` produces `And(Tag("bug"), Assignee("will"))`
- [ ] `parse("#bug @will")` produces same (implicit AND)
- [ ] `parse("#bug || #feature")` produces `Or(Tag("bug"), Tag("feature"))`
- [ ] `parse("!#done")` produces `Not(Tag("done"))`
- [ ] `parse("#a || #b && #c")` respects precedence: `Or(Tag(a), And(Tag(b), Tag(c)))`
- [ ] `parse("(#a || #b) && #c")` respects grouping
- [ ] `parse("not #done and @will or #bug")` works with keyword operators
- [ ] Evaluator correctly matches entities with tags, assignees, refs
- [ ] Invalid expressions produce clear error messages with span info
- [ ] Crate compiles and all tests pass

## Tests
- [ ] `swissarmyhammer-filter-expr/src/parser.rs` — parser tests for each atom type, precedence, grouping, implicit AND, keyword operators, error cases
- [ ] `swissarmyhammer-filter-expr/src/eval.rs` — evaluator tests with mock FilterContext
- [ ] `cargo test -p swissarmyhammer-filter-expr` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.