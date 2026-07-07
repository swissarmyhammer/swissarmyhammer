---
name: documentation
description: /// doc comments on public API, one-line summary, Parameters/Returns/Throws matching the signature
---

# Swift Documentation

- **Every `public`/`open` declaration carries a `///` doc comment** (skip `override`s, whose docs are inherited).
- **Use `///`, never `/** … */`, for documentation comments.**
- **The first line is a single-sentence summary ending in a period;** any elaboration follows after a blank `///` line. DO: `/// Returns the element at the given index.`
- **Optionally Document exactly the parameters, return, and throws the signature has — no more, no less.** One parameter uses inline `- Parameter name:`; two or more use a `- Parameters:` block with nested names. `- Returns:` appears iff the result is non-`Void`; `- Throws:` iff the function `throws`. Documented names must match the signature.
- **Describe what/why, not how.** DON'T: `/// Loops over the internal bucket array…`. DO: `/// A Boolean value indicating whether the set contains the given element.`
- **Match voice to kind:** imperative verb phrase for effectful methods, noun phrase for values/types. Wrap symbol references in backticks: `` /// … or `nil` if `index` is out of bounds. ``
