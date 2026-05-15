---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffff180
project: expr-filter
title: 'filter-expr: add $project atom to DSL grammar, parser, and FilterContext'
---
## What

Extend the filter expression DSL in `swissarmyhammer-filter-expr` to support project atoms written as `$project-slug`. The sigil for projects is `$`, chosen for readability alongside `#tag` / `@user` / `^ref` and free of existing operator conflicts.

**Files to modify (all in `swissarmyhammer-filter-expr/src/`):**
- `lib.rs` — add `Expr::Project(String)` variant next to `Tag`, `Assignee`, `Ref`. Update the crate-level doc comment at the top of the file to document the new atom. Update the `Expr` enum doc comment.
- `eval.rs` — add `fn has_project(&self, project: &str) -> bool;` to the `FilterContext` trait with a doc comment following the same pattern as `has_tag` / `has_assignee` / `has_ref`. Add an arm to `evaluate()` for `Expr::Project(project) => ctx.has_project(project)`. Update the `MockCtx` test helper to include a `projects` list and implement `has_project` for it.
- `parser.rs` — in `is_body_char`, add `$` to the excluded character set (so the string becomes `"#@^$()&|!"`). This prevents `$$bar` from parsing as `Project("$bar")`. In `atom_and_not`, add a `project` parser mirroring the existing `tag`/`mention`/`reference` parsers (`just('$').ignore_then(body).map(|s: &str| Expr::Project(s.to_string()))`) and add it to the `choice((tag, mention, reference, project, group))` list. Update the grammar doc comment above `filter_parser` to document the new atom: `atom = "#" body | "@" body | "^" body | "$" body | "(" expr ")"` and `body = [^ \t\n\r#@^$()&|!]+`.

**Context:**
- The parser is chumsky-based, with three existing atom parsers using the exact pattern to mirror. The evaluator is a straightforward match on `Expr`.
- Tasks store their project as a single-reference field `project` (see `swissarmyhammer-kanban/builtin/definitions/project.yaml`, `multiple: false`).
- Existing tests `error_invalid_chars` and `error_has_span_info` use `$$garbage` / `$$` as failure cases. After adding `$` to the excluded body chars, those strings still fail to parse (first `$` consumed as sigil, second `$` fails the `at_least(1)` body), so the tests remain valid but their inline comments should be updated so readers understand WHY the input fails.

**The filter matching against entities is implemented in a sibling card; this card only extends the DSL crate itself.**

## Acceptance Criteria

- [ ] `Expr::Project(String)` variant exists on the `Expr` enum
- [ ] `FilterContext::has_project(&self, project: &str) -> bool` trait method exists
- [ ] `evaluate()` dispatches `Expr::Project` to `ctx.has_project`
- [ ] Parser accepts `$auth-migration` → `Expr::Project("auth-migration")`
- [ ] Parser accepts `$v2.0` and `$project_1` (hyphen/dot/underscore bodies)
- [ ] Parser rejects `$` alone (zero-length body) and `$$bar` (empty body before second `$`)
- [ ] `$bug` parses as `Project("bug")` not as `Tag`; combinations like `$auth && #bug && @alice && ^01ABC` round-trip
- [ ] `Expr::Project(...)` participates in `&&`, `||`, `!`, implicit AND, and parentheses just like other atoms
- [ ] `is_body_char` excludes `$` so `$` cannot appear inside atom bodies
- [ ] Grammar doc comment in `parser.rs` above `filter_parser` lists the `$` atom and updated body character class
- [ ] Crate-level doc in `lib.rs` lists `$project` in the bullet list of atoms
- [ ] All existing tests still pass unchanged, including `error_invalid_chars` and `error_has_span_info`

## Tests

Add to `swissarmyhammer-filter-expr/src/parser.rs` tests module:
- [ ] `fn project_atom()` → `parse("$auth-migration").unwrap() == Expr::Project("auth-migration".into())`
- [ ] `fn project_with_dots()` → `$v2.0` parses as `Project("v2.0")`
- [ ] `fn project_with_underscores()` → `$my_project` parses
- [ ] `fn project_dollar_alone_is_error()` → `parse("$").is_err()`
- [ ] `fn project_double_dollar_is_error()` → `parse("$$bar").is_err()`
- [ ] `fn project_combines_with_and()` → `parse("$auth && #bug").unwrap()` is the expected `And(Project, Tag)` tree
- [ ] `fn project_implicit_and()` → `parse("$auth #bug @alice").unwrap()` builds a left-associative chain
- [ ] `fn not_project()` → `parse("!$auth").unwrap() == Expr::Not(Box::new(Expr::Project("auth".into())))`

Add to `swissarmyhammer-filter-expr/src/eval.rs` tests module:
- [ ] `fn project_positive()` — MockCtx with `projects: ["auth"]` → `Expr::Project("auth")` matches
- [ ] `fn project_negative()` — MockCtx with `projects: ["frontend"]` → `Expr::Project("auth")` does not match
- [ ] `fn project_case_insensitive()` — `$AUTH` matches a `projects: ["auth"]` context (mirror the existing `tag_case_insensitive` test)

Update to `swissarmyhammer-filter-expr/src/lib.rs` tests module:
- [ ] Update `TestCtx` to include `projects: Vec<String>` and implement `has_project`
- [ ] Update the `ctx()` helper signature to take a projects slice
- [ ] Add `fn parse_project_atom()` and `fn eval_project_match()` acceptance tests

Test command:
- [ ] `cargo test -p swissarmyhammer-filter-expr` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #expr-filter