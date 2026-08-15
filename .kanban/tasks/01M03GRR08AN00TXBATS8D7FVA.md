---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe680
title: function-length-python fails the whole run for one file ruff cannot parse
---
`builtin/validators/code-hygiene/rules/function-length-python.md` exits 1 when ruff writes a row of `"code": "invalid-syntax"` onto the report. One file the parser could not read therefore throws away every finding the run did make.

`builtin/validators/README.md` states the answer for exactly this shape, since `^m6ba1bf` (commit 9fcdd8387): a script that judged the code and could not judge ONE item writes a line opening `sah-diagnostic:` and still exits 0, and the engine renders each marked line in the report. "Do not exit nonzero for a declined item. A nonzero exit fails the WHOLE run, so one unjudged path throws away every finding the run did make."

A file that does not parse is one such item. ruff measures the OTHER files of the same run and reports them.

Measured with ruff 0.14.5 while implementing `^d3j6sbt`, with the shipped script:

| the run | stdout | stderr | exit |
|---|---|---|---|
| a file whose `if` body never opens | nothing | `function-length-python: ruff could not measure <path>: invalid-syntax Expected an indented block after `if` statement` | 1 |
| that file beside a path that holds no file | nothing | the marked decline line AND the line above | 1 |

`^d3j6sbt` landed the `sah-diagnostic:` carrier beside this branch for a path ruff could not READ, and left the parse branch as it stands, because the card that drove it named the read shapes alone. The two branches now answer the same class of event two different ways, which is the reason to close this.

The work:

- Measure what a run over a corpus file that does not parse reports beside the files that do. 7 corpus files do not parse; the rule body names them.
- Decide the answer against `builtin/validators/README.md` and state it: a `sah-diagnostic:` line at exit 0, so the findings of the other files stand.
- Rewrite `the_shipped_python_function_length_tool_rule_breaks_on_a_file_it_cannot_parse` to hold the new answer, and stage a file over the gate beside the unparsable one so the test proves the findings survive — the shape `verify_unreadable_file_is_declined` already holds.
- Restate the "A file ruff cannot parse" section of the rule body against the shipped script.
- Read the sibling rules for the same shape before closing.

Found while implementing `^d3j6sbt`. #tool-validators #objectivity