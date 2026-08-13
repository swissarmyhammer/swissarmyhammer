---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzteqq5fea77ddqzeyp6g25z
  text: |-
    Archived. The `no-commented-code-parsed` rule is removed — see ^wwb6hk7.

    The rule shelled out to the `sah` binary to reach a function already linked
    into the calling process. That mechanism is deleted, not repaired. The
    `no-commented-code` prompt rule decides again, so this finding has no rule to
    apply to.
  timestamp: 2026-08-12T07:42:05.999932+00:00
position_column: todo
position_ordinal: ffc780
title: no-commented-code-parsed reports a do-not-do-this code example and offers no marker
---
`builtin/validators/code-hygiene/rules/no-commented-code-parsed.md` runs `sah tool code_context commented_code find` and declares `supersedes: [no-commented-code]`.

`no-commented-code.md` exempts "Code examples showing "don't do this" patterns". A non-doc comment of more than 5 lines that holds a deliberate bad-practice example re-parses as several statements with few error nodes, so the tool reports it. The distinction between "an example of what not to do" and "disabled code" lives in the prose around the block, which the parse does not read.

The rule states there is no escape hatch: "There is no `no-commented-code:ignore` marker and there will not be one." The only remedy offered is to move the example into a doc comment, which is a mandatory change to correct code.

The TODO carve-out is partial. A TODO written as prose is clean, which the rule's own cross-check table confirms. A TODO TAIL on disabled code does not save the block: the rule names hugo's `htmltemplate/exec_test.go` — "six disabled calls each carrying a `// TODO` tail" — as one of its three real findings. That reading is defensible, and it is narrower than the flat carve-out the prompt rule states.

Four carve-outs ARE reproduced structurally: documentation comments, blocks of five lines or fewer, single-line debugging comments, and a comment with live code to its left.

Decide whether the example carve-out gets a marker, or state on the rule that it is dropped and why.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity