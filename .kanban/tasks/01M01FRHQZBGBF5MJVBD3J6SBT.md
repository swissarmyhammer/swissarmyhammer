---
assignees:
- claude-code
position_column: todo
position_ordinal: ffde80
title: function-length-python reads a file ruff cannot open as a clean file
---
`builtin/validators/code-hygiene/rules/function-length-python.md` breaks on a file ruff cannot PARSE, since ^kmxvk6r. It still reads a file ruff cannot OPEN as a clean file.

Measured with ruff 0.14.5, one file for each run, against the shipped command line:

| the file | stdout | stderr | exit |
|---|---|---|---|
| one function over the gate | a report of one row | nothing | 1 |
| no function over the gate | a report of no row | nothing | 0 |
| a path that holds no file | a report of no row | `warning: Failed to lint absent.py: No such file or directory (os error 2)` | **0** |
| a file that is not UTF-8 | a report of no row | `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8` | **0** |
| a file with no read permission | a report of no row | `warning: Failed to lint clean.py: Permission denied (os error 13)` | **0** |

Rows 3, 4 and 5 are the defect. ruff exits 0 and writes only a `warning:` line, so the run reports no finding and exits 0, and the engine reads a file ruff never judged as a clean file. The status gate the script holds today accepts 0 and 1, so it admits every one of them.

The sibling `complexity-python` answers this with a `[ ! -r "$file" ]` test that names the path and exits 1 before the tool runs. That test admits a file that is not UTF-8, so a `warning:` test on ruff's stderr is needed beside it — the shape the three swiftlint rules use for `No lintable files found`.

Ship a fixture or an acceptance test for each of the three shapes, watched RED first, the way `the_shipped_python_function_length_tool_rule_breaks_on_a_file_it_cannot_parse` holds the parse failure. State each measurement in the rule body's "A file ruff cannot parse" section.

Found while implementing ^kmxvk6r. #tool-validators #objectivity