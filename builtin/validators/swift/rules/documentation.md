---
name: documentation
description: /// doc comments on public API, one-line summary, Parameters/Returns/Throws matching the signature
---

# Swift Documentation

- Test methods do not need doc comments.
- **Add a `///` doc comment to every `public` or `open` declaration.** Skip `override` declarations. An `override` inherits its doc comment.
- **Use `///` for documentation comments. Do not use `/** … */`.**
- **Write the first line as a single-sentence summary that ends in a period.** Add any elaboration after a blank `///` line. DO: `/// Returns the element at the given index.`
- **Documenting parameters is optional. When you document them, match the signature exactly.** Do not add more than the signature has. Do not add less. Use inline `- Parameter name:` for one parameter. Use a `- Parameters:` block with nested names for two or more parameters. Add `- Returns:` only when the result is not `Void`. Add `- Throws:` only when the function `throws`. Documented names must match the signature.
- **Describe what the code does and why, not how it works.** DON'T: `/// Loops over the internal bucket array…`. DO: `/// A Boolean value indicating whether the set contains the given element.`
- **Match the voice to the kind of declaration.** Use an imperative verb phrase for a method with an effect. Use a noun phrase for a value or type. Wrap symbol references in backticks: `` /// … or `nil` if `index` is out of bounds. ``
