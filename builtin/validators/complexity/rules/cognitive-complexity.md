---
name: cognitive-complexity
description: Limit cognitive complexity of functions
---

# Cognitive Complexity Validator

The numbers are computed for you. Compare them against the gates. Do not count.

## What the probe gives you

The `complexity` probe parses the file and computes, per function, the published
Sonar cognitive complexity and the maximum condition-nesting depth. It lists one
row per function that is over a gate. Each row names the function, its line, and
every measured number with the gate beside it:

    src/tag_parser.rs:155 `edit_line_markers` — cognitive complexity 21 (gate 15),
    max condition-nesting depth 5 (gate 4), 12 branches, at most 3 boolean
    operands in one condition, loops nested 2 deep, longest else-if chain 3

## The gates

- **Cognitive complexity 15 or more.**
- **Condition-nesting depth 4 or more.** This is the rule's long-standing limit:
  conditions nested more than 3 levels deep.

A function over either gate is a finding. A function over neither is not.

The other numbers — branches, boolean operands, loop nesting, else-if chain — are
**evidence, not gates**. Use them to say what is wrong. Never flag a function on
one of them alone.

## What to report

Report exactly the functions the probe listed, one finding each. Cite the row's
numbers and name the structure that produced them — the nested loop, the long
else-if chain, the condition with five operands.

**If the probe listed no rows for a file, there is nothing to report in that
file.** An empty list is a measurement, not a gap: it means every function in the
file scored under both gates.

## Do not recount

The score comes from the parse, so it is the same on every run. A count made by
eye is not. Never dispute the probe's number, never re-derive a depth from
reading the source, and never report a function the probe did not list.

Two consequences of the published algorithm are worth stating, because they are
where hand counting goes wrong:

- **A `match` or `switch` counts once for the whole construct.** Its arms are
  branches of one decision. An arm is **not** a nesting level, and ten arms do
  not score ten times. A two-arm `Option` match is score 1 at depth 1.
- **An `if` / `else if` / `else` chain is flat.** Each branch adds 1, and no
  branch nests inside the one before it. Three branches score 3 at depth 1.

## When the probe could not compute

A file whose language has no scorer mapping gets one row saying so. That row is
not a finding, and it is not permission to stay silent either. Read the source
and apply the two gates by the definitions above: count a `match`/`switch` once,
keep its arms off the nesting count, and keep an else-if chain flat. Report the
depth you counted and the chain you counted it along, so the number can be
checked.

## Exceptions

- **Tests.** A function the probe marks as a test is already exempt and will not
  be listed. Identify a test from its attribute or framework naming convention at
  the **definition**, never from the file name. A complex helper named
  `build_request` in a file called `foo_test.rs` is still a complex function and
  is still listed.
- **Generated code and macro expansions.**
- **Configuration parsing with many options**, where the score comes from a long
  flat list of simple cases rather than from nesting.
