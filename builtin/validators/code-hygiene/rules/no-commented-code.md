---
name: no-commented-code
description: Detect large blocks of commented-out code
---

# No Commented Code Validator

This rule is the fallback. `no-commented-code-parsed` answers the question
without you for Rust, Python, TypeScript, TSX, JavaScript, Go, Java, C, C++, C#
and Swift, by re-parsing each comment block with the file's own grammar, and it
supersedes this rule for those files. You read a file only when its language
has no grammar in that rule's roster, or when `sah doctor` could not find the
`sah` binary the rule invokes.

Read that as a limit on your authority. Where the parse decides, the parse
decides. Where no parse decides, apply the same standard the parse applies,
written out below.

## What to Check

Examine the file content for large blocks of commented-out code:

1. **Consecutive Commented Lines**: More than 5 lines of code that are commented out
2. **Commented Functions**: Entire functions or methods that are commented out
3. **Commented Classes**: Whole classes or structs that are commented out
4. **Disabled Code**: Code that appears to be temporarily disabled with comments

The standard is the tool's: a block is commented-out code when the text inside
it, with the comment delimiters removed, reads as several statements or items
of the file's own language. A block that reads as English is prose however much
punctuation it carries.

## Why This Matters

- Commented code clutters the codebase and reduces readability
- Version control (git) preserves history - we don't need commented code for "backup"
- Commented code often becomes stale and misleading
- It creates confusion about what code is active

## Exceptions (Don't Flag)

Two of these are structural, and they are the two `no-commented-code-parsed`
honors for the languages it covers. Apply them the same way here.

- **Documentation comments.** A block inside a doc comment is documentation,
  including a code example. That is the exemption an author reaches for: move
  the example into `///`, `/**`, `"""` or the language's own documentation
  form, and it is no longer a finding.
- **Blocks of 5 lines or fewer.**
- TODO/FIXME comments with explanations
- Single-line temporary debugging comments (though these should be removed too)
- Code examples showing "don't do this" patterns
- A comment sitting after code on the same line. It annotates that line rather
  than disabling it, however much a run of them looks like a block.
