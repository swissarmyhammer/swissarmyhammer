---
assignees:
- claude-code
position_column: todo
position_ordinal: ffcf80
title: No shipped rule owns a stuttering Go name
---
`revive` reports a stuttering exported Go name — a name that repeats its own
package name, so a caller writes the word two times: `staged.StagedType`. The
`missing-docs-go` rule turns that check off with `disableStutteringCheck`,
because the rule supersedes `missing-docs` and no other, and a stuttering name
is not a missing doc comment.

No shipped rule owns the defect. Measured: 27 shipped rules match a `.go` file,
and no one of them reads a name. The list is held by the acceptance test
`the_shipped_rules_that_read_a_go_file_stay_the_stated_list`. The naming rules
that ship — `swift/naming-clarity`, `swift/doc-parameter-naming` and
`js-ts/naming-and-style` — read no `.go` file.

## What a rule needs

- [ ] Decide the tool. `revive` states a stuttering finding under
      `RuleName: exported` with `Category: naming`, and states a documentation
      finding under the SAME rule name with `Category: comments`. Measured on
      revive 1.15.0. A Go naming rule can therefore run the same `exported`
      rule and select the `naming` category.
- [ ] Read the message form before a filter reads it. The default message is
      `type name will be used as staged.StagedType by other packages, and that
      stutters; consider calling this Type`. The `sayRepetitiveInsteadOfStutters`
      argument writes `that is repetitive` in place of `that stutters`, so a
      filter on the word alone breaks when the argument is set. The `Category`
      field does not move. Both forms are measured on revive 1.15.0.
- [ ] Survey the other Go naming tools before the rule is written. `exported`
      holds the stutter check alone, and `staticcheck` names more.
- [ ] Keep `disableStutteringCheck` in `missing-docs-go`. That rule supersedes
      `missing-docs` alone, so it must not report a name.
- [ ] Ship a fixture pair and an acceptance test through the real tool.
- [ ] Correct the sentence in
      `builtin/validators/code-hygiene/rules/missing-docs-go.md` that states no
      rule owns the defect, and correct
      `SHIPPED_RULES_THAT_READ_A_GO_FILE` in
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`.

Found on ^s2056e1. #tool-validators #objectivity