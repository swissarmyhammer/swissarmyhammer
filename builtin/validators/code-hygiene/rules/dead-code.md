---
name: dead-code
description: Detect added symbols with no inbound callers, orphaned modules, unreachable branches, and commented-out code — the fallback for a language no dead-code tool rule covers
---

# Dead Code Validator

This rule is the fallback. Six tool rules answer the dead-code question without
you — `dead-code-rust`, `dead-code-go`, `dead-code-typescript`,
`dead-code-python`, `dead-code-dart` and `dead-code-swift` — and each supersedes
this rule for the files it matches. You read a file only when its language has
no tool rule, or when `sah doctor` could not find that rule's tool.

Read that as a limit on your authority. Where a tool decides, the tool decides.
Where no tool decides, apply the same standard the tools apply, written out
below.

A confirmed dead symbol is a **blocker**: delete it, don't ship it.

## The annotation contract

Staged code carries the language's own suppression marker, with a reason. Code
with no marker is dead.

There is no "I judged that a consumer is coming." A symbol introduced ahead of
its caller is legible only when the author said so in a marker the language's
own tool reads:

| Language | Marker |
|---|---|
| Rust | `#[expect(dead_code, reason = "...")]` |
| Go | `//lint:ignore U1000 <reason>` |
| TypeScript | `// ts-prune-ignore-next` |
| Python | `# noqa: V103` (and the sibling codes) |
| Dart | `// ignore: unused_element` (and the sibling diagnostics) |
| Swift | `// periphery:ignore` |

For a language not in that table, use the marker its own linter reads, and write
the reason beside it.

An unmarked symbol with no caller is dead however plausible its future looks.
Report it as a blocker, not as a warning, and do not soften the report with
"confirm a later task consumes this". The author has one answer available: write
the marker, or delete the code.

## What to Check

The engine attaches a `callers` probe result to each added symbol — its inbound
call sites from the call graph. Using that fact plus your reading of the diff,
flag:

1. **Uncalled added symbol**: an added or changed symbol with an empty inbound
   callgraph that is not an entry point, exported public API, or test, and that
   carries no suppression marker. The `callers` fact is authoritative: empty
   inbound + not exempt + not marked = dead.
2. **Orphaned modules**: a new module or file never wired into the production
   build / dependency graph.
3. **Unreachable branches**: branches that can never be taken (a condition that
   is always false, code after an unconditional return/throw).
4. **Commented-out code**: blocks of code disabled by comments rather than
   deleted — git history is the backup, not a comment.

## Why This Matters

- Dead code confuses every future reader about what is actually live.
- It carries maintenance cost (it shows up in searches, refactors, and reviews)
  while delivering nothing.
- Tests that exercise only a dead path give false confidence.

## Carve-outs (Don't Flag)

Three carve-outs remain, and every one of them is a structural fact you can
read off the definition rather than a judgment you make. Each is what a
compiler exempts on its own in the languages that have a tool rule.

- **Entry points**: `main`, binary entry functions, framework-invoked handlers,
  CLI command callbacks, registered hooks/callbacks, FFI exports — anything the
  runtime or a framework calls by convention rather than by an in-repo call site.
- **Exported public API**: a `pub`/exported item that is the crate's/library's
  surface for *external* callers. Its callers live outside this repo, so an empty
  inbound callgraph is expected, not dead. Where a language names its surface in
  one place — Python's `__all__`, a module's export list — that list is the
  answer, and a symbol absent from it is not exported.
- **Tests**: test functions and test-only helpers (identified by attribute or
  framework convention — `#[test]`, `#[tokio::test]`, `it(...)`, `def test_foo`,
  `func TestFoo(t *testing.T)`), and items gated by `#[cfg(test)]` / `mod tests`.

Work-in-process scaffolding is **not** a carve-out. It is the annotation
contract above: a marker, or dead.

Note: identify entry points / tests from the structural marker at the definition
(attribute, export modifier, registration), not from the file name. When the
`callers` fact shows real inbound callers, the symbol is **not** dead — the fact
refutes the claim; do not report it.
