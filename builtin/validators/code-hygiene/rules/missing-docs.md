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

- If you find missing docs in a file -- you need to check the WHOLE file and report every place docs are missing, not just the diff

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
  an undocumented public getter. Dart, Go, Python, Rust and TypeScript report
  it. Swift reports it only when the type declares no inherited type. Measured
  with swiftlint 0.65.0, with the shipped `missing-docs-swift` run script. Three
  probe files each declare one inherited type. The first probe file holds an
  undocumented `public struct Wide: Equatable` at row 1, an undocumented
  `public var name` at row 2, an undocumented `public func compute()` at row 3,
  an undocumented nested `public struct Inner` at row 4 and an undocumented
  `public var v` at row 5. The run reports 0 findings and exits 0. With
  `excludes_inherited_types: false` the run reports rows 1, 2, 3, 4 and 5. The
  second probe file holds a documented `open class Base`, an undocumented
  `public class Sub: Base` at row 6, an undocumented `public var name` at row 7
  and an undocumented `public func compute()` at row 8. The run reports 0
  findings and exits 0. With `excludes_inherited_types: false` the run reports
  rows 6, 7 and 8. The third probe file holds an undocumented
  `public enum Raw: String` at row 1, a case at row 2, an undocumented
  `public var name` at row 3 and an undocumented `public func compute()` at
  row 4. The run reports 0 findings and exits 0. With
  `excludes_inherited_types: false` the run reports rows 1, 2, 3 and 4. The
  fourth probe file holds the five rows of the first probe file in an
  undocumented `public struct Plain`, which declares no inherited type. The run
  reports rows 1, 2, 3, 4 and 5. The setting `excludes_inherited_types: true`
  is the cause. The shipped `missing-docs-swift` script writes that value into
  its own configuration, and it is swiftlint's own default as well. The setting
  makes the tool pass over the type declaration, over each member, over a
  nested type and over each member of the nested type. The three inherited
  types measured are a protocol conformance, a superclass and a raw-value type.
  No other inherited type was measured. Swift stays silent for every getter in
  a type that declares an inherited type.
- "Obvious implementations (Display, Debug, ToString, etc.)": four of the six
  stay silent on it. Dart, Go, Python and Rust stay silent. TypeScript reports
  an undocumented `toString()` and an undocumented `valueOf()`. Swift stays
  silent inside a type that declares an inherited type, for the reason above.
  Never carry this carve-out over from another language.

Run the language rule over the item to check either carve-out. What it reports
is the whole answer.

Never cite a carve-out against a language-rule finding. To exempt one item,
write the annotation its tool reads directly on the item in the code. Each
language rule states that annotation.

Note: Identify test items from the structural marker on the item itself (attribute, decorator, or framework-specific function-name convention applied at the definition), not from the file name or path. A function named `process_user` in a file called `foo_test.rs` is still a public API that needs documentation.
