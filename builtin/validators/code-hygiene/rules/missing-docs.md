---
name: missing-docs
description: Check that public functions and types have documentation comments
---

# Missing Documentation Validator

You are a code quality validator that checks for missing documentation on public APIs.


## What to Check

Examine the file content for public items lacking documentation:

- **Public Functions**: Functions without doc comments (///, /**, #, """)
- **Public Types**: Structs, classes, enums without doc comments
- **Public Constants**: Exported constants without explanation
- **Complex APIs**: Public interfaces that need usage examples

## Reporting

- Report every place docs are missing across what the review puts in scope. The
  prompt states that boundary and it decides which lines you may report on: the
  lines the change added or modified under a diff op, the whole of each named
  file under a file op. Report every instance inside it, never only the first.

## Exceptions (Don't Flag)

- Private or internal items
- Functions explicitly marked as tests by attribute or framework convention (e.g. `#[test]`, `#[tokio::test]`, `it(...)`, `def test_foo`, `func TestFoo(t *testing.T)`) and modules gated by `#[cfg(test)]` or `mod tests`
- Obvious implementations (Display, Debug, ToString, etc.)
- Generated code
- Simple getters/setters with self-explanatory names
- Items with #[doc(hidden)] or equivalent

Note: These exemptions yield to stricter language-specific documentation rules.
Where a language rule covers the file, that rule decides which of these
carve-outs hold. The rule's TOOL makes the decision, and the tool states the
decision when it runs: the tool reports the item, or the tool stays silent
about it.

A carve-out holds exactly where the tool stays silent, and nowhere else. So a
language-rule finding is never released by a carve-out above: the tool reported
the item, so the carve-out does not hold for that item. This answer needs no
reading of the language rule, and it holds for a language rule that states
nothing about the carve-outs.

A language rule can KEEP a carve-out. The Python documentation rule keeps the
"obvious implementation" carve-out: it leaves the magic-method code `D105` out of
its selector, so an undocumented `__str__` is not a finding.

A language rule can DROP a carve-out. The same Python rule reports an
undocumented `@property` getter, so the "simple getter" carve-out does not hold
for Python.

The two carve-outs were measured against each of the six shipped language rules
— Dart, Go, Python, Rust, Swift and TypeScript.

- "Simple getters/setters with self-explanatory names": five of the six report
  an undocumented public getter whatever its body holds. Dart, Go, Python, Rust
  and Swift report it. TypeScript reports it only when the body holds more than
  one statement: `missing-docs-typescript` reads the body for the word
  "simple", so a getter of one `return` and a setter of one assignment each
  stay silent, and an accessor of two statements reports. Measured over 4306
  files of six TypeScript repositories: 177 accessors reported before that
  reading and 30 after. Swift reports the getter in every type. Measured with
  Apple Swift 6.4.2 and the shipped `missing-docs-swift` run script: an
  undocumented `public var name: String { "" }` inside an undocumented
  `public struct Wide: Equatable` reports, and the structure reports as well.
  The toolchain rule reads the access modifier and the doc comment, and it
  does not read the body or the inherited types.
- "Obvious implementations (Display, Debug, ToString, etc.)": five of the six
  stay silent on it. Dart, Go, Python, Rust and TypeScript stay silent. Each
  one names the shape in its own way: Python leaves the magic-method code out
  of its selector, Go takes revive's fixed list of six method names, and
  TypeScript names `toString`, `valueOf`, `toLocaleString`, `toJSON` and every
  method keyed by a `Symbol` member. Swift reports it. Measured with the same
  script: an undocumented `public var description: String` inside
  `extension Wide: CustomStringConvertible` reports. A `public override`
  member is the one Swift shape that stays silent, because the toolchain rule
  skips every declaration that carries `override`. Never carry this carve-out
  over from another language.

Run the language rule over the item to check either carve-out. What it reports
is the whole answer.

Never cite a carve-out against a language-rule finding. To exempt one item,
write the annotation its tool reads directly on the item in the code. Each
language rule states that annotation.

Note: Identify test items from the structural marker on the item itself (attribute, decorator, or framework-specific function-name convention applied at the definition), not from the file name or path. A function named `process_user` in a file called `foo_test.rs` is still a public API that needs documentation.
