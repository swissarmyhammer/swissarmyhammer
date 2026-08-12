---
assignees:
- claude-code
position_column: todo
position_ordinal: ffd480
title: magic-numbers-swift and missing-docs-swift read a file swiftlint cannot decode as a clean file
---
swiftlint reads a source file as UTF-8 alone. A file that holds other bytes — a Swift file a person saved in Latin-1, or a binary file under a `.swift` name — makes swiftlint write ``Could not read contents of `<path>` `` to stderr. swiftlint then lints no line of that file.

Measured with swiftlint 0.65.0 over one file that holds `let name = "café"` in Latin-1, above one function of cyclomatic complexity 16:

| the run | status | stdout | stderr |
|---|---|---|---|
| the Latin-1 file alone | 0 | an empty array, 5 bytes | `Could not read contents of` |
| the Latin-1 file beside one file that holds a finding | 2 | 1 entry, 414 bytes | `Could not read contents of` |

Row 1 is the status and the report of a clean file. Row 2 passes the report test each Swift script makes at status 2. So neither the status nor the report tells the two apart, and the file swiftlint never read reaches the engine as a clean tree.

The `[ ! -r "$file" ]` guard admits the file, because the file IS readable. The DECODE is what fails.

`complexity-swift` was corrected on ^h2ezbs7. It now tests stderr for the decode message after it forwards swiftlint's own message, writes one line of its own, and exits 1. The acceptance test `the_shipped_swift_complexity_tool_rule_breaks_on_a_file_it_cannot_decode` holds that.

`builtin/validators/code-hygiene/rules/magic-numbers-swift.md` and `builtin/validators/code-hygiene/rules/missing-docs-swift.md` hold the same shape and make no such test. Give each the same test, state the measurement on each rule body, and hold each with an acceptance test that drives the shipped bytes. The Swift acceptance tests stand in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/magic_numbers.rs` and `.../missing_docs.rs`. `ShippedNamedPath.source` now takes bytes, so a probe can stage a file that is not UTF-8.

## The anchor, which each stderr test needs as well

Card ^h2ezbs7 found a second defect of the same shape and corrected it in `complexity-swift`: a `grep` that reads ALL of stderr answers the file NAME, because swiftlint writes the PATH of a file into stderr.

Both sibling rules hold the unanchored greps today: `grep -qF 'Could not read configuration'` and `grep -qF 'No lintable files found'`.

Measured with swiftlint 0.65.0 over one file under `Generated/` that holds one undocumented `public func` and the literal `86400`, beside a project `.swiftlint.yml` that states `excluded: [Generated]`. The file NAME is the one difference between the rows:

| the rule | the file name | findings | what the script wrote |
|---|---|---|---|
| `magic-numbers-swift` | `Plain.swift` | 0 | swiftlint's own `Error: No lintable files found at paths:` line |
| `magic-numbers-swift` | `Could not read configuration.swift` | 1 | `magic-numbers-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.` |
| `missing-docs-swift` | `Plain.swift` | 0 | swiftlint's own `Error: No lintable files found at paths:` line |
| `missing-docs-swift` | `Could not read configuration.swift` | 1 | `missing-docs-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.` |

Each second row is a WRONG FINDING on a file the project excludes.

swiftlint writes each message of its own at the START of a line, and it writes the path echo after `Error: `. `complexity-swift` now spells its three tests:

- `grep -qE '^Could not read configuration:'`
- ``grep -qE '^Could not read contents of ` '``
- `grep -qE '^Error: No lintable files found at paths:'`

Anchor every stderr grep of both sibling scripts the same way, state the measurement on each rule body, and hold the false-fire shape with an acceptance test on the shipped bytes.

#tool-validators #objectivity